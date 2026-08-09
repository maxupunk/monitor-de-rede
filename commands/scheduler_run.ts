import { BaseCommand } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import { DateTime } from 'luxon'
import Monitor from '#models/monitor'
import Probe from '#models/probe'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ResultProcessor } from '#modules/monitoring/result_processor'
import { ProbeTaskDispatcher } from '#modules/probes/probe_task_dispatcher'
import { isProbeAlive, ProbeWatchdog } from '#modules/probes/probe_liveness'
import { VpnTrafficRecorder } from '#modules/vpn/vpn_traffic_recorder'
import { DiscoveryQueue } from '#modules/discovery/discovery_queue'
import { DataPrunerService } from '#services/data_pruner_service'
import { errorMessage } from '#modules/shared/errors'

/**
 * Cadência da leitura de status dos túneis. Fina de propósito: é o que faz a
 * tela de dispositivos VPN acompanhar um túnel que acabou de subir.
 */
const VPN_STATUS_INTERVAL_SECONDS = 10

/** Cadência da gravação de histórico de tráfego VPN — quatro linhas em `metrics` por peer, não precisa ser fina. */
const VPN_TRAFFIC_INTERVAL_SECONDS = 30

export default class SchedulerRun extends BaseCommand {
  static commandName = 'scheduler:run'
  static description = 'Inicia o processo de agendamento de verificações de monitores'

  static options: CommandOptions = {
    startApp: true,
    stayAlive: true,
  }

  private monitorRunner = new MonitorRunner()
  private resultProcessor = new ResultProcessor()
  private probeTaskDispatcher = new ProbeTaskDispatcher()
  private vpnTrafficRecorder = new VpnTrafficRecorder()
  private discoveryQueue = new DiscoveryQueue()
  private probeWatchdog = new ProbeWatchdog()
  private dataPrunerService = new DataPrunerService()
  private nextVpnStatusSyncAt: DateTime | null = null
  private nextVpnTrafficSyncAt: DateTime | null = null
  private nextDataPruneAt: DateTime | null = null

  async run() {
    this.logger.info('Processo Scheduler de Monitoramento inicializado.')

    while (true) {
      // Antes de despachar: um probe que caiu precisa aparecer como caído, ou o
      // operador só vê monitores parados sem explicação.
      try {
        await this.probeWatchdog.markStaleProbesOffline()
      } catch (err: unknown) {
        const errorMsg = errorMessage(err)
        this.logger.error(`Erro ao revisar a vida dos probes: ${errorMsg}`)
      }

      try {
        await this.checkDueMonitors()
      } catch (err: unknown) {
        const errorMsg = errorMessage(err)
        this.logger.error(`Erro durante ciclo do scheduler: ${errorMsg}`)
      }

      try {
        await this.syncVpnTrafficIfDue()
      } catch (err: unknown) {
        const errorMsg = errorMessage(err)
        this.logger.error(`Erro ao sincronizar tráfego VPN: ${errorMsg}`)
      }

      try {
        await this.runDiscoveryQueue()
      } catch (err: unknown) {
        const errorMsg = errorMessage(err)
        this.logger.error(`Erro na fila de descoberta de rede: ${errorMsg}`)
      }

      try {
        await this.runDataPrunerIfDue()
      } catch (err: unknown) {
        const errorMsg = errorMessage(err)
        this.logger.error(`Erro ao executar purga de dados antigos: ${errorMsg}`)
      }

      await new Promise((resolve) => setTimeout(resolve, 5000))
    }
  }

  private async syncVpnTrafficIfDue() {
    const now = DateTime.now()

    // O ciclo com histórico já sincroniza o status; roda primeiro para não
    // gravar duas amostras de tráfego separadas por poucos segundos.
    if (!this.nextVpnTrafficSyncAt || now >= this.nextVpnTrafficSyncAt) {
      await this.vpnTrafficRecorder.recordAll()
      this.nextVpnTrafficSyncAt = now.plus({ seconds: VPN_TRAFFIC_INTERVAL_SECONDS })
      this.nextVpnStatusSyncAt = now.plus({ seconds: VPN_STATUS_INTERVAL_SECONDS })
      return
    }

    if (this.nextVpnStatusSyncAt && now < this.nextVpnStatusSyncAt) return

    await this.vpnTrafficRecorder.syncAll()
    this.nextVpnStatusSyncAt = now.plus({ seconds: VPN_STATUS_INTERVAL_SECONDS })
  }

  private async runDataPrunerIfDue() {
    const now = DateTime.now()
    if (this.nextDataPruneAt && now < this.nextDataPruneAt) return

    const stats = await this.dataPrunerService.pruneAll()
    const totalDeleted =
      stats.outboxDeleted + stats.resultsDeleted + stats.metricsDeleted + stats.discoveryDeleted

    if (totalDeleted > 0) {
      this.logger.info(
        `[DataPruner] Purga de dados antigos executada: outbox=${stats.outboxDeleted}, resultados=${stats.resultsDeleted}, métricas=${stats.metricsDeleted}, descoberta=${stats.discoveryDeleted}`
      )
    }

    this.nextDataPruneAt = now.plus({ hours: 1 })
  }

  /**
   * Varreduras de rede: primeiro agenda as redes vencidas, depois executa o que
   * estiver pendente — inclusive o que o operador enfileirou pelo botão
   * "Escanear" em `/networks`, já que o processo HTTP não varre nada.
   */
  private async runDiscoveryQueue() {
    const queued = await this.discoveryQueue.scheduleDueNetworks()
    if (queued > 0) {
      this.logger.info(`[Scheduler] ${queued} varredura(s) periódica(s) de rede agendada(s).`)
    }

    const processed = await this.discoveryQueue.processPendingRuns()
    if (processed > 0) {
      this.logger.info(`[Scheduler] ${processed} varredura(s) de rede executada(s).`)
    }
  }

  private async checkDueMonitors() {
    const now = DateTime.now()

    const dueMonitors = await Monitor.query()
      .where('enabled', true)
      .preload('probe')
      .where((query) => {
        query.whereNull('next_run_at').orWhere('next_run_at', '<=', now.toSQL()!)
      })
      .limit(50)

    if (dueMonitors.length === 0) {
      return
    }

    this.logger.info(
      `Scheduler encontrou ${dueMonitors.length} monitor(es) vencidos para execução.`
    )

    for (const monitor of dueMonitors) {
      const nextRun = now.plus({ seconds: monitor.intervalSeconds || 60 })
      monitor.nextRunAt = nextRun
      await monitor.save()

      this.executeMonitorAsync(monitor)
    }
  }

  private async executeMonitorAsync(monitor: Monitor) {
    try {
      if (monitor.probeId) {
        const probe = monitor.probe ?? (await Probe.find(monitor.probeId))

        // Probe sem heartbeat não vai buscar tarefa nenhuma. Enfileirar em
        // silêncio deixaria o monitor parado em `unknown` sem explicação — que
        // é justamente como esse tipo de falha costuma passar despercebido.
        if (!isProbeAlive(probe)) {
          await this.reportProbeUnavailable(monitor, probe?.name ?? `#${monitor.probeId}`)
          return
        }

        this.logger.info(
          `[Scheduler] Despachando monitor #${monitor.id} (${monitor.type}) para Probe #${monitor.probeId}`
        )
        await this.probeTaskDispatcher.dispatchTask(monitor.probeId, {
          id: `task-${monitor.id}-${Date.now()}`,
          monitorId: monitor.id,
          type: monitor.type,
          timeoutMs: (monitor.timeoutSeconds || 5) * 1000,
          payload: monitor.configuration,
        })
      } else {
        this.logger.info(
          `[Scheduler] Executando monitor #${monitor.id} (${monitor.type}) - ${monitor.name}`
        )
        const result = await this.monitorRunner.runMonitor(monitor.type, monitor.configuration, {
          timeoutMs: (monitor.timeoutSeconds || 5) * 1000,
        })
        await this.resultProcessor.processResult(monitor.id, result, monitor.probeId)
        this.logger.info(`[Scheduler] Monitor #${monitor.id} finalizado: status=${result.status}`)
      }
    } catch (err: unknown) {
      const errorMsg = errorMessage(err)
      this.logger.error(`[Scheduler] Erro ao executar monitor #${monitor.id}: ${errorMsg}`)
    }
  }

  /**
   * Registra a impossibilidade de medir como resultado `unknown`.
   *
   * Não é `down`: o alvo pode estar perfeitamente no ar — quem sumiu foi o
   * agente. Mas a checagem precisa deixar rastro no histórico, senão o operador
   * vê apenas um monitor parado e sem motivo aparente.
   */
  private async reportProbeUnavailable(monitor: Monitor, probeLabel: string): Promise<void> {
    this.logger.warning(
      `[Scheduler] Monitor #${monitor.id} não executado: probe ${probeLabel} sem heartbeat`
    )

    const now = new Date()
    await this.resultProcessor.processResult(
      monitor.id,
      {
        success: false,
        status: 'unknown',
        durationMs: 0,
        startedAt: now,
        finishedAt: now,
        message: `Probe ${probeLabel} está sem heartbeat — a checagem não pôde ser executada.`,
        metrics: [],
        data: { probeId: monitor.probeId, reason: 'probe_offline' },
      },
      monitor.probeId
    )
  }
}

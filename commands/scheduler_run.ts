import { BaseCommand } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import { DateTime } from 'luxon'
import Monitor from '#models/monitor'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ResultProcessor } from '#modules/monitoring/result_processor'

export default class SchedulerRun extends BaseCommand {
  static commandName = 'scheduler:run'
  static description = 'Inicia o processo de agendamento de verificações de monitores'

  static options: CommandOptions = {
    startApp: true,
    stayAlive: true,
  }

  private monitorRunner = new MonitorRunner()
  private resultProcessor = new ResultProcessor()

  async run() {
    this.logger.info('Processo Scheduler de Monitoramento inicializado.')

    while (true) {
      try {
        await this.checkDueMonitors()
      } catch (err: unknown) {
        const errorMsg = err instanceof Error ? err.message : String(err)
        this.logger.error(`Erro durante ciclo do scheduler: ${errorMsg}`)
      }

      await new Promise((resolve) => setTimeout(resolve, 5000))
    }
  }

  private async checkDueMonitors() {
    const now = DateTime.now()

    const dueMonitors = await Monitor.query()
      .where('enabled', true)
      .where((query) => {
        query.whereNull('next_run_at').orWhere('next_run_at', '<=', now.toSQL()!)
      })
      .limit(50)

    if (dueMonitors.length === 0) {
      return
    }

    this.logger.info(`Scheduler encontrou ${dueMonitors.length} monitor(es) vencidos para execução.`)

    for (const monitor of dueMonitors) {
      const nextRun = now.plus({ seconds: monitor.intervalSeconds || 60 })
      monitor.nextRunAt = nextRun
      await monitor.save()

      this.executeMonitorAsync(monitor)
    }
  }

  private async executeMonitorAsync(monitor: Monitor) {
    try {
      this.logger.info(`[Scheduler] Executando monitor #${monitor.id} (${monitor.type}) - ${monitor.name}`)
      const result = await this.monitorRunner.runMonitor(monitor.type, monitor.configuration)
      await this.resultProcessor.processResult(monitor.id, result, monitor.probeId)
      this.logger.info(`[Scheduler] Monitor #${monitor.id} finalizado: status=${result.status}`)
    } catch (err: unknown) {
      const errorMsg = err instanceof Error ? err.message : String(err)
      this.logger.error(`[Scheduler] Erro ao executar monitor #${monitor.id}: ${errorMsg}`)
    }
  }
}

import { DateTime } from 'luxon'
import DiscoveryRun from '#models/discovery_run'
import Network from '#models/network'
import { DiscoveryService } from './discovery_service.js'
import { isScannableCidr, parseCidrRange } from './cidr_range.js'
import { errorMessage } from '#modules/shared/errors'

/**
 * Fila persistente de varreduras de rede.
 *
 * A arquitetura (§4.1) é explícita: *"o servidor HTTP não deverá executar
 * scans"*. Então `POST /api/networks/:id/scan` apenas grava uma `DiscoveryRun`
 * pendente e responde; quem varre é o `scheduler:run`, no mesmo laço que já
 * despacha os monitores vencidos.
 *
 * A fila mora na tabela `discovery_runs` — e não em memória — pelo mesmo motivo
 * das tarefas de probe: quem enfileira (processo HTTP) e quem executa
 * (scheduler) são processos diferentes.
 */

/** Varreduras processadas por ciclo — uma faixa /24 já leva dezenas de segundos. */
const RUNS_PER_CYCLE = 1

/** Piso do intervalo de varredura periódica, para não saturar a rede. */
export const MIN_SCAN_INTERVAL_SECONDS = 300

export class DiscoveryQueue {
  constructor(private discoveryService = new DiscoveryService()) {}

  /**
   * Enfileira uma varredura da rede informada.
   *
   * Devolve a run existente quando já há uma pendente ou em curso: dois cliques
   * no botão "Escanear" não devem virar duas varreduras concorrentes da mesma
   * faixa — elas competiriam pelos mesmos pings e duplicariam os resultados.
   */
  async enqueueNetworkScan(
    network: Network
  ): Promise<{ run: DiscoveryRun; alreadyQueued: boolean }> {
    if (!isScannableCidr(network.cidr)) {
      throw new Error(
        `A rede "${network.name}" não tem uma faixa CIDR varredurável (valor atual: "${network.cidr}").`
      )
    }

    const pending = await DiscoveryRun.query()
      .where('networkId', network.id)
      .whereIn('status', ['pending', 'running'])
      .orderBy('id', 'desc')
      .first()

    if (pending) return { run: pending, alreadyQueued: true }

    const range = parseCidrRange(network.cidr)
    const run = await DiscoveryRun.create({
      networkId: network.id,
      probeId: network.probeId ?? null,
      status: 'pending',
      startedAt: DateTime.now(),
      configuration: {
        cidr: network.cidr,
        usableHosts: range.usableHosts,
        truncated: range.truncated,
      },
    })

    return { run, alreadyQueued: false }
  }

  /**
   * Cria varreduras para as redes com `scan_enabled` cujo `next_scan_at` venceu.
   * Devolve quantas foram enfileiradas.
   */
  async scheduleDueNetworks(): Promise<number> {
    const now = DateTime.now()

    const networks = await Network.query()
      .where('active', true)
      .where('scanEnabled', true)
      .where((query) => {
        query.whereNull('nextScanAt').orWhere('next_scan_at', '<=', now.toSQL()!)
      })

    let queued = 0
    for (const network of networks) {
      if (!isScannableCidr(network.cidr)) continue

      // O próximo horário é gravado antes de enfileirar: se a varredura falhar,
      // o scheduler não fica tentando a mesma rede a cada ciclo.
      network.nextScanAt = now.plus({
        seconds: Math.max(MIN_SCAN_INTERVAL_SECONDS, network.scanInterval || 3600),
      })
      await network.save()

      const { alreadyQueued } = await this.enqueueNetworkScan(network)
      if (!alreadyQueued) queued++
    }

    return queued
  }

  /**
   * Executa as varreduras pendentes. Devolve quantas foram processadas.
   *
   * Uma por ciclo de propósito: varrer duas faixas ao mesmo tempo multiplica o
   * tráfego ICMP saindo do mesmo host e distorce a latência medida pelos
   * monitores que rodam no mesmo processo.
   */
  async processPendingRuns(): Promise<number> {
    const pendingRuns = await DiscoveryRun.query()
      .where('status', 'pending')
      .orderBy('id', 'asc')
      .limit(RUNS_PER_CYCLE)
      .preload('network')

    let processed = 0
    for (const run of pendingRuns) {
      const cidr = (run.configuration?.cidr as string | undefined) ?? run.network?.cidr
      if (!cidr) {
        run.status = 'failed'
        run.error = 'Varredura sem faixa CIDR definida'
        run.finishedAt = DateTime.now()
        await run.save()
        continue
      }

      try {
        await this.discoveryService.runDiscovery(cidr, run.networkId, run.probeId, run)
        await this.markNetworkScanned(run.networkId)
      } catch (err: unknown) {
        // `runDiscovery` já marcou a run como `failed`; aqui só evitamos que a
        // falha de uma rede interrompa o laço do scheduler.
        console.error(`[DiscoveryQueue] Varredura #${run.id} falhou: ${errorMessage(err)}`)
      }

      processed++
    }

    return processed
  }

  private async markNetworkScanned(networkId: number): Promise<void> {
    const network = await Network.find(networkId)
    if (!network) return

    const now = DateTime.now()
    network.lastScanAt = now
    if (network.scanEnabled) {
      network.nextScanAt = now.plus({
        seconds: Math.max(MIN_SCAN_INTERVAL_SECONDS, network.scanInterval || 3600),
      })
    }
    await network.save()
  }
}

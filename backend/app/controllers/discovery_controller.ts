import type { HttpContext } from '@adonisjs/core/http'
import { DateTime } from 'luxon'
import vine from '@vinejs/vine'
import DiscoveryRun from '#models/discovery_run'
import Network from '#models/network'
import { DiscoveryService } from '#modules/discovery/discovery_service'
import {
  scanSessionService,
  type ScannedHost,
} from '#modules/discovery/scan_session_service'
import { isScannableCidr } from '#modules/discovery/cidr_range'
import { errorMessage } from '#modules/shared/errors'

export default class DiscoveryController {
  private discoveryService = new DiscoveryService()

  /**
   * GET /api/discovery/scan-state
   *
   * Retorna o estado atual da varredura em memória. Permite que o frontend
   * restaure o progresso e os hosts encontrados ao voltar para a página.
   */
  async scanState({ response }: HttpContext) {
    const state = scanSessionService.getState()
    return response.ok({
      data: this.serializeScanState(state),
    })
  }

  /**
   * GET /api/discovery/scan-stream
   *
   * Server-Sent Events (SSE) com progresso, hosts encontrados e conclusão da
   * varredura ativa. Reconectar funciona: o listener recebe o estado completo
   * a cada mudança.
   */
  async scanStream({ response, request }: HttpContext) {
    const rawRes = response.response
    rawRes.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
    })

    const sendState = () => {
      try {
        const state = scanSessionService.getState()
        rawRes.write(`data: ${JSON.stringify(this.serializeScanState(state))}\n\n`)
      } catch {
        // Cliente desconectado.
      }
    }

    // Envia o estado imediatamente ao conectar.
    sendState()

    const unsubscribe = scanSessionService.subscribe(sendState)

    request.request.on('close', () => {
      unsubscribe()
    })
  }

  /**
   * POST /api/discovery/scan
   *
   * Inicia uma varredura de forma assíncrona. O scan continua rodando no
   * servidor mesmo se o frontend fechar a conexão. Iniciar um novo scan limpa
   * a sessão anterior.
   */
  async scan({ request, response }: HttpContext) {
    const schema = vine.object({
      networkId: vine.number(),
    })
    const { networkId } = await vine.validate({ schema, data: request.all() })

    const network = await Network.findOrFail(networkId)
    if (!isScannableCidr(network.cidr)) {
      return response.badRequest({
        message: `A rede "${network.name}" não tem uma faixa CIDR varredurável (valor atual: "${network.cidr}").`,
      })
    }

    const runRecord = await DiscoveryRun.create({
      networkId: network.id,
      probeId: network.probeId ?? null,
      status: 'running',
      startedAt: DateTime.now(),
      configuration: { cidr: network.cidr },
    })

    const signal = scanSessionService.startSession(runRecord.id, network.id)

    // Executa o scan em background, desacoplado da request.
    this.runDiscoveryInBackground(network.cidr, network.id, network.probeId ?? null, runRecord, signal)

    return response.accepted({
      runId: runRecord.id,
      status: 'running',
    })
  }

  /**
   * POST /api/discovery/scan-cancel
   *
   * Cancela a varredura em andamento.
   */
  async scanCancel({ response }: HttpContext) {
    scanSessionService.cancel()
    return response.ok({ status: 'cancelled' })
  }

  private async runDiscoveryInBackground(
    cidr: string,
    networkId: number,
    probeId: number | null,
    runRecord: DiscoveryRun,
    signal?: AbortSignal
  ) {
    try {
      await this.discoveryService.runDiscovery(
        cidr,
        networkId,
        probeId,
        runRecord,
        scanSessionService.asCallbacks(),
        signal
      )
      scanSessionService.complete()
    } catch (err: unknown) {
      const message = errorMessage(err)
      if (err instanceof Error && err.name === 'AbortError') {
        runRecord.status = 'failed'
        runRecord.finishedAt = DateTime.now()
        runRecord.error = 'Varredura cancelada.'
        await runRecord.save()
        scanSessionService.cancel()
        return
      }

      runRecord.status = 'failed'
      runRecord.finishedAt = DateTime.now()
      runRecord.error = message
      await runRecord.save()
      scanSessionService.fail(message)
    }
  }

  private serializeScanState(state: ReturnType<typeof scanSessionService.getState>) {
    return {
      runId: state.runId,
      networkId: state.networkId,
      status: state.status,
      phase: state.phase,
      progressCurrent: state.progressCurrent,
      progressTotal: state.progressTotal,
      hosts: state.hosts.map((host) => this.serializeHost(host)),
      logs: state.logs,
      error: state.error,
      startedAt: state.startedAt,
      finishedAt: state.finishedAt,
    }
  }

  private serializeHost(host: ScannedHost) {
    return {
      ipAddress: host.ipAddress,
      macAddress: host.macAddress ?? null,
      hostname: host.hostname ?? null,
      mdnsName: host.mdnsName ?? null,
      vendor: host.vendor ?? null,
      deviceType: host.deviceType ?? null,
      openPorts: host.openPorts ?? null,
      confidence: host.confidence,
      data: host.data ?? null,
    }
  }

  async runs({ request, response }: HttpContext) {
    const query = DiscoveryRun.query()
      .preload('network')
      .preload('probe')
      .withCount('results')
      .orderBy('id', 'desc')

    const pageParam = request.input('page')
    if (pageParam) {
      const page = Number(pageParam) || 1
      const limit = Math.min(Number(request.input('limit', 20)), 100)
      const paginated = await query.paginate(page, limit)

      return response.ok({
        data: paginated.all().map((run) => this.serializeRun(run)),
        meta: paginated.toJSON().meta,
      })
    }

    const runs = await query
    return response.ok(runs.map((run) => this.serializeRun(run)))
  }

  private serializeRun(run: DiscoveryRun) {
    const json = run.serialize()
    json.devicesFound = Number(run.$extras.results_count ?? 0)
    json.cidr = (run.configuration?.cidr as string | undefined) ?? run.network?.cidr ?? null
    json.networkName = run.network?.name ?? null
    return json
  }

  async runDetails({ params, response }: HttpContext) {
    const run = await DiscoveryRun.query().where('id', params.id).preload('results').firstOrFail()
    return response.ok(run)
  }

  /**
   * DELETE /api/discovery/cleanup?olderThanDays=7
   * Remove runs (e seus resultados em cascata) mais antigas que o prazo.
   */
  async cleanup({ request, response }: HttpContext) {
    const olderThanDays = Math.max(1, Number(request.input('olderThanDays', 7)))
    const cutoff = DateTime.now().minus({ days: olderThanDays })

    const runsToDelete = await DiscoveryRun.query().where('createdAt', '<=', cutoff.toSQL()).select('id')
    const runIds = runsToDelete.map((run) => run.id)

    if (runIds.length === 0) {
      return response.ok({ removedRuns: 0, removedResults: 0 })
    }

    await DiscoveryRun.query().whereIn('id', runIds).delete()

    return response.ok({
      removedRuns: runIds.length,
      message: `${runIds.length} varredura(s) antiga(s) removida(s).`,
    })
  }
}

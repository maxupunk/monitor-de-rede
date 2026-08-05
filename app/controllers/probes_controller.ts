import type { HttpContext } from '@adonisjs/core/http'
import crypto from 'node:crypto'
import { DateTime } from 'luxon'
import Probe from '#models/probe'
import { ProbeTaskDispatcher } from '#modules/probes/probe_task_dispatcher'
import { ProbeResultReceiver } from '#modules/probes/probe_result_receiver'
import { EventBus } from '#modules/events/event_bus'

export default class ProbesController {
  private taskDispatcher = new ProbeTaskDispatcher()
  private resultReceiver = new ProbeResultReceiver()
  private eventBus = EventBus.getInstance()

  private hashToken(rawToken: string): string {
    return crypto.createHash('sha256').update(rawToken).digest('hex')
  }

  /** Publica no SSE apenas quando o estado do probe realmente muda */
  private emitProbeStatus(probe: Probe, previousStatus?: Probe['status']) {
    if (previousStatus !== undefined && previousStatus === probe.status) return
    this.eventBus.emit('probe:status', {
      id: probe.id,
      probeId: probe.id,
      name: probe.name,
      status: probe.status,
      version: probe.version ?? null,
      lastSeenAt: probe.lastSeenAt?.toISO() ?? null,
    })
  }

  private async authenticateProbe(request: HttpContext['request']): Promise<Probe | null> {
    const rawToken = request.header('x-probe-token') || request.input('token')
    if (!rawToken || typeof rawToken !== 'string') {
      return null
    }

    const tokenHash = this.hashToken(rawToken)
    const probe = await Probe.query()
      .where('tokenHash', tokenHash)
      .whereNot('status', 'revoked')
      .first()

    return probe
  }

  async index({ response }: HttpContext) {
    const probes = await Probe.all()
    return response.ok(probes)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only(['siteId', 'name', 'tokenHash', 'status', 'version', 'configuration'])
    if (!data.status) data.status = 'pending'
    if (!data.tokenHash) {
      const rawSecret = crypto.randomBytes(32).toString('hex')
      data.tokenHash = this.hashToken(rawSecret)
    }
    const probe = await Probe.create(data)
    return response.created(probe)
  }

  async show({ params, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    return response.ok(probe)
  }

  async update({ params, request, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    const previousStatus = probe.status
    const data = request.only(['siteId', 'name', 'status', 'version', 'configuration'])
    probe.merge(data)
    await probe.save()
    this.emitProbeStatus(probe, previousStatus)
    return response.ok(probe)
  }

  async destroy({ params, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    await probe.delete()
    return response.noContent()
  }

  async revoke({ params, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    const previousStatus = probe.status
    probe.status = 'revoked'
    probe.revokedAt = DateTime.now()
    await probe.save()
    this.emitProbeStatus(probe, previousStatus)
    return response.ok(probe)
  }

  async heartbeat({ request, response }: HttpContext) {
    const probe = await this.authenticateProbe(request)
    if (!probe) {
      return response.unauthorized({ error: 'Probe não encontrado ou token inválido' })
    }

    const body = request.body()
    const previousStatus = probe.status
    probe.status = 'online'
    probe.lastSeenAt = DateTime.now()
    if (body.version) probe.version = body.version
    if (body.configuration) probe.configuration = body.configuration
    await probe.save()

    this.emitProbeStatus(probe, previousStatus)

    return response.ok({ status: 'ok', probeId: probe.id })
  }

  async getTasks({ request, response }: HttpContext) {
    const probe = await this.authenticateProbe(request)
    if (!probe) {
      return response.unauthorized({ error: 'Probe não encontrado ou token inválido' })
    }

    const tasks = this.taskDispatcher.getPendingTasks(probe.id)
    return response.ok({ tasks })
  }

  async postResults({ request, response }: HttpContext) {
    const probe = await this.authenticateProbe(request)
    if (!probe) {
      return response.unauthorized({ error: 'Probe não encontrado ou token inválido' })
    }

    const { results } = request.only(['results'])
    if (Array.isArray(results) && results.length > 0) {
      await this.resultReceiver.receiveBatchResults(probe.id, results)
    }

    return response.ok({ status: 'processed', count: Array.isArray(results) ? results.length : 0 })
  }

  async test({ params, response }: HttpContext) {
    return response.ok({ message: `Teste de conectividade enviado para o probe ID ${params.id}` })
  }
}

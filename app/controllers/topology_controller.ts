import type { HttpContext } from '@adonisjs/core/http'
import { TopologyService } from '#modules/topology/topology_service'
import vine from '@vinejs/vine'

export default class TopologyController {
  private topologyService = new TopologyService()

  /**
   * GET /api/topology
   * Retorna a estrutura em grafo (nodes & edges) da topologia de rede.
   */
  async index({ request, response }: HttpContext) {
    const siteId = request.input('site_id') ? Number(request.input('site_id')) : undefined
    const graph = await this.topologyService.getTopology(siteId)
    return response.ok(graph)
  }

  /**
   * POST /api/topology/links
   * Cria uma ligação manual entre dois dispositivos/interfaces.
   */
  async storeLink({ request, response }: HttpContext) {
    const schema = vine.object({
      source_device_id: vine.number(),
      target_device_id: vine.number(),
      source_interface_id: vine.number().optional(),
      target_interface_id: vine.number().optional(),
    })

    const payload = await vine.validate({
      schema,
      data: request.all(),
    })

    const link = await this.topologyService.createManualLink(
      payload.source_device_id,
      payload.target_device_id,
      payload.source_interface_id,
      payload.target_interface_id
    )

    return response.created(link)
  }

  /**
   * DELETE /api/topology/links/:id
   * Remove uma ligação de topologia existente.
   */
  async destroyLink({ params, response }: HttpContext) {
    const linkId = Number(params.id)
    const success = await this.topologyService.deleteLink(linkId)
    if (!success) {
      return response.notFound({ message: 'Ligação não encontrada' })
    }
    return response.ok({ message: 'Ligação removida com sucesso' })
  }

  /**
   * POST /api/topology/recalculate
   * Força a execução da inferência de sub-redes e atualização dos links.
   */
  async recalculate({ response }: HttpContext) {
    const inferredLinks = await this.topologyService.inferSubnetLinks()
    return response.ok({
      message: 'Recálculo de topologia concluído',
      inferredCount: inferredLinks.length,
    })
  }
}

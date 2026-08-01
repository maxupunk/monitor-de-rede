import type { HttpContext } from '@adonisjs/core/http'

export default class TopologyController {
  async index({ response }: HttpContext) {
    return response.ok({ nodes: [], edges: [] })
  }

  async storeLink({ request, response }: HttpContext) {
    const data = request.only(['sourceDeviceId', 'targetDeviceId', 'linkType', 'confidence'])
    return response.created(data)
  }

  async updateLink({ params, request, response }: HttpContext) {
    const data = request.only(['linkType', 'confidence', 'confirmed'])
    return response.ok({ id: params.id, ...data })
  }

  async destroyLink({ params, response }: HttpContext) {
    return response.ok({ id: params.id, deleted: true })
  }
}

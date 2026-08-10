import type { HttpContext } from '@adonisjs/core/http'
import Site from '#models/site'
import { ResourceCleanupService } from '#services/resource_cleanup_service'

export default class SitesController {
  private cleanupService = new ResourceCleanupService()

  async index({ response }: HttpContext) {
    const sites = await Site.all()
    return response.ok(sites)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only(['name', 'description', 'location', 'active'])
    const site = await Site.create(data)
    return response.created(site)
  }

  async show({ params, response }: HttpContext) {
    const site = await Site.findOrFail(params.id)
    return response.ok(site)
  }

  async update({ params, request, response }: HttpContext) {
    const site = await Site.findOrFail(params.id)
    const data = request.only(['name', 'description', 'location', 'active'])
    site.merge(data)
    await site.save()
    return response.ok(site)
  }

  async destroy({ params, response }: HttpContext) {
    const site = await Site.findOrFail(params.id)
    await this.cleanupService.deleteSite(site.id)
    return response.noContent()
  }
}

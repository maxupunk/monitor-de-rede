import type { HttpContext } from '@adonisjs/core/http'
import Device from '#models/device'

export default class DevicesController {
  async index({ response }: HttpContext) {
    const devices = await Device.all()
    return response.ok(devices)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only(['siteId', 'networkId', 'name', 'type', 'vendor', 'model', 'serialNumber', 'description', 'status'])
    const device = await Device.create(data)
    return response.created(device)
  }

  async show({ params, response }: HttpContext) {
    const device = await Device.findOrFail(params.id)
    return response.ok(device)
  }

  async update({ params, request, response }: HttpContext) {
    const device = await Device.findOrFail(params.id)
    const data = request.only(['siteId', 'networkId', 'name', 'type', 'vendor', 'model', 'serialNumber', 'description', 'status'])
    device.merge(data)
    await device.save()
    return response.ok(device)
  }

  async destroy({ params, response }: HttpContext) {
    const device = await Device.findOrFail(params.id)
    await device.delete()
    return response.noContent()
  }

  async interfaces({ params, response }: HttpContext) {
    return response.ok({ deviceId: params.id, interfaces: [] })
  }

  async monitors({ params, response }: HttpContext) {
    return response.ok({ deviceId: params.id, monitors: [] })
  }

  async metrics({ params, response }: HttpContext) {
    return response.ok({ deviceId: params.id, metrics: [] })
  }

  async events({ params, response }: HttpContext) {
    return response.ok({ deviceId: params.id, events: [] })
  }
}

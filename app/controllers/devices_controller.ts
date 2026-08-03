import type { HttpContext } from '@adonisjs/core/http'
import Device from '#models/device'
import Monitor from '#models/monitor'

export default class DevicesController {
  async index({ response }: HttpContext) {
    const devices = await Device.query().preload('site').preload('parent')
    return response.ok(devices)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only([
      'siteId',
      'networkId',
      'parentId',
      'ipAddress',
      'name',
      'type',
      'vendor',
      'model',
      'serialNumber',
      'description',
      'status',
      'isMonitored',
      'snmpEnabled',
      'snmpCommunity',
      'snmpVersion',
    ])

    const device = await Device.create(data)
    await this.syncDeviceMonitor(device)

    return response.created(device)
  }

  async show({ params, response }: HttpContext) {
    const device = await Device.query().where('id', params.id).preload('site').preload('parent').firstOrFail()
    return response.ok(device)
  }

  async update({ params, request, response }: HttpContext) {
    const device = await Device.findOrFail(params.id)
    const data = request.only([
      'siteId',
      'networkId',
      'parentId',
      'ipAddress',
      'name',
      'type',
      'vendor',
      'model',
      'serialNumber',
      'description',
      'status',
      'isMonitored',
      'snmpEnabled',
      'snmpCommunity',
      'snmpVersion',
    ])

    device.merge(data)
    await device.save()
    await this.syncDeviceMonitor(device)

    return response.ok(device)
  }

  private async syncDeviceMonitor(device: Device) {
    const existingMonitor = await Monitor.query().where('deviceId', device.id).first()

    if (device.isMonitored) {
      const targetHost = device.ipAddress || device.name
      if (existingMonitor) {
        existingMonitor.enabled = true
        existingMonitor.name = `Ping ${device.name}`
        existingMonitor.configuration = { host: targetHost }
        await existingMonitor.save()
      } else {
        await Monitor.create({
          deviceId: device.id,
          name: `Ping ${device.name}`,
          type: 'ping',
          configuration: { host: targetHost },
          intervalSeconds: 60,
          timeoutSeconds: 5,
          retryCount: 3,
          enabled: true,
          status: 'unknown',
        })
      }
    } else if (existingMonitor) {
      existingMonitor.enabled = false
      await existingMonitor.save()
    }
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

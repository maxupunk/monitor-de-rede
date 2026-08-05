import type { HttpContext } from '@adonisjs/core/http'
import Device from '#models/device'
import Monitor from '#models/monitor'
import Metric from '#models/metric'
import AlertEvent from '#models/alert_event'
import { syncZabbixTemplateMonitor } from '#modules/zabbix/zabbix_template_monitor_sync'
import { ResourceCleanupService } from '#services/resource_cleanup_service'

export default class DevicesController {
  private cleanupService = new ResourceCleanupService()

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
      'zabbixTemplateId',
    ])

    const device = await Device.create(data)
    await this.syncDeviceMonitor(device)
    await syncZabbixTemplateMonitor(device)

    return response.created(device)
  }

  async show({ params, response }: HttpContext) {
    const device = await Device.query()
      .where('id', params.id)
      .preload('site')
      .preload('parent')
      .preload('vpnPeer')
      .preload('zabbixTemplate', (query) => query.preload('items'))
      .firstOrFail()
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
      'zabbixTemplateId',
    ])

    device.merge(data)
    await device.save()
    await this.syncDeviceMonitor(device)
    await syncZabbixTemplateMonitor(device)

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
    await this.cleanupService.deleteDevice(device.id)
    return response.noContent()
  }

  async interfaces({ params, response }: HttpContext) {
    return response.ok({ deviceId: params.id, interfaces: [] })
  }

  async monitors({ params, response }: HttpContext) {
    const monitors = await Monitor.query()
      .where('deviceId', params.id)
      .preload('results', (query) => query.orderBy('startedAt', 'desc').limit(1))

    const formatted = monitors.map((mon) => {
      const json = mon.serialize()
      const latestResult = mon.results?.[0]
      return {
        ...json,
        target: mon.target,
        port: mon.port,
        latencyMs: latestResult?.latencyMs ?? undefined,
      }
    })

    return response.ok(formatted)
  }

  async metrics({ params, response }: HttpContext) {
    const metrics = await Metric.query()
      .where('deviceId', params.id)
      .preload('interface')
      .orderBy('recordedAt', 'desc')
      .limit(1000)

    const formatted = metrics
      .filter((met) => {
        if (met.interfaceId && met.interface) {
          return met.interface.adminStatus === 'up'
        }
        return true
      })
      .map((met) => {
        return {
          id: met.id,
          deviceId: met.deviceId,
          interfaceId: met.interfaceId,
          interfaceName: met.interface ? met.interface.name : null,
          metricName: met.name,
          metricValue: met.value,
          unit: met.unit,
          createdAt: met.recordedAt
            ? met.recordedAt.toFormat('dd/MM/yyyy HH:mm:ss')
            : met.createdAt.toFormat('dd/MM/yyyy HH:mm:ss'),
        }
      })

    return response.ok(formatted)
  }

  async events({ params, response }: HttpContext) {
    const events = await AlertEvent.query()
      .where('deviceId', params.id)
      .orderBy('createdAt', 'desc')
      .limit(50)

    const formatted = events.map((evt) => {
      return {
        id: evt.id,
        deviceId: evt.deviceId,
        eventType: evt.status,
        severity: evt.severity,
        message: evt.message || 'Sem mensagem de detalhes',
        createdAt: evt.createdAt ? evt.createdAt.toFormat('dd/MM/yyyy HH:mm:ss') : '',
      }
    })

    return response.ok(formatted)
  }
}

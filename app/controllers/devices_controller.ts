import type { HttpContext } from '@adonisjs/core/http'
import Device from '#models/device'
import Monitor from '#models/monitor'
import Metric from '#models/metric'
import AlertEvent from '#models/alert_event'
import { monitorListQuery, presentMonitors } from '#modules/monitoring/monitor_presenter'
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

  /**
   * Mesmo payload de `GET /api/monitors`, filtrado por equipamento.
   *
   * A aba "Monitores" de `/devices/:id` usa o mesmo componente de listagem de
   * `/monitors`: se o contrato divergir, o componente perde linha do tempo e
   * sparkline justamente na tela em que o operador está investigando um
   * equipamento específico.
   */
  async monitors({ params, response }: HttpContext) {
    const monitors = await monitorListQuery()
      .where('deviceId', params.id)
      .preload('device')
      .preload('probe')

    return response.ok(await presentMonitors(monitors))
  }

  async metrics({ params, request, response }: HttpContext) {
    const pageParam = request.input('page')
    if (pageParam) {
      const page = Number(pageParam) || 1
      const limit = Math.min(Number(request.input('limit', 20)), 100)

      const paginated = await Metric.query()
        .where('deviceId', params.id)
        .preload('interface')
        .orderBy('recordedAt', 'desc')
        .paginate(page, limit)

      const json = paginated.toJSON()
      const data = (paginated.all() as Metric[])
        .filter((met) => {
          if (met.interfaceId && met.interface) {
            return met.interface.adminStatus === 'up'
          }
          return true
        })
        .map((met) => ({
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
        }))

      return response.ok({ data, meta: json.meta })
    }

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

  async events({ params, request, response }: HttpContext) {
    const pageParam = request.input('page')
    if (pageParam) {
      const page = Number(pageParam) || 1
      const limit = Math.min(Number(request.input('limit', 20)), 100)

      const paginated = await AlertEvent.query()
        .where('deviceId', params.id)
        .orderBy('createdAt', 'desc')
        .paginate(page, limit)

      const json = paginated.toJSON()
      const data = (paginated.all() as AlertEvent[]).map((evt) => ({
        id: evt.id,
        deviceId: evt.deviceId,
        eventType: evt.status,
        severity: evt.severity,
        message: evt.message || 'Sem mensagem de detalhes',
        createdAt: evt.createdAt ? evt.createdAt.toFormat('dd/MM/yyyy HH:mm:ss') : '',
      }))

      return response.ok({ data, meta: json.meta })
    }

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

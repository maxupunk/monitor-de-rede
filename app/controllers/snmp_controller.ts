import type { HttpContext } from '@adonisjs/core/http'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import { SnmpService } from '#modules/snmp/snmp_service'
import vine from '@vinejs/vine'

export default class SnmpController {
  private snmpService = new SnmpService()

  /**
   * POST /api/devices/:id/snmp/poll
   * Executa varredura SNMP sob demanda para um dispositivo.
   */
  async poll({ params, request, response }: HttpContext) {
    const device = await Device.find(params.id)
    if (!device) {
      return response.notFound({ message: 'Dispositivo não encontrado' })
    }

    const schema = vine.object({
      host: vine.string().optional(),
      version: vine.enum(['v1', 'v2c', 'v3']).optional(),
      community: vine.string().optional(),
      port: vine.number().optional(),
    })

    const payload = await vine.validate({
      schema,
      data: request.all(),
    })

    const version = (payload.version || device.snmpVersion || 'v2c') as 'v1' | 'v2c' | 'v3'
    const config = {
      host: payload.host || device.ipAddress || device.name,
      version,
      community: payload.community || device.snmpCommunity || 'public',
      port: payload.port || 161,
    }

    try {
      const result = await this.snmpService.pollDevice(device, config)
      return response.ok({
        message: 'Varredura SNMP executada com sucesso',
        result,
      })
    } catch (error) {
      return response.badRequest({
        message: 'Falha ao executar varredura SNMP',
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  /**
   * GET /api/devices/:id/interfaces
   * Retorna a lista de interfaces de um dispositivo com métricas de tráfego.
   */
  async interfaces({ params, response }: HttpContext) {
    const device = await Device.find(params.id)
    if (!device) {
      return response.notFound({ message: 'Dispositivo não encontrado' })
    }

    const interfaces = await DeviceInterface.query()
      .where('deviceId', device.id)
      .preload('metrics', (q) => {
        q.orderBy('recordedAt', 'desc').limit(10)
      })

    const formatted = interfaces.map((intf) => {
      const json = intf.serialize()
      return {
        ...json,
        ifIndex: intf.snmpIndex,
        ifName: intf.name,
        ifDescr: intf.description,
        ifAdminStatus: intf.adminStatus,
        ifOperStatus: intf.operStatus,
        ifSpeed: intf.speed,
        ifType: intf.type,
      }
    })

    return response.ok(formatted)
  }
}

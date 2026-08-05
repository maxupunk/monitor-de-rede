import type { HttpContext } from '@adonisjs/core/http'
import vine from '@vinejs/vine'
import { PortScannerService } from '#modules/network_tools/port_scanner_service'

export default class PortScanController {
  private portScannerService = new PortScannerService()

  /**
   * POST /api/port-scan
   * Executa uma varredura de portas TCP ou UDP sob demanda em um host — ferramenta
   * reutilizável (não exige que o host já esteja cadastrado como dispositivo).
   */
  async scan({ request, response }: HttpContext) {
    const schema = vine.object({
      host: vine.string().trim().minLength(1),
      protocol: vine.enum(['tcp', 'udp']),
      ports: vine
        .array(vine.number().range([1, 65535]))
        .minLength(1)
        .maxLength(1024),
      timeoutMs: vine.number().range([100, 5000]).optional(),
    })

    const payload = await vine.validate({ schema, data: request.all() })

    try {
      const results = await this.portScannerService.scan(
        payload.host,
        payload.ports,
        payload.protocol,
        payload.timeoutMs
      )
      return response.ok({ host: payload.host, protocol: payload.protocol, results })
    } catch (error) {
      return response.badRequest({
        message: 'Falha ao executar varredura de portas',
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }
}

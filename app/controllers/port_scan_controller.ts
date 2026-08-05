import type { HttpContext } from '@adonisjs/core/http'
import vine from '@vinejs/vine'
import { PortScannerService } from '#modules/network_tools/port_scanner_service'
import { errorMessage } from '#modules/shared/errors'

export default class PortScanController {
  private portScannerService = new PortScannerService()

  /**
   * POST /api/port-scan
   * Executa uma varredura de portas TCP ou UDP sob demanda em um host — ferramenta
   * reutilizável (não exige que o host já esteja cadastrado como dispositivo).
   *
   * A resposta é transmitida como NDJSON (uma linha JSON por porta verificada), para que o
   * frontend acompanhe o progresso em tempo real em vez de esperar a varredura inteira.
   * Se o cliente cancelar (fechar a conexão), interrompemos a varredura no próximo lote.
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

    const rawRes = response.response
    rawRes.writeHead(200, {
      'Content-Type': 'application/x-ndjson',
      'Cache-Control': 'no-cache',
      'Connection': 'keep-alive',
    })

    const abortController = new AbortController()
    request.request.on('close', () => abortController.abort())

    const writeLine = (obj: unknown) => {
      try {
        rawRes.write(JSON.stringify(obj) + '\n')
      } catch {
        // Conexão já encerrada pelo cliente
      }
    }

    try {
      await this.portScannerService.scan(
        payload.host,
        payload.ports,
        payload.protocol,
        payload.timeoutMs,
        {
          signal: abortController.signal,
          onResult: (item) => writeLine({ type: 'result', ...item }),
        }
      )
      if (!abortController.signal.aborted) {
        writeLine({ type: 'done' })
      }
    } catch (error) {
      writeLine({
        type: 'error',
        message: errorMessage(error),
      })
    } finally {
      rawRes.end()
    }
  }
}

import { BaseCommand, flags } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import { DiscoveryService } from '#modules/discovery/discovery_service'
import { errorMessage } from '#modules/shared/errors'

export default class NetworkScan extends BaseCommand {
  static commandName = 'network:scan'
  static description = 'Executa uma varredura de rede manual'

  @flags.string({ description: 'CIDR da rede a ser escaneada (ex: 192.168.1.0/24)' })
  declare cidr?: string

  static options: CommandOptions = {
    startApp: true,
  }

  private discoveryService = new DiscoveryService()

  async run() {
    const targetCidr = this.cidr || '192.168.1.0/24'
    this.logger.info(`Iniciando varredura de rede no bloco CIDR: ${targetCidr}...`)

    try {
      const results = await this.discoveryService.runDiscovery(targetCidr)
      this.logger.info(`Varredura concluída! ${results.length} dispositivo(s) encontrado(s):`)

      for (const host of results) {
        this.logger.info(
          ` -> IP: ${host.ipAddress.padEnd(15)} | MAC: ${(host.macAddress || 'N/A').padEnd(17)} | Hostname: ${host.hostname || 'N/A'} | Tipo: ${host.deviceType || 'unknown'}`
        )
      }
    } catch (err: unknown) {
      const errorMsg = errorMessage(err)
      this.logger.error(`Erro ao executar varredura de rede: ${errorMsg}`)
    }
  }
}

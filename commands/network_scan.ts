import { BaseCommand, flags } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'

export default class NetworkScan extends BaseCommand {
  static commandName = 'network:scan'
  static description = 'Executa uma varredura de rede manual'

  @flags.string({ description: 'CIDR da rede a ser escaneada (ex: 192.168.1.0/24)' })
  declare cidr?: string

  static options: CommandOptions = {
    startApp: true,
  }

  async run() {
    this.logger.info(`Iniciando varredura de rede ${this.cidr || 'padrão'}`)
  }
}

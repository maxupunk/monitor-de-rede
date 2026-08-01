import { BaseCommand, args } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'

export default class MonitorTest extends BaseCommand {
  static commandName = 'monitor:test'
  static description = 'Executa um teste pontual em um monitor por ID'

  @args.string({ description: 'ID do monitor' })
  declare monitorId: string

  static options: CommandOptions = {
    startApp: true,
  }

  async run() {
    this.logger.info(`Executando teste no monitor ID: ${this.monitorId}`)
  }
}

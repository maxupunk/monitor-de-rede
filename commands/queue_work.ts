import { BaseCommand } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'

export default class QueueWork extends BaseCommand {
  static commandName = 'queue:work'
  static description = 'Inicia o worker de processamento de filas da aplicação'

  static options: CommandOptions = {
    startApp: true,
    stayAlive: true,
  }

  async run() {
    this.logger.info('Worker de filas inicializado.')
  }
}

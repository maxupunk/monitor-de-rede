import { BaseCommand } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'

export default class SchedulerRun extends BaseCommand {
  static commandName = 'scheduler:run'
  static description = 'Inicia o processo de agendamento de verificações de monitores'

  static options: CommandOptions = {
    startApp: true,
    stayAlive: true,
  }

  async run() {
    this.logger.info('Processo Scheduler inicializado.')
  }
}

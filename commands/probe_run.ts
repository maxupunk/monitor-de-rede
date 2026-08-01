import { BaseCommand } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'

export default class ProbeRun extends BaseCommand {
  static commandName = 'probe:run'
  static description = 'Inicia o processo do probe de rede local ou remoto'

  static options: CommandOptions = {
    startApp: true,
    stayAlive: true,
  }

  async run() {
    this.logger.info('Processo Probe inicializado e aguardando tarefas.')
  }
}

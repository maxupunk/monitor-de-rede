import { BaseCommand, flags } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import { ProbeAgent } from '#modules/probes/probe_agent'

export default class ProbeRun extends BaseCommand {
  static commandName = 'probe:run'
  static description = 'Inicia o processo do probe de rede local ou remoto'

  static options: CommandOptions = {
    startApp: true,
    stayAlive: true,
  }

  @flags.string({ description: 'URL do servidor central de monitoramento', required: false })
  declare serverUrl: string

  @flags.string({ description: 'Token de autenticação do probe', required: false })
  declare token: string

  @flags.number({ description: 'Intervalo de polling/heartbeat em ms', required: false })
  declare intervalMs: number

  async run() {
    this.logger.info('Processo Probe inicializado e conectando ao servidor central.')

    const agent = new ProbeAgent({
      serverUrl: this.serverUrl,
      probeToken: this.token,
      intervalMs: this.intervalMs,
    })

    await agent.start()
  }
}

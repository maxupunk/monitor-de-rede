import { BaseCommand, args } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import Monitor from '#models/monitor'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ResultProcessor } from '#modules/monitoring/result_processor'

export default class MonitorTest extends BaseCommand {
  static commandName = 'monitor:test'
  static description = 'Executa um teste pontual em um monitor por ID'

  @args.string({ description: 'ID do monitor' })
  declare monitorId: string

  static options: CommandOptions = {
    startApp: true,
  }

  private monitorRunner = new MonitorRunner()
  private resultProcessor = new ResultProcessor()

  async run() {
    const id = Number.parseInt(this.monitorId, 10)
    if (Number.isNaN(id)) {
      this.logger.error('ID do monitor inválido.')
      return
    }

    const monitor = await Monitor.find(id)
    if (!monitor) {
      this.logger.error(`Monitor #${id} não encontrado no banco de dados.`)
      return
    }

    this.logger.info(
      `Executando teste no monitor #${monitor.id} (${monitor.type}) - ${monitor.name}...`
    )

    const result = await this.monitorRunner.runMonitor(monitor.type, monitor.configuration)
    await this.resultProcessor.processResult(monitor.id, result, monitor.probeId)

    this.logger.info(`Status: ${result.status.toUpperCase()}`)
    this.logger.info(`Mensagem: ${result.message}`)
    this.logger.info(`Duração: ${result.durationMs}ms`)
  }
}

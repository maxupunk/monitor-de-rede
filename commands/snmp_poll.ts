import { BaseCommand, args, flags } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import Device from '#models/device'
import { SnmpService } from '#modules/snmp/snmp_service'

export default class SnmpPoll extends BaseCommand {
  static commandName = 'snmp:poll'
  static description =
    'Executa varredura e coleta SNMP em um dispositivo específico ou em todos os dispositivos'

  @args.string({ description: 'ID do dispositivo (opcional)', required: false })
  declare deviceId?: string

  @flags.string({ description: 'Comunidade SNMP (padrão: public)' })
  declare community?: string

  @flags.string({ description: 'Versão SNMP (v1, v2c, v3; padrão: v2c)' })
  declare version?: string

  static options: CommandOptions = {
    startApp: true,
  }

  private snmpService = new SnmpService()

  async run() {
    const community = this.community || 'public'
    const version = (this.version as 'v1' | 'v2c' | 'v3') || 'v2c'

    if (this.deviceId) {
      const id = Number.parseInt(this.deviceId, 10)
      if (Number.isNaN(id)) {
        this.logger.error('ID de dispositivo inválido.')
        return
      }

      const device = await Device.find(id)
      if (!device) {
        this.logger.error(`Dispositivo #${id} não encontrado.`)
        return
      }

      this.logger.info(
        `Iniciando varredura SNMP para o dispositivo #${device.id} (${device.name})...`
      )
      const res = await this.snmpService.pollDevice(device, {
        host: device.ipAddress || device.name,
        version,
        community,
      })

      this.logger.success(
        `Coleta concluída: ${res.interfaceCount} interfaces, ${res.metricCount} métricas, ${res.neighborCount} vizinhos.`
      )
    } else {
      const devices = await Device.all()
      this.logger.info(`Iniciando varredura SNMP para ${devices.length} dispositivo(s)...`)

      for (const device of devices) {
        try {
          const res = await this.snmpService.pollDevice(device, {
            host: device.ipAddress || device.name,
            version,
            community,
          })
          this.logger.info(
            `Dispositivo #${device.id} (${device.name}): ${res.interfaceCount} interfaces, ${res.metricCount} métricas, ${res.neighborCount} vizinhos.`
          )
        } catch (error) {
          this.logger.error(
            `Falha na varredura do dispositivo #${device.id}: ${error instanceof Error ? error.message : String(error)}`
          )
        }
      }
      this.logger.success('Varredura SNMP finalizada.')
    }
  }
}

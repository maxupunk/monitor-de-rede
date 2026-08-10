import { BaseCommand, flags } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import crypto from 'node:crypto'
import { DateTime } from 'luxon'
import Probe from '#models/probe'

export default class ProbeRegister extends BaseCommand {
  static commandName = 'probe:register'
  static description = 'Registra um novo agente probe e gera o token de autenticação'

  static options: CommandOptions = {
    startApp: true,
  }

  @flags.string({ description: 'Nome descritivo do probe', required: true })
  declare name: string

  @flags.number({ description: 'ID do site associado ao probe', required: false })
  declare siteId: number

  async run() {
    const rawToken = crypto.randomBytes(32).toString('hex')
    const tokenHash = crypto.createHash('sha256').update(rawToken).digest('hex')

    const probe = await Probe.create({
      name: this.name,
      siteId: this.siteId || null,
      tokenHash,
      status: 'pending',
      registeredAt: DateTime.now(),
    })

    this.logger.success(`Probe "${probe.name}" (ID #${probe.id}) registrado com sucesso!`)
    this.logger.info('----------------------------------------------------')
    this.logger.info(`PROBE_TOKEN: ${rawToken}`)
    this.logger.info('Guarde este token! Ele não será exibido novamente.')
    this.logger.info('----------------------------------------------------')
  }
}

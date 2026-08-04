import { BaseCommand } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import { VpnProbeRegistrar } from '#modules/vpn/vpn_probe_registrar'

export default class VpnProbeRegister extends BaseCommand {
  static commandName = 'vpn:probe-register'
  static description = 'Registra (ou atualiza) o probe dedicado da VPN WireGuard'

  static options: CommandOptions = {
    startApp: true,
  }

  async run() {
    const registrar = new VpnProbeRegistrar()
    const registration = await registrar.registerWithGeneratedToken()

    this.logger.success(
      `Probe "${registration.probe.name}" (ID #${registration.probe.id}) ${registration.created ? 'registrado' : 'atualizado'} com sucesso!`
    )

    if (registration.token) {
      this.logger.info('----------------------------------------------------')
      this.logger.info(`VPN_PROBE_TOKEN: ${registration.token}`)
      this.logger.info('Defina esse valor no .env / docker-compose do vpn-probe.')
      this.logger.info('Ele não será exibido novamente.')
      this.logger.info('----------------------------------------------------')
    } else {
      this.logger.info('Token obtido de VPN_PROBE_TOKEN (variável de ambiente).')
    }
  }
}

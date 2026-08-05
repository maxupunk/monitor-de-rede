/*
|--------------------------------------------------------------------------
| Registro automático do vpn-probe
|--------------------------------------------------------------------------
|
| Executado na inicialização do servidor HTTP. Quando `VPN_PROBE_TOKEN` está
| definido (docker-compose), o probe dedicado da VPN já sobe registrado e apto a
| receber tarefas de monitoramento pelo túnel WireGuard.
|
*/

import logger from '@adonisjs/core/services/logger'
import { VpnProbeRegistrar } from '#modules/vpn/vpn_probe_registrar'
import { errorMessage } from '#modules/shared/errors'

const registrar = new VpnProbeRegistrar()

try {
  const registration = await registrar.register()

  if (registration) {
    logger.info(
      `[VPN] probe "${registration.probe.name}" ${registration.created ? 'registrado' : 'atualizado'} (ID #${registration.probe.id})`
    )
  }
} catch (error: unknown) {
  // Falha aqui não pode impedir a API de subir (ex.: banco ainda migrando).
  const message = errorMessage(error)
  logger.warn(`[VPN] não foi possível registrar o vpn-probe automaticamente: ${message}`)
}

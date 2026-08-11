import { BaseCommand } from '@adonisjs/core/ace'
import type { CommandOptions } from '@adonisjs/core/types/ace'
import VpnServer from '#models/vpn_server'
import VpnPeer from '#models/vpn_peer'

/**
 * Exporta os segredos da VPN em texto claro para a migração da Fase 9.
 *
 * Existe por causa do desvio D6 do `docs/roadmap_backend_rust.md`: o backend
 * Rust cifra com XChaCha20-Poly1305 e não lê o formato do `encryption` do
 * AdonisJS. Só este processo sabe decifrar o que está no banco hoje, então o
 * único caminho é exportar em claro aqui e re-cifrar lá
 * (`backend_rust-cli task vpn_secrets_import`).
 *
 * ⚠️ A saída contém **a chave privada do servidor WireGuard e todas as chaves
 * pré-compartilhadas**. Redirecione para um arquivo em disco local, use-o
 * imediatamente e apague com `shred -u`. Nunca versione, nunca mande por chat.
 */
export default class VpnExportSecrets extends BaseCommand {
  static commandName = 'vpn:export-secrets'
  static description =
    'Exporta os segredos da VPN em claro para a migração ao backend Rust (Fase 9)'

  static options: CommandOptions = {
    startApp: true,
  }

  async run() {
    const servers = await VpnServer.all()
    const peers = await VpnPeer.all()

    const payload = {
      servers: servers.map((server) => ({
        id: server.id,
        privateKey: server.privateKey,
      })),
      peers: peers.map((peer) => ({
        id: peer.id,
        presharedKey: peer.presharedKey ?? null,
      })),
    }

    // `console.log` e não `this.logger`: a saída é dado, não mensagem — quem
    // chama redireciona para um arquivo.
    console.log(JSON.stringify(payload, null, 2))

    this.logger.warning(
      `Exportados ${payload.servers.length} servidor(es) e ${payload.peers.length} peer(s) EM TEXTO CLARO. Apague o arquivo assim que importar.`
    )
  }
}

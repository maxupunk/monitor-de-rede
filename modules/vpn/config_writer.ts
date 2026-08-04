import fs from 'node:fs'
import path from 'node:path'
import {
  WireGuardConfigBuilder,
  type PeerEntryInput,
  type ServerInterfaceInput,
} from './config_builder.js'

/**
 * Persistência do `wg0.conf` no volume compartilhado com o container WireGuard.
 *
 * O servidor **nunca** executa `docker exec`: escreve o arquivo e o watcher
 * dentro do container aplica com `wg syncconf`, sem derrubar túneis ativos
 * (ver `docs/roadmap_vpn.md` §2.4).
 */

/** Porta de saída — permite trocar disco por outro destino em testes. */
export interface VpnConfigSink {
  write(fileName: string, contents: string): Promise<void>
  read(fileName: string): Promise<string | null>
}

/** Diretório do volume `wg-config`; em Windows cai para uma pasta local. */
export function resolveConfigDir(): string {
  if (process.env.WG_CONFIG_DIR) return process.env.WG_CONFIG_DIR
  return process.platform === 'win32' ? path.join(process.cwd(), 'tmp', 'wireguard') : '/config'
}

export class FileConfigSink implements VpnConfigSink {
  constructor(private baseDir: string = resolveConfigDir()) {}

  private resolve(fileName: string): string {
    return path.join(this.baseDir, fileName)
  }

  async write(fileName: string, contents: string): Promise<void> {
    fs.mkdirSync(this.baseDir, { recursive: true })

    // Escrita atômica: evita que o watcher leia um arquivo pela metade.
    const target = this.resolve(fileName)
    const temporary = `${target}.tmp`
    await fs.promises.writeFile(temporary, contents, { mode: 0o600 })
    await fs.promises.rename(temporary, target)
  }

  async read(fileName: string): Promise<string | null> {
    try {
      return await fs.promises.readFile(this.resolve(fileName), 'utf-8')
    } catch {
      return null
    }
  }
}

export class ConfigWriter {
  private builder = new WireGuardConfigBuilder()

  constructor(private sink: VpnConfigSink = new FileConfigSink()) {}

  /** Escreve `<interface>.conf` e devolve o conteúdo aplicado. */
  async writeServerConfig(server: ServerInterfaceInput, peers: PeerEntryInput[]): Promise<string> {
    const contents = this.builder.build(server, peers)
    await this.sink.write(`${server.interfaceName}.conf`, contents)
    return contents
  }
}

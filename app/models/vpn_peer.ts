import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, computed } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import encryption from '@adonisjs/core/services/encryption'
import Device from '#models/device'
import VpnServer from '#models/vpn_server'

/** Perfis de equipamento suportados pelos geradores de configuração. */
export type VpnDeviceProfile = 'mikrotik' | 'openwrt' | 'linux' | 'windows' | 'mobile'

/** Estado do túnel derivado do último sinal de vida recebido do peer. */
export type VpnPeerConnectionStatus = 'connected' | 'unstable' | 'disconnected' | 'awaiting'

/**
 * `REJECT_AFTER_TIME` do WireGuard: o keypair atual deixa de ser aceito depois
 * disso. É o teto do intervalo entre handshakes de um túnel em uso — mas só de
 * um túnel *em uso*: sem dados para enviar, o protocolo não renegocia nada.
 */
export const REJECT_AFTER_SECONDS = 180

/**
 * Folga do caminho até o banco: o watcher republica `wg show dump` a cada 5s e
 * o scheduler sincroniza a cada 10s. Sem essa margem, um peer saudável cruzava
 * o limite só por causa da latência da coleta.
 */
export const STATUS_PIPELINE_SLACK_SECONDS = 45

/** Keepalives que podem faltar antes de o túnel virar suspeito. */
export const KEEPALIVE_MISSES_ALLOWED = 3

/** Janela usada quando não há keepalive contabilizado — resta olhar o handshake. */
export const HANDSHAKE_CONNECTED_SECONDS = REJECT_AFTER_SECONDS + STATUS_PIPELINE_SLACK_SECONDS

/** Acima disso o túnel é dado como caído, não apenas instável. */
export const HANDSHAKE_UNSTABLE_SECONDS = 900

/**
 * Peer WireGuard. Guarda apenas material criptográfico e telemetria — nome e IP
 * vêm do `Device` vinculado, e o CIDR vem da `Network` do servidor.
 *
 * A chave privada do cliente **nunca** é persistida (ver `docs/roadmap_vpn.md` §3.4).
 */
export default class VpnPeer extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare vpnServerId: number

  @column()
  declare deviceId: number

  @column()
  declare publicKey: string

  /** Chave simétrica pré-compartilhada: cifrada em repouso e não serializada. */
  @column({
    columnName: 'preshared_key_encrypted',
    serializeAs: null,
    prepare: (value: string | null) => (value ? encryption.encrypt(value) : value),
    consume: (value: string | null) => (value ? encryption.decrypt<string>(value) : value),
  })
  declare presharedKey: string | null

  @column()
  declare deviceProfile: VpnDeviceProfile

  @column()
  declare persistentKeepalive: number

  @column.dateTime()
  declare lastHandshakeAt: DateTime | null

  /**
   * Último ciclo em que o servidor contabilizou bytes novos vindos do peer.
   * Com `PersistentKeepalive` ativo isso acontece a cada 25s, o que torna esse
   * carimbo um sinal de vida muito mais fiel que o handshake.
   */
  @column.dateTime()
  declare lastSeenAt: DateTime | null

  // Postgres devolve colunas bigint como string para não perder precisão;
  // normaliza para number aqui para não vazar esse detalhe do driver à API/UI.
  @column({ consume: (value: string | number) => Number(value) })
  declare bytesRx: number

  @column({ consume: (value: string | number) => Number(value) })
  declare bytesTx: number

  @column()
  declare enabled: boolean

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => VpnServer)
  declare vpnServer: BelongsTo<typeof VpnServer>

  @belongsTo(() => Device)
  declare device: BelongsTo<typeof Device>

  /**
   * Sinal de vida mais recente: keepalive contabilizado ou renegociação de
   * chaves, o que tiver acontecido por último.
   */
  get lastActivityAt(): DateTime | null {
    if (!this.lastHandshakeAt) return this.lastSeenAt ?? null
    if (!this.lastSeenAt) return this.lastHandshakeAt
    return this.lastSeenAt > this.lastHandshakeAt ? this.lastSeenAt : this.lastHandshakeAt
  }

  /**
   * Quanto tempo sem sinal ainda conta como conectado.
   *
   * Com keepalive ativo a régua é o próprio keepalive: três perdas seguidas mais
   * a folga da coleta. Sem ele — peer que só fala quando tem tráfego — resta o
   * handshake, cuja janela precisa ser bem mais larga.
   */
  private get connectedWindowSeconds(): number {
    if (this.persistentKeepalive > 0 && this.lastSeenAt) {
      return this.persistentKeepalive * KEEPALIVE_MISSES_ALLOWED + STATUS_PIPELINE_SLACK_SECONDS
    }
    return HANDSHAKE_CONNECTED_SECONDS
  }

  @computed()
  get connectionStatus(): VpnPeerConnectionStatus {
    const lastActivity = this.lastActivityAt
    if (!lastActivity) return 'awaiting'

    const elapsedSeconds = DateTime.now().diff(lastActivity, 'seconds').seconds
    if (elapsedSeconds <= this.connectedWindowSeconds) return 'connected'
    if (elapsedSeconds <= HANDSHAKE_UNSTABLE_SECONDS) return 'unstable'
    return 'disconnected'
  }
}

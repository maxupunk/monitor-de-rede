import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, computed } from '@adonisjs/lucid/orm'
import type { BelongsTo } from '@adonisjs/lucid/types/relations'
import encryption from '@adonisjs/core/services/encryption'
import Device from '#models/device'
import VpnServer from '#models/vpn_server'

/** Perfis de equipamento suportados pelos geradores de configuração. */
export type VpnDeviceProfile = 'mikrotik' | 'openwrt' | 'linux' | 'windows' | 'mobile'

/** Estado do túnel derivado do último handshake. */
export type VpnPeerConnectionStatus = 'connected' | 'unstable' | 'disconnected' | 'awaiting'

export const HANDSHAKE_CONNECTED_SECONDS = 180
export const HANDSHAKE_UNSTABLE_SECONDS = 600

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

  @computed()
  get connectionStatus(): VpnPeerConnectionStatus {
    if (!this.lastHandshakeAt) return 'awaiting'

    const elapsedSeconds = DateTime.now().diff(this.lastHandshakeAt, 'seconds').seconds
    if (elapsedSeconds <= HANDSHAKE_CONNECTED_SECONDS) return 'connected'
    if (elapsedSeconds <= HANDSHAKE_UNSTABLE_SECONDS) return 'unstable'
    return 'disconnected'
  }
}

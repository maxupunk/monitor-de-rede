import { DateTime } from 'luxon'
import { BaseModel, column, belongsTo, hasMany } from '@adonisjs/lucid/orm'
import type { BelongsTo, HasMany } from '@adonisjs/lucid/types/relations'
import encryption from '@adonisjs/core/services/encryption'
import Network from '#models/network'
import VpnPeer from '#models/vpn_peer'

/**
 * Servidor WireGuard. O CIDR não é duplicado aqui: a fonte da verdade é
 * `networks.cidr` da rede vinculada.
 */
export default class VpnServer extends BaseModel {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare networkId: number

  @column()
  declare interfaceName: string

  @column()
  declare listenPort: number

  @column()
  declare publicEndpoint: string | null

  @column()
  declare publicKey: string

  /**
   * Chave privada do servidor: cifrada em repouso (APP_KEY) e nunca serializada
   * em respostas da API.
   */
  @column({
    columnName: 'private_key_encrypted',
    serializeAs: null,
    prepare: (value: string | null) => (value ? encryption.encrypt(value) : value),
    consume: (value: string | null) => (value ? encryption.decrypt<string>(value) : value),
  })
  declare privateKey: string

  @column()
  declare allowPeerToPeer: boolean

  @column()
  declare mtu: number

  @column()
  declare dnsServers: string | null

  @column()
  declare active: boolean

  @column.dateTime()
  declare lastSyncedAt: DateTime | null

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  @belongsTo(() => Network)
  declare network: BelongsTo<typeof Network>

  @hasMany(() => VpnPeer)
  declare peers: HasMany<typeof VpnPeer>
}

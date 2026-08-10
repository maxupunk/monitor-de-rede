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
 * `KEEPALIVE_TIMEOUT` do WireGuard: quem recebe um pacote e não tem nada a
 * devolver responde com um keepalive vazio depois desse tempo.
 */
export const KEEPALIVE_TIMEOUT_SECONDS = 10

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

/**
 * Sem keepalive o silêncio é ambíguo: um túnel ocioso é indistinguível de um
 * túnel morto, porque o WireGuard só renegocia quando há o que enviar. Por isso
 * a janela até "caído" é generosa aqui — é o mesmo valor adotado pelo wg-easy.
 */
export const HANDSHAKE_DISCONNECTED_SECONDS = 600

/**
 * Cadência real com que o RX cresce num peer com keepalive.
 *
 * Não é o `PersistentKeepalive` puro: o valor no dump é o intervalo do
 * *servidor*, e o que faz o RX subir é a resposta do peer — um keepalive
 * passivo emitido `KEEPALIVE_TIMEOUT` depois. Usar só o keepalive subestimava
 * o intervalo e a janela de "conectado" valia ~2,4 perdas em vez de 3.
 */
export function effectiveKeepaliveSeconds(persistentKeepalive: number): number {
  return persistentKeepalive + KEEPALIVE_TIMEOUT_SECONDS
}

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

  /**
   * Estado apurado no ciclo anterior de sincronização.
   *
   * `connectionStatus` é derivado da telemetria a cada leitura e por isso não
   * guarda história: sem este carimbo não há como distinguir "o túnel está
   * caído" de "o túnel *acabou de* cair" — e é a segunda leitura que vira alerta.
   */
  @column()
  declare lastConnectionStatus: VpnPeerConnectionStatus | null

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
  /** Há keepalive contabilizado, ou só resta o handshake como régua? */
  private get hasKeepaliveHeartbeat(): boolean {
    return this.persistentKeepalive > 0 && this.lastSeenAt !== null
  }

  private get connectedWindowSeconds(): number {
    if (this.hasKeepaliveHeartbeat) {
      return (
        effectiveKeepaliveSeconds(this.persistentKeepalive) * KEEPALIVE_MISSES_ALLOWED +
        STATUS_PIPELINE_SLACK_SECONDS
      )
    }
    return HANDSHAKE_CONNECTED_SECONDS
  }

  /**
   * Onde "instável" acaba e "caído" começa.
   *
   * Com keepalive existe batimento previsível, então o dobro da janela de
   * conectado já é diagnóstico — não há motivo para esperar 15 minutos. Sem
   * keepalive o silêncio não prova nada e a régua tem que ser generosa.
   */
  private get disconnectedWindowSeconds(): number {
    if (this.hasKeepaliveHeartbeat) return this.connectedWindowSeconds * 2
    return HANDSHAKE_DISCONNECTED_SECONDS
  }

  /**
   * Janela curta para afirmar que o túnel está de pé **agora**.
   *
   * `connectionStatus` é tolerante de propósito, para o status não piscar com
   * uma perda isolada de keepalive. Mas um diagnóstico como "o túnel está de
   * pé, o bloqueio é do ICMP" precisa de prova recente, não de tolerância: um
   * único batimento perdido já basta para calar o aviso.
   */
  private get proofOfLifeWindowSeconds(): number {
    if (this.hasKeepaliveHeartbeat) {
      return effectiveKeepaliveSeconds(this.persistentKeepalive) + STATUS_PIPELINE_SLACK_SECONDS
    }
    return HANDSHAKE_CONNECTED_SECONDS
  }

  /** O túnel deu sinal de vida recente o bastante para sustentar um diagnóstico. */
  get hasFreshProofOfLife(): boolean {
    const lastActivity = this.lastActivityAt
    if (!lastActivity) return false

    return DateTime.now().diff(lastActivity, 'seconds').seconds <= this.proofOfLifeWindowSeconds
  }

  @computed()
  get connectionStatus(): VpnPeerConnectionStatus {
    const lastActivity = this.lastActivityAt
    if (!lastActivity) return 'awaiting'

    const elapsedSeconds = DateTime.now().diff(lastActivity, 'seconds').seconds
    if (elapsedSeconds <= this.connectedWindowSeconds) return 'connected'
    if (elapsedSeconds <= this.disconnectedWindowSeconds) return 'unstable'
    return 'disconnected'
  }
}

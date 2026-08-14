import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { ArtifactSummaryItem } from '@/bindings/ArtifactSummaryItem'
import type { ArtifactVariant } from '@/bindings/ArtifactVariant'
import type { PreflightResult } from '@/bindings/PreflightResult'
import type { ProfileCard } from '@/bindings/ProfileCard'
import type { SerializedVpnArtifact } from '@/bindings/SerializedVpnArtifact'
import type { VpnPeerConnectionStatus } from '@/bindings/VpnPeerConnectionStatus'
import type { VpnPeerDeviceView } from '@/bindings/VpnPeerDeviceView'
import type { VpnPeerListItem } from '@/bindings/VpnPeerListItem'
import type { VpnPeerWithDevice } from '@/bindings/VpnPeerWithDevice'
import type { VpnServerResponse } from '@/bindings/VpnServerResponse'
import type { VpnServerStateResponse } from '@/bindings/VpnServerStateResponse'

/**
 * Os tipos da VPN são gerados pelo `ts-rs` a partir dos structs Rust
 * (`backend/src/views/vpn.rs` e `services/vpn/`) — desvio D7 / ajuste F7
 * do roadmap do backend Rust. Aqui ficam só os **apelidos** com os nomes que as
 * telas já usam, para que trocar um campo no backend quebre o `vue-tsc` em vez
 * da tela em produção.
 *
 * O que continua escrito à mão está marcado caso a caso, sempre com o motivo.
 */
export type VpnConnectionStatus = VpnPeerConnectionStatus
export type VpnProfileOption = ProfileCard
export type VpnServer = VpnServerResponse
export type VpnServerState = VpnServerStateResponse
export type VpnPeerDevice = VpnPeerDeviceView
export type VpnPeer = VpnPeerListItem
export type VpnPeerRenamed = VpnPeerWithDevice
export type VpnArtifactSummaryItem = ArtifactSummaryItem
export type VpnArtifactVariant = ArtifactVariant
export type VpnArtifact = SerializedVpnArtifact
export type VpnPreflightResult = PreflightResult

/**
 * Perfis com rótulo e ícone próprios nesta interface.
 *
 * **Não** é o contrato: quem decide os perfis aceitos é o registro do backend
 * (`services/vpn/profiles/registry.rs`), e por isso `deviceProfile` chega como
 * `string`. Esta união existe só para tipar as tabelas de apresentação abaixo —
 * um perfil novo no backend aparece no wizard sozinho e cai no rótulo padrão
 * até ganhar um ícone aqui.
 */
export type VpnDeviceProfile = 'mikrotik' | 'openwrt' | 'linux' | 'windows' | 'mobile'

/**
 * Corpo de `POST /api/vpn/peers`, escrito à mão de propósito: no Rust todo
 * campo do `CreatePeerInput` é opcional (a validação e a mensagem em português
 * ficam no controller), e gerar o binding daria um tipo em que `name` e
 * `profile` seriam opcionais — o oposto do que o wizard precisa exigir.
 */
export interface CreateVpnPeerPayload {
  name: string
  profile: string
  ipAddress?: string | null
  siteId?: number | null
  snmpEnabled?: boolean
  snmpCommunity?: string | null
  description?: string | null
}

/** Fonte única de rótulo/ícone por perfil — reutilizada em qualquer tela que liste peers da VPN. */
export const VPN_PROFILE_LABELS: Record<VpnDeviceProfile, string> = {
  mikrotik: 'MikroTik',
  openwrt: 'OpenWrt',
  linux: 'Linux',
  windows: 'Windows',
  mobile: 'Celular',
}

export const VPN_PROFILE_ICONS: Record<VpnDeviceProfile, string> = {
  mikrotik: 'mdi-router-network',
  openwrt: 'mdi-router-wireless',
  linux: 'mdi-linux',
  windows: 'mdi-microsoft-windows',
  mobile: 'mdi-cellphone',
}

export const VPN_STATUS_LABELS: Record<VpnConnectionStatus, string> = {
  connected: 'Conectado',
  unstable: 'Instável',
  disconnected: 'Desconectado',
  awaiting: 'Aguardando',
}

export const VPN_STATUS_COLORS: Record<VpnConnectionStatus, string> = {
  connected: 'success',
  unstable: 'warning',
  disconnected: 'error',
  awaiting: 'grey',
}

/**
 * Recebem `string`, e não a união: o perfil vem do registro do backend, então
 * um perfil ainda sem rótulo aqui precisa cair no padrão — que é o que estas
 * funções sempre fizeram em runtime.
 */
export function vpnProfileLabel(profile: string): string {
  return VPN_PROFILE_LABELS[profile as VpnDeviceProfile] || profile
}

export function vpnProfileIcon(profile: string): string {
  return VPN_PROFILE_ICONS[profile as VpnDeviceProfile] || 'mdi-devices'
}

export function vpnStatusLabel(status: VpnConnectionStatus): string {
  return VPN_STATUS_LABELS[status] || status
}

export function vpnStatusColor(status: VpnConnectionStatus): string {
  return VPN_STATUS_COLORS[status] || 'grey'
}

export const useVpnStore = defineStore('vpn', () => {
  const state = ref<VpnServerState | null>(null)
  const peers = ref<VpnPeer[]>([])
  const preflight = ref<VpnPreflightResult | null>(null)
  const lastArtifact = ref<VpnArtifact | null>(null)

  const loading = ref(false)
  const saving = ref(false)
  const testing = ref(false)
  const error = ref<string | null>(null)

  const isConfigured = computed(() => state.value?.configured === true)
  const profiles = computed<VpnProfileOption[]>(() => state.value?.profiles ?? [])

  function fail(err: unknown, fallback: string): null {
    error.value = err instanceof Error ? err.message : fallback
    return null
  }

  /**
   * Aplica o snapshot de tráfego/handshake publicado pelo scheduler
   * (`vpn:peers_updated`), mantendo a tela viva sem recarregamento.
   */
  function applyRealtimePeers(data: Record<string, unknown>) {
    const incoming = (data.peers as Array<Record<string, unknown>>) || []
    if (incoming.length === 0) return

    for (const update of incoming) {
      const peer = peers.value.find((p) => p.id === Number(update.id))
      if (!peer) continue

      peer.connectionStatus = update.connectionStatus as VpnConnectionStatus
      peer.lastHandshakeAt = (update.lastHandshakeAt as string | null) ?? null
      peer.lastSeenAt = (update.lastSeenAt as string | null) ?? null
      peer.bytesRx = Number(update.bytesRx ?? peer.bytesRx)
      peer.bytesTx = Number(update.bytesTx ?? peer.bytesTx)
      peer.needsFirewallHint = Boolean(update.needsFirewallHint)
      peer.pingOutsideTunnel = Boolean(update.pingOutsideTunnel)
      peer.pingMonitorId = (update.pingMonitorId as number | null) ?? null
    }
  }

  async function fetchServer() {
    loading.value = true
    error.value = null
    try {
      state.value = await apiService.get<VpnServerState>('/vpn/server')
    } catch (err: unknown) {
      fail(err, 'Erro ao carregar o servidor VPN')
    } finally {
      loading.value = false
    }
  }

  async function saveServer(payload: Record<string, unknown>): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      await apiService.put('/vpn/server', payload)
      await fetchServer()
      return true
    } catch (err: unknown) {
      fail(err, 'Erro ao salvar a configuração do servidor VPN')
      return false
    } finally {
      saving.value = false
    }
  }

  async function runPreflight(): Promise<VpnPreflightResult | null> {
    testing.value = true
    error.value = null
    try {
      preflight.value = await apiService.post<VpnPreflightResult>('/vpn/server/preflight', {
        publicEndpoint: state.value?.server?.publicEndpoint ?? null,
        listenPort: state.value?.server?.listenPort ?? 51820,
      })
      return preflight.value
    } catch (err: unknown) {
      return fail(err, 'Erro ao executar o teste de acessibilidade')
    } finally {
      testing.value = false
    }
  }

  async function detectEndpoint(): Promise<string | null> {
    testing.value = true
    error.value = null
    try {
      const result = await apiService.post<{ detected: boolean; publicEndpoint: string | null }>(
        '/vpn/server/detect-endpoint'
      )
      return result.publicEndpoint
    } catch (err: unknown) {
      return fail(err, 'Erro ao detectar o endereço público')
    } finally {
      testing.value = false
    }
  }

  async function fetchPeers() {
    loading.value = true
    error.value = null
    try {
      peers.value = await apiService.get<VpnPeer[]>('/vpn/peers')
    } catch (err: unknown) {
      fail(err, 'Erro ao carregar os dispositivos VPN')
    } finally {
      loading.value = false
    }
  }

  async function suggestNextIp(): Promise<string | null> {
    try {
      const result = await apiService.get<{ ipAddress: string }>('/vpn/peers/next-ip')
      return result.ipAddress
    } catch (err: unknown) {
      return fail(err, 'Erro ao sugerir o próximo IP livre')
    }
  }

  async function createPeer(payload: CreateVpnPeerPayload): Promise<VpnArtifact | null> {
    saving.value = true
    error.value = null
    try {
      const result = await apiService.post<{ artifact: VpnArtifact }>('/vpn/peers', payload)
      lastArtifact.value = result.artifact
      await fetchPeers()
      return result.artifact
    } catch (err: unknown) {
      return fail(err, 'Erro ao criar o dispositivo VPN')
    } finally {
      saving.value = false
    }
  }

  /** Renomeia o dispositivo do peer, mantendo a linha da tabela sincronizada. */
  async function renamePeer(peerId: number, name: string): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      const updated = await apiService.patch<VpnPeerRenamed>(`/vpn/peers/${peerId}`, { name })
      const peer = peers.value.find((item) => item.id === peerId)
      if (peer && updated.device) peer.device = updated.device
      return true
    } catch (err: unknown) {
      fail(err, 'Erro ao renomear o dispositivo VPN')
      return false
    } finally {
      saving.value = false
    }
  }

  async function fetchConfig(peerId: number): Promise<VpnArtifact | null> {
    error.value = null
    try {
      const artifact = await apiService.get<VpnArtifact>(`/vpn/peers/${peerId}/config`)
      lastArtifact.value = artifact
      return artifact
    } catch (err: unknown) {
      return fail(err, 'Erro ao obter a configuração do dispositivo')
    }
  }

  async function rotateKeys(peerId: number): Promise<VpnArtifact | null> {
    saving.value = true
    error.value = null
    try {
      const result = await apiService.post<{ artifact: VpnArtifact }>(`/vpn/peers/${peerId}/rotate`)
      lastArtifact.value = result.artifact
      await fetchPeers()
      return result.artifact
    } catch (err: unknown) {
      return fail(err, 'Erro ao rotacionar as chaves')
    } finally {
      saving.value = false
    }
  }

  async function fetchFirewallHints(peerId: number): Promise<string | null> {
    error.value = null
    try {
      const result = await apiService.post<{ content: string }>(
        `/vpn/peers/${peerId}/firewall-hints`
      )
      return result.content
    } catch (err: unknown) {
      return fail(err, 'Erro ao obter as regras de firewall')
    }
  }

  async function revokePeer(peerId: number): Promise<boolean> {
    error.value = null
    try {
      await apiService.delete(`/vpn/peers/${peerId}`)
      peers.value = peers.value.filter((peer) => peer.id !== peerId)
      return true
    } catch (err: unknown) {
      fail(err, 'Erro ao revogar o dispositivo VPN')
      return false
    }
  }

  return {
    state,
    peers,
    preflight,
    lastArtifact,
    loading,
    saving,
    testing,
    error,
    isConfigured,
    profiles,
    fetchServer,
    saveServer,
    runPreflight,
    detectEndpoint,
    fetchPeers,
    suggestNextIp,
    createPeer,
    renamePeer,
    fetchConfig,
    rotateKeys,
    fetchFirewallHints,
    revokePeer,
    applyRealtimePeers,
  }
})

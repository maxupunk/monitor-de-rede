import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'

export type VpnDeviceProfile = 'mikrotik' | 'openwrt' | 'linux' | 'windows' | 'mobile'
export type VpnConnectionStatus = 'connected' | 'unstable' | 'disconnected' | 'awaiting'

export interface VpnProfileOption {
  profile: VpnDeviceProfile
  label: string
  icon: string
  supportsQrCode: boolean
}

export interface VpnServer {
  id: number
  networkId: number
  interfaceName: string
  listenPort: number
  publicEndpoint: string | null
  publicKey: string
  allowPeerToPeer: boolean
  mtu: number
  dnsServers: string | null
  active: boolean
  lastSyncedAt: string | null
}

export interface VpnServerState {
  configured: boolean
  server: VpnServer | null
  cidr: string | null
  serverAddress: string | null
  peersTotal: number
  peersConnected: number
  bytesRx: number
  bytesTx: number
  persistentKeepalive: number
  profiles: VpnProfileOption[]
}

export interface VpnPeerDevice {
  id: number
  name: string
  ipAddress: string | null
  snmpEnabled: boolean
  status: string
}

export interface VpnPeer {
  id: number
  vpnServerId: number
  deviceId: number
  publicKey: string
  deviceProfile: VpnDeviceProfile
  persistentKeepalive: number
  lastHandshakeAt: string | null
  bytesRx: number
  bytesTx: number
  enabled: boolean
  connectionStatus: VpnConnectionStatus
  needsFirewallHint: boolean
  device: VpnPeerDevice | null
}

export interface VpnArtifact {
  profile: VpnDeviceProfile
  label: string
  delivery: 'copy' | 'download' | 'qrcode'
  fileName: string
  language: string
  content: string
  instructions: string[]
  supportsQrCode: boolean
}

export interface VpnPreflightResult {
  status: 'reachable' | 'port_forward_required' | 'cgnat' | 'unknown'
  level: 'success' | 'warning' | 'error'
  message: string
  recommendation: string
  publicIp: string | null
  resolvedIp: string | null
  port: number
  isCgnat: boolean
  behindNat: boolean
  verified: boolean
}

export interface CreateVpnPeerPayload {
  name: string
  profile: VpnDeviceProfile
  ipAddress?: string | null
  siteId?: number | null
  snmpEnabled?: boolean
  snmpCommunity?: string | null
  description?: string | null
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

  async function fetchQrCode(peerId: number): Promise<string | null> {
    error.value = null
    try {
      const result = await apiService.get<{ svg: string }>(`/vpn/peers/${peerId}/qrcode`)
      return result.svg
    } catch (err: unknown) {
      return fail(err, 'Erro ao gerar o QR Code')
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
    fetchConfig,
    fetchQrCode,
    rotateKeys,
    fetchFirewallHints,
    revokePeer,
  }
})

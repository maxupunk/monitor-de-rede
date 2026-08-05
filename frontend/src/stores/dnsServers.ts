import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { DnsProtocol } from '@/utils/monitorTypes'

export interface DnsServer {
  id: number
  name: string
  /** IP, `ip:porta` (UDP/TCP) ou endpoint https (DoH) */
  address: string
  protocol: Exclude<DnsProtocol, 'system'>
  /** Participa da comparação de latência do dashboard */
  isDefault: boolean
  description: string | null
  createdAt?: string
  updatedAt?: string
}

export type DnsServerPayload = Omit<DnsServer, 'id' | 'createdAt' | 'updatedAt'>

export const useDnsServersStore = defineStore('dnsServers', () => {
  const servers = ref<DnsServer[]>([])
  const loading = ref(false)
  const saving = ref(false)
  const error = ref<string | null>(null)
  const loaded = ref(false)

  const benchmarkServers = computed(() => servers.value.filter((server) => server.isDefault))

  function findByAddress(address: string, protocol?: DnsProtocol): DnsServer | undefined {
    const normalized = address.trim().toLowerCase()
    return servers.value.find(
      (server) =>
        server.address.toLowerCase() === normalized &&
        (protocol === undefined || server.protocol === protocol)
    )
  }

  async function fetchServers(force = false): Promise<boolean> {
    if (loaded.value && !force) return true
    loading.value = true
    error.value = null
    try {
      servers.value = await apiService.get<DnsServer[]>('/dns/servers')
      loaded.value = true
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar os servidores DNS'
      return false
    } finally {
      loading.value = false
    }
  }

  async function createServer(payload: DnsServerPayload): Promise<DnsServer | null> {
    saving.value = true
    error.value = null
    try {
      const created = await apiService.post<DnsServer>('/dns/servers', payload)
      servers.value.push(created)
      return created
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao cadastrar o servidor DNS'
      return null
    } finally {
      saving.value = false
    }
  }

  async function updateServer(
    id: number,
    payload: Partial<DnsServerPayload>
  ): Promise<DnsServer | null> {
    saving.value = true
    error.value = null
    try {
      const updated = await apiService.put<DnsServer>(`/dns/servers/${id}`, payload)
      const index = servers.value.findIndex((server) => server.id === id)
      if (index !== -1) servers.value[index] = updated
      return updated
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar o servidor DNS'
      return null
    } finally {
      saving.value = false
    }
  }

  async function deleteServer(id: number): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      await apiService.delete(`/dns/servers/${id}`)
      servers.value = servers.value.filter((server) => server.id !== id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir o servidor DNS'
      return false
    } finally {
      saving.value = false
    }
  }

  /**
   * Cadastra o endereço digitado no formulário de monitores quando ele ainda
   * não existe — evita que o usuário precise sair da tela para registrar.
   */
  async function ensureServer(
    address: string,
    protocol: Exclude<DnsProtocol, 'system'>,
    name?: string
  ): Promise<DnsServer | null> {
    const trimmed = address.trim()
    if (!trimmed) return null

    const existing = findByAddress(trimmed, protocol)
    if (existing) return existing

    return createServer({
      name: name?.trim() || trimmed,
      address: trimmed,
      protocol,
      isDefault: true,
      description: null,
    })
  }

  return {
    servers,
    loading,
    saving,
    error,
    loaded,
    benchmarkServers,
    findByAddress,
    fetchServers,
    createServer,
    updateServer,
    deleteServer,
    ensureServer,
  }
})

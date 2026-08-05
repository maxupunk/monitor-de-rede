import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { useCrudResource } from './crudResource'
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
  const resource = useCrudResource<DnsServer>('/dns/servers', {
    fetch: 'Erro ao carregar os servidores DNS',
    create: 'Erro ao cadastrar o servidor DNS',
    update: 'Erro ao atualizar o servidor DNS',
    delete: 'Erro ao excluir o servidor DNS',
  })
  const servers = resource.items
  const saving = ref(false)
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
    const ok = await resource.fetchAll()
    if (ok) loaded.value = true
    return ok
  }

  async function createServer(payload: DnsServerPayload): Promise<DnsServer | null> {
    saving.value = true
    try {
      return await resource.create(payload)
    } finally {
      saving.value = false
    }
  }

  async function updateServer(
    id: number,
    payload: Partial<DnsServerPayload>
  ): Promise<DnsServer | null> {
    saving.value = true
    try {
      return await resource.update(id, payload)
    } finally {
      saving.value = false
    }
  }

  async function deleteServer(id: number): Promise<boolean> {
    saving.value = true
    try {
      return await resource.remove(id)
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
    loading: resource.loading,
    saving,
    error: resource.error,
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

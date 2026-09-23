import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { AgentCreateInput } from '@/bindings/AgentCreateInput'
import type { AgentEnrollmentView } from '@/bindings/AgentEnrollmentView'
import type { AgentInstallCommands } from '@/bindings/AgentInstallCommands'
import type { AgentUpdateInput } from '@/bindings/AgentUpdateInput'
import type { AgentView } from '@/bindings/AgentView'
import type { AgentWithEnrollment } from '@/bindings/AgentWithEnrollment'

/**
 * Agentes remotos (ADR 011): cadastro, códigos de instalação e estado vivo.
 * A lista é carregada ao abrir a tela; online/offline chega pelo SSE
 * (`probe:status`), sem polling.
 */
export const useAgentsStore = defineStore('agents', () => {
  const agents = ref<AgentView[]>([])
  const loading = ref(false)
  const saving = ref(false)
  const error = ref<string | null>(null)
  const loaded = ref(false)

  function fail(reason: unknown, fallback: string): void {
    error.value = reason instanceof Error ? reason.message : fallback
  }

  function upsert(agent: AgentView): void {
    const index = agents.value.findIndex((item) => item.id === agent.id)
    if (index === -1) {
      agents.value = [...agents.value, agent].sort((a, b) => a.name.localeCompare(b.name))
    } else {
      agents.value.splice(index, 1, agent)
    }
  }

  async function fetchAgents(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      agents.value = await apiService.get<AgentView[]>('/agents')
      loaded.value = true
    } catch (reason: unknown) {
      fail(reason, 'Erro ao carregar os servidores remotos')
    } finally {
      loading.value = false
    }
  }

  async function createAgent(input: AgentCreateInput): Promise<AgentWithEnrollment | null> {
    saving.value = true
    error.value = null
    try {
      const created = await apiService.post<AgentWithEnrollment>('/agents', input)
      upsert(created.agent)
      return created
    } catch (reason: unknown) {
      fail(reason, 'Erro ao conectar o servidor')
      return null
    } finally {
      saving.value = false
    }
  }

  async function updateAgent(id: number, input: AgentUpdateInput): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      upsert(await apiService.put<AgentView>(`/agents/${id}`, input))
      return true
    } catch (reason: unknown) {
      fail(reason, 'Erro ao alterar o servidor')
      return false
    } finally {
      saving.value = false
    }
  }

  async function reissueEnrollment(id: number): Promise<AgentEnrollmentView | null> {
    saving.value = true
    error.value = null
    try {
      return await apiService.post<AgentEnrollmentView>(`/agents/${id}/enrollment`)
    } catch (reason: unknown) {
      fail(reason, 'Erro ao gerar o código de instalação')
      return null
    } finally {
      saving.value = false
    }
  }

  /** Os mesmos comandos de instalação, para outro endereço desta central. */
  async function installCommands(
    code: string,
    addressId: string
  ): Promise<AgentInstallCommands | null> {
    error.value = null
    try {
      return await apiService.post<AgentInstallCommands>('/agents/install-commands', {
        code,
        addressId,
      })
    } catch (reason: unknown) {
      fail(reason, 'Erro ao gerar os comandos para este endereço')
      return null
    }
  }

  async function revokeAgent(id: number): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      upsert(await apiService.post<AgentView>(`/agents/${id}/revoke`))
      return true
    } catch (reason: unknown) {
      fail(reason, 'Erro ao revogar o acesso do servidor')
      return false
    } finally {
      saving.value = false
    }
  }

  async function removeAgent(id: number): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      await apiService.delete(`/agents/${id}`)
      agents.value = agents.value.filter((agent) => agent.id !== id)
      return true
    } catch (reason: unknown) {
      fail(reason, 'Erro ao remover o servidor')
      return false
    } finally {
      saving.value = false
    }
  }

  /** Aplica `probe:status`: conexão e última atividade sem nova consulta. */
  function applyRealtimeStatus(data: Record<string, unknown>): void {
    const id = Number(data.id ?? data.probeId)
    const agent = agents.value.find((item) => item.id === id)
    if (!agent) return
    if (typeof data.status === 'string') {
      agent.status = data.status
      agent.connected = data.status === 'online'
    }
    if (typeof data.version === 'string') agent.version = data.version
    if (typeof data.lastSeenAt === 'string') agent.lastSeenAt = data.lastSeenAt
  }

  return {
    agents,
    loading,
    saving,
    error,
    loaded,
    fetchAgents,
    createAgent,
    updateAgent,
    reissueEnrollment,
    installCommands,
    revokeAgent,
    removeAgent,
    applyRealtimeStatus,
  }
})

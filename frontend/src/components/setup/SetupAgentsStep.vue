<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">Servidores e Docker</div>
      <div class="text-body-2">
        Cada servidor é acompanhado por um agente que só disca para esta central. O desta máquina já
        vem com a instalação; para os outros, gere o comando abaixo e rode no servidor.
      </div>
    </div>

    <v-sheet border rounded="lg" class="pa-4 mb-4 bg-surface d-flex align-center ga-3">
      <v-avatar color="info" size="40" rounded="lg" variant="tonal">
        <v-icon size="22">mdi-server</v-icon>
      </v-avatar>
      <div class="flex-grow-1 min-w-0">
        <div class="font-weight-bold text-subtitle-1">Agente local</div>
        <div class="text-body-2">{{ localDescription }}</div>
      </div>
      <v-progress-circular
        v-if="agentsStore.loading"
        indeterminate
        size="22"
        color="primary"
      ></v-progress-circular>
      <v-chip
        v-else-if="localAgent"
        :color="localAgent.connected ? 'success' : 'warning'"
        size="small"
        variant="tonal"
        class="font-weight-bold"
      >
        {{ localAgent.connected ? 'Conectado' : 'Aguardando conexão' }}
      </v-chip>
    </v-sheet>

    <v-sheet border rounded="lg" class="pa-4 bg-surface">
      <div class="d-flex align-center ga-3 mb-3">
        <v-avatar color="primary" size="40" rounded="lg" variant="tonal">
          <v-icon size="22">mdi-server-plus</v-icon>
        </v-avatar>
        <div>
          <div class="font-weight-bold text-subtitle-1">Conectar outro servidor</div>
          <div class="text-body-2">
            Dê um nome ao servidor e gere o comando de instalação (vale uma vez).
          </div>
        </div>
      </div>
      <div class="d-flex align-center ga-2 flex-wrap">
        <v-text-field
          v-model="name"
          label="Nome do servidor"
          placeholder="Ex: srv-aplicacoes"
          variant="outlined"
          density="compact"
          prepend-inner-icon="mdi-server"
          hide-details
          class="flex-grow-1"
          @keyup.enter="connect"
        ></v-text-field>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-console"
          :loading="agentsStore.saving"
          :disabled="!name.trim()"
          @click="connect"
        >
          Gerar comando
        </v-btn>
      </div>
      <v-alert v-if="agentsStore.error" type="error" variant="tonal" density="compact" class="mt-3">
        {{ agentsStore.error }}
      </v-alert>
      <div v-if="remoteCount > 0" class="text-body-2 mt-3">
        {{ remoteCount }} servidor(es) remoto(s) já cadastrado(s) em
        <strong>Servidores › Servidores remotos</strong>.
      </div>
    </v-sheet>

    <AgentEnrollmentDialog
      v-model="enrollmentOpen"
      :enrollment="enrollment"
      :agent-name="enrollmentAgent"
      :agent-id="enrollmentAgentId"
    ></AgentEnrollmentDialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import AgentEnrollmentDialog from '@/components/agents/AgentEnrollmentDialog.vue'
import { useAgentsStore } from '@/stores/agents'
import type { AgentEnrollmentView } from '@/bindings/AgentEnrollmentView'

const agentsStore = useAgentsStore()

const name = ref('')
const enrollmentOpen = ref(false)
const enrollment = ref<AgentEnrollmentView | null>(null)
const enrollmentAgent = ref('')
const enrollmentAgentId = ref<number | null>(null)

const localAgent = computed(() => agentsStore.agents.find((agent) => agent.local) ?? null)
const remoteCount = computed(() => agentsStore.agents.filter((agent) => !agent.local).length)

const localDescription = computed(() => {
  if (agentsStore.loading) return 'Procurando o agente desta máquina…'
  const agent = localAgent.value
  if (!agent) {
    return 'Não encontrado. Ele sobe com o serviço netmonitor-agent do docker compose, ao lado da central.'
  }
  const host = [agent.host.hostname, agent.host.os].filter(Boolean).join(' · ')
  return agent.connected
    ? `Docker desta máquina já está visível${host ? ` (${host})` : ''}.`
    : 'Registrado, mas ainda não conectou. Confira se o serviço netmonitor-agent está rodando.'
})

async function connect() {
  const trimmed = name.value.trim()
  if (!trimmed) return
  const created = await agentsStore.createAgent({
    name: trimmed,
    siteId: null,
    deviceId: null,
    enforceTunnelIp: null,
  })
  if (!created) return
  name.value = ''
  enrollment.value = created.enrollment
  enrollmentAgent.value = created.agent.name
  enrollmentAgentId.value = created.agent.id
  enrollmentOpen.value = true
}

// Lista carregada uma vez ao abrir a etapa; conectado/desconectado chega pelo SSE.
onMounted(() => {
  if (!agentsStore.loaded) void agentsStore.fetchAgents()
})
</script>

<style scoped>
.min-w-0 {
  min-width: 0;
}
</style>

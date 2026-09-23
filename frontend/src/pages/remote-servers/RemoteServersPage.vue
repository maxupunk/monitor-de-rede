<template>
  <div>
    <PageHeader
      title="Servidores remotos"
      subtitle="Servidores conectados à central: Docker, métricas do host e monitores do site"
    >
      <template #actions>
        <v-btn
          variant="tonal"
          prepend-icon="mdi-refresh"
          :loading="agentsStore.loading"
          @click="agentsStore.fetchAgents()"
        >
          Atualizar
        </v-btn>
        <v-btn
          v-if="auth.isAdmin"
          color="primary"
          prepend-icon="mdi-plus"
          class="ml-2"
          @click="openCreate"
        >
          Conectar servidor
        </v-btn>
      </template>
    </PageHeader>

    <v-alert
      v-if="agentsStore.error"
      type="error"
      variant="tonal"
      closable
      class="mb-4"
      @click:close="agentsStore.error = null"
    >
      {{ agentsStore.error }}
    </v-alert>

    <v-skeleton-loader
      v-if="agentsStore.loading && agentsStore.agents.length === 0"
      type="card, card"
    ></v-skeleton-loader>

    <v-card
      v-else-if="agentsStore.agents.length === 0"
      rounded="xl"
      variant="outlined"
      class="pa-8 text-center"
    >
      <v-icon size="48" color="primary">mdi-server</v-icon>
      <div class="text-h6 mt-3">Nenhum servidor remoto conectado</div>
      <p class="text-body-2 text-medium-emphasis mt-2 mb-0">
        Conecte um servidor aqui ou marque "Conectar este servidor à central" ao criar um peer Linux
        na VPN: o script do túnel já faz a instalação.
      </p>
    </v-card>

    <v-row v-else dense>
      <v-col v-for="agent in agentsStore.agents" :key="agent.id" cols="12" lg="6">
        <v-card rounded="xl" variant="outlined" class="h-100">
          <v-card-title class="d-flex align-center ga-2 flex-wrap">
            <v-icon :color="statusMeta(agent).color">mdi-server</v-icon>
            <span class="text-truncate">{{ agent.name }}</span>
            <v-spacer></v-spacer>
            <v-chip size="small" variant="tonal" :color="statusMeta(agent).color">
              {{ statusMeta(agent).label }}
            </v-chip>
          </v-card-title>
          <v-card-subtitle>
            <span v-if="agent.host.hostname">{{ agent.host.hostname }} · </span>
            <span v-if="agent.host.os">{{ agent.host.os }} · </span>
            <span v-if="agent.host.arch">{{ agent.host.arch }}</span>
            <span v-if="!agent.host.hostname">Ainda não se conectou</span>
          </v-card-subtitle>
          <v-card-text>
            <v-list density="compact" class="bg-transparent pa-0">
              <v-list-item
                prepend-icon="mdi-docker"
                :title="agent.host.docker.available ? 'Docker disponível' : 'Docker indisponível'"
                :subtitle="agent.host.docker.version || agent.host.docker.reason || '—'"
              ></v-list-item>
              <v-list-item
                prepend-icon="mdi-file-cabinet"
                :title="
                  agent.host.compose.available ? 'Compose disponível' : 'Compose indisponível'
                "
                :subtitle="agent.host.compose.version || agent.host.compose.reason || '—'"
              ></v-list-item>
              <v-list-item
                prepend-icon="mdi-vpn"
                :title="
                  agent.deviceName ? `Dispositivo ${agent.deviceName}` : 'Sem dispositivo vinculado'
                "
                :subtitle="deviceSubtitle(agent)"
              ></v-list-item>
              <v-list-item
                prepend-icon="mdi-clock-outline"
                :title="`Visto ${formatRelativeTime(agent.lastSeenAt)}`"
                :subtitle="agent.version ? `Versão ${agent.version}` : 'Versão desconhecida'"
              ></v-list-item>
            </v-list>
            <div class="d-flex flex-wrap ga-1 mt-3">
              <v-chip
                v-for="permission in agent.host.policy"
                :key="permission"
                size="x-small"
                variant="outlined"
                :color="permissionColor(permission)"
              >
                {{ permissionLabel(permission) }}
              </v-chip>
            </div>
          </v-card-text>
          <v-card-actions class="flex-wrap ga-1 px-4 pb-4">
            <v-btn
              color="primary"
              variant="tonal"
              size="small"
              prepend-icon="mdi-docker"
              :to="{ path: '/docker', query: { host: agent.hostKey } }"
            >
              Docker
            </v-btn>
            <v-btn
              color="primary"
              variant="tonal"
              size="small"
              prepend-icon="mdi-chart-timeline-variant"
              :to="{ path: '/docker/history', query: { host: agent.hostKey } }"
            >
              Histórico
            </v-btn>
            <template v-if="auth.isAdmin">
              <v-btn
                v-if="agent.status !== 'revoked'"
                color="info"
                variant="tonal"
                size="small"
                prepend-icon="mdi-key-plus"
                :loading="agentsStore.saving && pendingId === agent.id"
                @click="reissue(agent)"
              >
                Código de instalação
              </v-btn>
              <v-btn
                v-if="agent.status !== 'revoked'"
                color="warning"
                variant="tonal"
                size="small"
                prepend-icon="mdi-cancel"
                @click="ask('revoke', agent)"
              >
                Revogar
              </v-btn>
              <v-btn
                color="error"
                variant="tonal"
                size="small"
                prepend-icon="mdi-delete-outline"
                @click="ask('remove', agent)"
              >
                Remover
              </v-btn>
            </template>
          </v-card-actions>
        </v-card>
      </v-col>
    </v-row>

    <v-dialog v-model="createOpen" max-width="480">
      <v-card rounded="xl">
        <v-card-title>Conectar servidor</v-card-title>
        <v-card-text>
          <v-text-field
            v-model="form.name"
            label="Nome do servidor"
            variant="outlined"
            :rules="[(value: string) => Boolean(value?.trim()) || 'Informe um nome']"
            autofocus
          ></v-text-field>
          <v-switch
            v-model="form.enforceTunnelIp"
            color="primary"
            label="Exigir conexão pelo túnel VPN do dispositivo vinculado"
            hide-details
          ></v-switch>
          <p class="text-caption text-medium-emphasis mt-2 mb-0">
            Servidores criados pela VPN já nascem vinculados ao dispositivo do túnel. No próximo
            passo você escolhe por qual endereço desta central ele vai se conectar.
          </p>
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="createOpen = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            variant="flat"
            :loading="agentsStore.saving"
            :disabled="!form.name.trim()"
            @click="create"
          >
            Cadastrar
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="confirm.open" max-width="440">
      <v-card rounded="xl">
        <v-card-title>{{
          confirm.kind === 'revoke' ? 'Revogar acesso' : 'Remover servidor'
        }}</v-card-title>
        <v-card-text>
          <template v-if="confirm.kind === 'revoke'">
            O servidor <strong>{{ confirm.name }}</strong> é desconectado e a credencial deixa de
            valer. Para voltar a conectá-lo, cadastre-o de novo.
          </template>
          <template v-else>
            O servidor <strong>{{ confirm.name }}</strong
            >, os monitores atribuídos a ele e o histórico de métricas do host serão apagados.
          </template>
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="confirm.open = false">Cancelar</v-btn>
          <v-btn
            :color="confirm.kind === 'revoke' ? 'warning' : 'error'"
            variant="flat"
            :loading="agentsStore.saving"
            @click="runConfirmed"
          >
            {{ confirm.kind === 'revoke' ? 'Revogar' : 'Remover' }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <AgentEnrollmentDialog
      v-model="enrollmentOpen"
      :enrollment="enrollment"
      :agent-name="enrollmentAgent"
    ></AgentEnrollmentDialog>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import AgentEnrollmentDialog from '@/components/agents/AgentEnrollmentDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import { useAgentsStore } from '@/stores/agents'
import { useAuthStore } from '@/stores/auth'
import { formatRelativeTime } from '@/utils/formatters'
import type { AgentEnrollmentView } from '@/bindings/AgentEnrollmentView'
import type { AgentView } from '@/bindings/AgentView'
import type { Permission } from '@/bindings/Permission'

const agentsStore = useAgentsStore()
const auth = useAuthStore()

const createOpen = ref(false)
const form = reactive({ name: '', enforceTunnelIp: true })
const enrollmentOpen = ref(false)
const enrollment = ref<AgentEnrollmentView | null>(null)
const enrollmentAgent = ref('')
const pendingId = ref<number | null>(null)
const confirm = reactive({
  open: false,
  kind: 'revoke' as 'revoke' | 'remove',
  id: 0,
  name: '',
})

const PERMISSION_LABELS: Record<Permission, string> = {
  read: 'Leitura',
  lifecycle: 'Ciclo de vida',
  update: 'Pull / recriar',
  compose: 'Compose',
  monitor: 'Monitores',
  discovery: 'Descoberta',
}

function permissionLabel(permission: Permission): string {
  return PERMISSION_LABELS[permission]
}

function permissionColor(permission: Permission): string {
  return permission === 'update' || permission === 'compose' ? 'warning' : 'primary'
}

function statusMeta(agent: AgentView): { label: string; color: string } {
  if (agent.status === 'revoked') return { label: 'Revogado', color: 'grey' }
  if (agent.connected || agent.status === 'online') return { label: 'Conectado', color: 'success' }
  if (agent.status === 'pending') return { label: 'Aguardando instalação', color: 'info' }
  return { label: 'Desconectado', color: 'error' }
}

function deviceSubtitle(agent: AgentView): string {
  const tunnel = agent.enforceTunnelIp ? 'conexão exigida pelo túnel' : 'conexão de qualquer origem'
  return agent.deviceIp ? `${agent.deviceIp} · ${tunnel}` : tunnel
}

function openCreate() {
  form.name = ''
  form.enforceTunnelIp = true
  createOpen.value = true
}

async function create() {
  const created = await agentsStore.createAgent({
    name: form.name.trim(),
    siteId: null,
    deviceId: null,
    enforceTunnelIp: form.enforceTunnelIp,
  })
  if (!created) return
  createOpen.value = false
  enrollment.value = created.enrollment
  enrollmentAgent.value = created.agent.name
  enrollmentOpen.value = true
}

async function reissue(agent: AgentView) {
  pendingId.value = agent.id
  const issued = await agentsStore.reissueEnrollment(agent.id)
  pendingId.value = null
  if (!issued) return
  enrollment.value = issued
  enrollmentAgent.value = agent.name
  enrollmentOpen.value = true
}

function ask(kind: 'revoke' | 'remove', agent: AgentView) {
  confirm.kind = kind
  confirm.id = agent.id
  confirm.name = agent.name
  confirm.open = true
}

async function runConfirmed() {
  const ok =
    confirm.kind === 'revoke'
      ? await agentsStore.revokeAgent(confirm.id)
      : await agentsStore.removeAgent(confirm.id)
  if (ok) confirm.open = false
}

onMounted(() => {
  agentsStore.fetchAgents()
})
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    max-width="760"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="enrollment && commands" rounded="xl">
      <v-card-title class="d-flex align-center ga-2">
        <v-icon :color="connected ? 'success' : 'primary'">
          {{ connected ? 'mdi-check-circle' : 'mdi-server' }}
        </v-icon>
        {{ connected ? `${agentName} conectado à central` : `Conectar ${agentName} à central` }}
      </v-card-title>
      <v-card-text>
        <v-alert v-if="connected" type="success" variant="tonal" density="compact" class="mb-4">
          O servidor se conectou{{ connectedDetails }}. Já dá para ver o Docker e as métricas dele.
        </v-alert>
        <v-alert v-else type="warning" variant="tonal" density="compact" class="mb-4">
          O código <strong>{{ enrollment.code.slice(0, 12) }}…</strong> vale uma única vez e expira
          em {{ expiresInMinutes }} minutos. Ele não será exibido de novo — gere outro se precisar.
        </v-alert>

        <!-- Mesmo seletor do guia de syslog: a lista e o atalho para corrigi-la. -->
        <div class="d-flex align-center ga-2 mb-2">
          <v-select
            :model-value="commands.addressId"
            :items="addressOptions"
            item-title="title"
            item-value="value"
            :item-props="addressItemProps"
            label="Endereço desta central que o servidor vai usar"
            density="compact"
            variant="outlined"
            hide-details
            :loading="switching"
            :disabled="addressOptions.length === 0"
            class="flex-grow-1 min-width-0"
            @update:model-value="changeAddress"
          ></v-select>
          <ServerAddressesButton density="compact" @saved="reloadAddresses" />
        </div>
        <p class="text-caption text-medium-emphasis mb-4">
          O servidor vai conectar em <code>{{ commands.serverUrl }}</code
          >. Escolha o caminho pelo qual ele alcança esta central: túnel VPN, rede local ou
          internet.
        </p>
        <v-alert v-if="addressHint" type="info" variant="tonal" density="compact" class="mb-4">
          {{ addressHint }}
        </v-alert>

        <CopyableCommand
          label="Serviço systemd (recomendado)"
          :command="commands.systemdCommand"
          class="mb-4"
        ></CopyableCommand>
        <CopyableCommand
          label="Container Docker"
          :command="commands.dockerCommand"
        ></CopyableCommand>
        <p class="text-caption text-medium-emphasis mt-4 mb-0">
          Rode <strong>um</strong> dos comandos no servidor. Ele instala o agente NetMonitor, que só
          disca para esta central e não abre nenhuma porta. A política local
          <code>AGENT_ALLOW</code> decide o que o servidor aceita: acrescente <code>update</code> e
          <code>compose</code> só onde pull, recriação e ações de compose devem ser permitidos.
        </p>
      </v-card-text>
      <v-card-actions>
        <v-btn
          v-if="connected && agent"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-docker"
          :to="{ path: '/docker', query: { host: agent.hostKey } }"
          @click="emit('update:modelValue', false)"
        >
          Abrir Docker
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="flat" @click="emit('update:modelValue', false)">
          Concluir
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import CopyableCommand from '@/components/CopyableCommand.vue'
import ServerAddressesButton from '@/components/ServerAddressesButton.vue'
import { useAgentsStore } from '@/stores/agents'
import { useServerAddressesStore } from '@/stores/serverAddresses'
import type { AgentEnrollmentView } from '@/bindings/AgentEnrollmentView'
import type { AgentInstallCommands } from '@/bindings/AgentInstallCommands'

interface AddressOption {
  value: string
  title: string
  subtitle: string
}

const props = defineProps<{
  modelValue: boolean
  enrollment: AgentEnrollmentView | null
  agentName: string
  /** Para acompanhar pelo SSE (`probe:status`) o momento em que ele conecta. */
  agentId?: number | null
}>()

const emit = defineEmits<{
  (event: 'update:modelValue', value: boolean): void
}>()

const agentsStore = useAgentsStore()
const addressesStore = useServerAddressesStore()
const commands = ref<AgentInstallCommands | null>(null)
const switching = ref(false)
/** Só a conexão que acontece com o diálogo aberto conta: numa reinstalação o
 * servidor podia já estar conectado com a instalação antiga. */
const connected = ref(false)

const agent = computed(() =>
  props.agentId ? (agentsStore.agents.find((item) => item.id === props.agentId) ?? null) : null
)

const connectedDetails = computed(() => {
  const host = agent.value?.host
  const parts = [host?.hostname, host?.os].filter(Boolean)
  return parts.length ? ` (${parts.join(' · ')})` : ''
})

watch(
  () => agent.value?.connected,
  (now, before) => {
    if (props.modelValue && now && before === false) connected.value = true
  }
)

const expiresInMinutes = computed(() => Math.round((props.enrollment?.expiresInSeconds ?? 0) / 60))

const addressOptions = computed<AddressOption[]>(() =>
  addressesStore.usable.map((entry) => ({
    value: entry.id,
    title: `${entry.label} — ${entry.value}`,
    subtitle: entry.description,
  }))
)

function addressItemProps(item: AddressOption): Record<string, unknown> {
  return { subtitle: item.subtitle }
}

/** Sem entrada escolhida: ou a lista está vazia, ou `AGENT_SERVER_URL` fixou o endereço. */
const addressHint = computed(() => {
  if (!commands.value || commands.value.addressId) return null
  return addressOptions.value.length === 0
    ? 'Nenhum endereço deste servidor está cadastrado; o comando usa o endereço pelo qual você abriu esta tela. Cadastre o endereço certo na engrenagem ao lado.'
    : 'O endereço está fixado na configuração da central (AGENT_SERVER_URL).'
})

async function changeAddress(addressId: string | null): Promise<void> {
  if (!props.enrollment || !addressId || addressId === commands.value?.addressId) return
  switching.value = true
  const rendered = await agentsStore.installCommands(props.enrollment.code, addressId)
  switching.value = false
  if (rendered) commands.value = rendered
}

/** Depois de corrigir a lista, os comandos seguem o endereço escolhido. */
async function reloadAddresses(): Promise<void> {
  await addressesStore.fetchAll(true)
  const current = commands.value?.addressId
  const stillThere = addressesStore.usable.some((entry) => entry.id === current)
  const target = stillThere ? current : addressesStore.usable[0]?.id
  if (target && props.enrollment) {
    switching.value = true
    const rendered = await agentsStore.installCommands(props.enrollment.code, target)
    switching.value = false
    if (rendered) commands.value = rendered
  }
}

watch(
  () => props.enrollment,
  (enrollment) => {
    commands.value = enrollment?.commands ?? null
    connected.value = false
    if (enrollment) void addressesStore.fetchAll()
  },
  { immediate: true }
)
</script>

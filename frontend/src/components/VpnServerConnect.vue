<template>
  <div>
    <v-dialog :model-value="stage === 'setup'" max-width="480" @update:model-value="finish">
      <v-card rounded="lg">
        <v-card-title class="d-flex align-center">
          <v-icon start color="warning">mdi-shield-alert-outline</v-icon>
          A VPN ainda não está pronta
        </v-card-title>
        <v-card-text>
          <p class="text-body-2 text-high-emphasis mb-0">
            Para conectar um servidor pela VPN, esta central precisa primeiro do servidor WireGuard
            configurado, com o IP público (ou DDNS) e a porta UDP liberada. Configure a VPN e volte
            aqui.
          </p>
          <v-alert
            v-if="vpnStore.error"
            type="error"
            variant="tonal"
            density="compact"
            class="mt-4"
          >
            {{ vpnStore.error }}
          </v-alert>
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="finish">Fechar</v-btn>
          <v-btn color="primary" variant="flat" prepend-icon="mdi-cog" :to="{ name: 'vpn-server' }">
            Configurar a VPN
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <VpnPeerWizard
      :model-value="stage === 'wizard'"
      :preset="preset"
      @update:model-value="onWizardToggle"
      @created="created = true"
    ></VpnPeerWizard>

    <VpnScriptViewer
      :model-value="stage === 'script'"
      :artifact="vpnStore.lastArtifact"
      @update:model-value="finish"
    ></VpnScriptViewer>
  </div>
</template>

<script setup lang="ts">
/**
 * Conecta um servidor remoto pela VPN: o mesmo assistente de peer, já no
 * perfil Linux e com o agente incluído, seguido do script que sobe o túnel e
 * instala o agente. Com `agentId`, é o servidor já cadastrado que entra no
 * túnel — mesmo cadastro, mesmo histórico.
 */
import { computed, ref, watch } from 'vue'
import VpnPeerWizard from '@/components/VpnPeerWizard.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import { useVpnStore } from '@/stores/vpn'

const props = defineProps<{
  modelValue: boolean
  serverName: string
  agentId?: number | null
}>()

const emit = defineEmits<{
  (event: 'update:modelValue', value: boolean): void
  (event: 'done'): void
}>()

const vpnStore = useVpnStore()
const stage = ref<'idle' | 'setup' | 'wizard' | 'script'>('idle')
const created = ref(false)

const preset = computed(() => ({ name: props.serverName, agentId: props.agentId ?? null }))

watch(
  () => props.modelValue,
  async (open) => {
    if (!open) {
      stage.value = 'idle'
      return
    }
    created.value = false
    await vpnStore.fetchServer()
    if (!props.modelValue) return
    stage.value = vpnStore.isConfigured ? 'wizard' : 'setup'
  },
  { immediate: true }
)

/** O assistente fecha sozinho ao gerar o peer: aí o script assume a tela. */
function onWizardToggle(open: boolean) {
  if (open) return
  if (created.value) stage.value = 'script'
  else finish()
}

function finish() {
  if (created.value) emit('done')
  emit('update:modelValue', false)
}
</script>

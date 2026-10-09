<template>
  <v-card class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="font-weight-bold d-flex align-center justify-space-between flex-wrap ga-2">
      <div class="d-flex align-center">
        <v-icon start color="primary">mdi-robot-outline</v-icon>
        Assistente inteligente (IA)
      </div>
      <v-switch
        v-model="form.enabled"
        color="primary"
        density="compact"
        hide-details
        label="Ativo"
      />
    </v-card-title>

    <v-card-subtitle class="text-wrap">
      Provedor de IA para diagnósticos de rede e dúvidas sobre o sistema
    </v-card-subtitle>

    <v-divider class="my-2" />

    <v-card-text class="flex-grow-1">
      <v-progress-linear v-if="!loaded" indeterminate color="primary" />
      <template v-else>
        <v-alert v-if="!form.enabled" type="info" variant="tonal" density="compact" class="mb-4">
          O assistente está desligado. Ligue a chave <strong>Ativo</strong> no topo do cartão para
          usar o chat de diagnóstico e suporte.
        </v-alert>

        <!-- Provedor e modelo -->
        <div class="font-weight-bold mb-2">Provedor</div>
        <v-select
          v-model="form.activeDriver"
          label="Provedor de IA"
          :items="DRIVER_OPTIONS"
          item-title="title"
          item-value="value"
          variant="outlined"
          density="compact"
          prepend-inner-icon="mdi-swap-horizontal"
          hide-details="auto"
          class="mb-3"
        />
        <OllamaProviderFields
          v-if="driver === 'ollama'"
          v-model:base-url="form.ollamaBaseUrl"
          v-model:model="form.ollamaModel"
          v-model:num-ctx="form.ollamaNumCtx"
          @open-catalog="showModelSearchDialog = true"
          @install="installOllamaModel"
        />
        <OpenrouterProviderFields
          v-else-if="driver === 'openrouter'"
          v-model:api-key="form.openrouterApiKey"
          v-model:model="form.openrouterModel"
          @open-catalog="showModelSearchDialog = true"
        />
        <OpencodeProviderFields
          v-else
          v-model:api-key="form.opencodeApiKey"
          v-model:model="form.opencodeModel"
          :base-url="form.opencodeBaseUrl"
          @open-catalog="showModelSearchDialog = true"
        />

        <v-divider class="my-5" />

        <!-- Como a IA responde e o que ela pode fazer -->
        <AiModeToggle
          v-model="form.responseStyle"
          title="Estilo de resposta"
          :options="RESPONSE_STYLE_OPTIONS"
          class="mb-5"
        />
        <AiAutomationSettings
          v-model:allow-active-tools="form.allowActiveTools"
          v-model:require-tool-confirmation="form.requireToolConfirmation"
          v-model:allow-actions="form.allowActions"
          v-model:container-actions="form.containerActions"
          v-model:proactive="form.proactive"
        />
      </template>

      <v-alert
        v-if="aiStore.testResult"
        :type="aiStore.testResult.success ? 'success' : 'error'"
        variant="tonal"
        density="compact"
        class="mt-4"
        closable
        @click:close="aiStore.testResult = null"
      >
        {{ aiStore.testResult.message }}
        <span v-if="aiStore.testResult.success">
          (latência: {{ formatLatency(aiStore.testResult.latencyMs) }})
        </span>
      </v-alert>

      <v-alert
        v-if="aiStore.saveError"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-4"
        icon="mdi-alert-circle-outline"
        closable
        @click:close="aiStore.saveError = null"
      >
        <strong>Não foi possível salvar:</strong> {{ aiStore.saveError }}
      </v-alert>
    </v-card-text>

    <v-card-actions class="pa-4 pt-0 justify-space-between flex-wrap ga-2">
      <v-btn
        variant="outlined"
        color="primary"
        prepend-icon="mdi-lan-connect"
        :loading="aiStore.testingConnection"
        :disabled="!loaded"
        @click="handleTestConnection"
      >
        Testar conexão
      </v-btn>
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-content-save-outline"
        :loading="aiStore.savingSettings"
        :disabled="!loaded"
        @click="handleSave"
      >
        Salvar
      </v-btn>
    </v-card-actions>
  </v-card>

  <AiModelSearchDialog
    v-model="showModelSearchDialog"
    :driver="driver"
    :current-model="currentModel"
    :api-key="providerAccess.apiKey"
    :base-url="providerAccess.baseUrl"
    @select="selectModel"
    @install="installOllamaModel"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useAiStore, type AiSettings } from '@/stores/ai'
import { RESPONSE_STYLE_OPTIONS } from '@/components/ai/aiResponseStyle'
import { formatLatency } from '@/utils/formatters'
import AiModelSearchDialog from './AiModelSearchDialog.vue'
import AiAutomationSettings from './AiAutomationSettings.vue'
import AiModeToggle from './ai/AiModeToggle.vue'
import OllamaProviderFields from './ai/OllamaProviderFields.vue'
import OpencodeProviderFields from './ai/OpencodeProviderFields.vue'
import OpenrouterProviderFields from './ai/OpenrouterProviderFields.vue'
import {
  DRIVER_OPTIONS,
  MODEL_FIELDS,
  connectionTestInput,
  defaultAiSettings,
  extractModelId,
  modelForDriver,
  normalizeAiSettings,
  resolveDriver,
} from './ai/aiProviders'

/**
 * Orquestra o formulário do Assistente IA: carrega, normaliza, testa e salva.
 * Os campos de cada provedor, o instalador do Ollama e as permissões vivem
 * nos componentes de `./ai/`. Sucesso sai pelo `saved` (snackbar da página);
 * erro de gravação fica no alerta junto do botão, até ser corrigido.
 */
const emit = defineEmits<{
  (e: 'saved', message: string, color?: string): void
}>()

const aiStore = useAiStore()
const showModelSearchDialog = ref(false)
const loaded = ref(false)

// `customSystemPrompt` não tem campo na tela, mas viaja no formulário de volta à API.
const form = reactive<AiSettings>(defaultAiSettings())

const driver = computed(() => resolveDriver(form.activeDriver))
const currentModel = computed(() => modelForDriver(form))
/** Chave e endpoint do provedor ativo, os mesmos do teste de conexão. */
const providerAccess = computed(() => connectionTestInput(form))

function normalizeForm() {
  Object.assign(form, normalizeAiSettings(form))
}

function selectModel(modelId: string) {
  form[MODEL_FIELDS[driver.value]] = extractModelId(modelId)
}

/** Único caminho de download no Ollama: o instalador do cartão e o catálogo. */
async function installOllamaModel(modelName: string) {
  const target = modelName.trim()
  if (!target) return
  await aiStore.pullOllamaModel(target, form.ollamaBaseUrl, () => {
    form.ollamaModel = target
    emit('saved', `Modelo ${target} baixado e instalado no Ollama.`, 'success')
  })
}

async function handleTestConnection() {
  normalizeForm()
  await aiStore.testConnection(connectionTestInput(form))
}

async function handleSave() {
  normalizeForm()
  const result = await aiStore.saveSettings({ ...form })
  if (result.success) emit('saved', 'Configurações de IA salvas.', 'success')
}

onMounted(async () => {
  await aiStore.loadSettings()
  // Cópia própria (normalizeAiSettings copia `proactive`): editar não mexe na store antes de salvar.
  Object.assign(
    form,
    normalizeAiSettings(aiStore.settings ? { ...defaultAiSettings(), ...aiStore.settings } : form)
  )
  loaded.value = true
})
</script>

<template>
  <v-row dense>
    <v-col cols="12">
      <div class="d-flex align-center flex-wrap ga-2 pa-2 px-3 rounded-lg border">
        <v-icon color="primary" size="18">mdi-link-variant</v-icon>
        <span class="text-body-2 font-weight-medium">Endpoint oficial:</span>
        <code class="text-body-2 font-weight-bold text-primary">{{ OPENCODE_BASE_URL }}</code>
      </div>
    </v-col>

    <v-col cols="12" md="6">
      <AiApiKeyField
        v-model="apiKey"
        label="API Key do OpenCode Zen"
        placeholder="oc_sk_..."
        @committed="apiKey && refresh()"
      />
    </v-col>

    <v-col cols="12" md="6">
      <AiModelPicker
        v-model="model"
        label="Modelo OpenCode"
        driver="opencode"
        :placeholder="DEFAULT_MODELS.opencode"
        :items="choices"
        :loading="aiStore.loadingOpencodeModels"
        @refresh="refresh"
        @open-catalog="emit('open-catalog')"
      />
    </v-col>

    <v-col v-if="aiStore.opencodeError" cols="12">
      <v-alert
        type="info"
        variant="tonal"
        density="compact"
        closable
        icon="mdi-information-outline"
        @click:close="aiStore.opencodeError = null"
      >
        {{ aiStore.opencodeError }}
      </v-alert>
    </v-col>
  </v-row>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useAiStore } from '@/stores/ai'
import AiApiKeyField from './AiApiKeyField.vue'
import AiModelPicker from './AiModelPicker.vue'
import { opencodeChoices } from './aiModelCatalog'
import { FALLBACK_OPENCODE_MODELS } from './aiModelFallbacks'
import { DEFAULT_MODELS, OPENCODE_BASE_URL } from './aiProviders'

/** Campos do OpenCode Go / Zen. Montar é escolher o provedor: carrega o catálogo. */
const props = defineProps<{
  baseUrl?: string | null
}>()

const apiKey = defineModel<string | null | undefined>('apiKey', { required: true })
const model = defineModel<string | null | undefined>('model', { required: true })

const emit = defineEmits<{
  (e: 'open-catalog'): void
}>()

const aiStore = useAiStore()

const choices = computed(() =>
  opencodeChoices(aiStore.opencodeModels.length ? aiStore.opencodeModels : FALLBACK_OPENCODE_MODELS)
)

function refresh() {
  void aiStore.loadOpencodeModels(apiKey.value, props.baseUrl || OPENCODE_BASE_URL)
}

onMounted(refresh)
</script>

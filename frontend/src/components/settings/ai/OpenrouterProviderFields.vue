<template>
  <v-row dense>
    <v-col cols="12" md="6">
      <AiApiKeyField
        v-model="apiKey"
        label="OpenRouter API Key"
        placeholder="sk-or-..."
        @committed="apiKey && refresh()"
      />
    </v-col>

    <v-col cols="12" md="6">
      <AiModelPicker
        v-model="model"
        label="Modelo OpenRouter"
        driver="openrouter"
        :placeholder="DEFAULT_MODELS.openrouter"
        :items="choices"
        :loading="aiStore.loadingOpenrouterModels"
        @refresh="refresh"
        @open-catalog="emit('open-catalog')"
      >
        <template #hint>
          <code>{{ DEFAULT_MODELS.openrouter }}</code> escolhe sozinho um modelo gratuito.
        </template>
      </AiModelPicker>
    </v-col>

    <v-col v-if="aiStore.openrouterError" cols="12">
      <v-alert
        type="info"
        variant="tonal"
        density="compact"
        closable
        icon="mdi-information-outline"
        @click:close="aiStore.openrouterError = null"
      >
        {{ aiStore.openrouterError }}
      </v-alert>
    </v-col>
  </v-row>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useAiStore } from '@/stores/ai'
import AiApiKeyField from './AiApiKeyField.vue'
import AiModelPicker from './AiModelPicker.vue'
import { openrouterChoices } from './aiModelCatalog'
import { FALLBACK_OPENROUTER_MODELS } from './aiModelFallbacks'
import { DEFAULT_MODELS } from './aiProviders'

/** Campos do OpenRouter. Montar é escolher o provedor: carrega o catálogo. */
const apiKey = defineModel<string | null | undefined>('apiKey', { required: true })
const model = defineModel<string | null | undefined>('model', { required: true })

const emit = defineEmits<{
  (e: 'open-catalog'): void
}>()

const aiStore = useAiStore()

const choices = computed(() =>
  openrouterChoices(
    aiStore.openrouterModels.length ? aiStore.openrouterModels : FALLBACK_OPENROUTER_MODELS
  )
)

function refresh() {
  void aiStore.loadOpenrouterModels(apiKey.value)
}

onMounted(refresh)
</script>

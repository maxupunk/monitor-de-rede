<template>
  <v-row dense>
    <v-col cols="12" md="6">
      <v-text-field
        v-model="baseUrl"
        label="URL base do Ollama"
        :placeholder="OLLAMA_DEFAULT_BASE_URL"
        variant="outlined"
        density="compact"
        prepend-inner-icon="mdi-server"
        hide-details="auto"
        @blur="baseUrl && refresh()"
      />
    </v-col>

    <v-col cols="12" md="6">
      <AiModelPicker
        v-model="model"
        label="Modelo em uso"
        driver="ollama"
        :placeholder="DEFAULT_MODELS.ollama"
        :items="choices"
        :loading="aiStore.loadingOllamaModels"
        clearable
        catalog-label="Ver biblioteca de modelos"
        @refresh="refresh"
        @open-catalog="emit('open-catalog')"
      />
    </v-col>

    <v-col cols="12" md="6">
      <v-select
        v-model="numCtx"
        label="Janela de contexto (num_ctx)"
        :items="OLLAMA_NUM_CTX_OPTIONS"
        item-title="title"
        item-value="value"
        variant="outlined"
        density="compact"
        prepend-inner-icon="mdi-memory"
        hint="16k a 32k atende a maioria dos modelos locais. O novo limite vale depois de Salvar."
        persistent-hint
      />
    </v-col>

    <v-col v-if="!aiStore.ollamaOnline" cols="12">
      <v-alert type="warning" variant="tonal" density="compact" icon="mdi-server-off">
        <div class="d-flex align-center justify-space-between flex-wrap ga-2">
          <div>
            <div class="font-weight-bold">Ollama fora do ar ou inacessível</div>
            <div class="text-body-2">
              {{
                aiStore.ollamaError ||
                'Não foi possível conectar ao Ollama. Verifique se o serviço está em execução.'
              }}
            </div>
          </div>
          <v-btn
            variant="flat"
            color="warning"
            size="small"
            prepend-icon="mdi-refresh"
            :loading="aiStore.loadingOllamaModels"
            @click="refresh"
          >
            Tentar novamente
          </v-btn>
        </div>
      </v-alert>
    </v-col>

    <v-col cols="12">
      <OllamaModelInstaller :current-model="model" @install="emit('install', $event)" />
    </v-col>
  </v-row>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useAiStore } from '@/stores/ai'
import AiModelPicker from './AiModelPicker.vue'
import OllamaModelInstaller from './OllamaModelInstaller.vue'
import { isOllamaModelInstalled, ollamaChoices, withTypedModel } from './aiModelCatalog'
import { DEFAULT_MODELS, OLLAMA_DEFAULT_BASE_URL, OLLAMA_NUM_CTX_OPTIONS } from './aiProviders'

/** Campos do Ollama local. Montar é escolher o provedor: consulta o servidor. */
const baseUrl = defineModel<string | null | undefined>('baseUrl', { required: true })
const model = defineModel<string | null | undefined>('model', { required: true })
const numCtx = defineModel<number | null | undefined>('numCtx', { required: true })

const emit = defineEmits<{
  (e: 'open-catalog'): void
  (e: 'install', modelName: string): void
}>()

const aiStore = useAiStore()

const choices = computed(() =>
  withTypedModel(
    ollamaChoices(aiStore.installedOllamaModels, aiStore.recommendedOllamaModels),
    model.value,
    isOllamaModelInstalled(aiStore.installedOllamaModels, model.value)
  )
)

function refresh() {
  void aiStore.loadOllamaModels(baseUrl.value)
}

onMounted(refresh)
</script>

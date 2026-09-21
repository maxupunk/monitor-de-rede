<template>
  <v-card class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="font-weight-bold d-flex align-center justify-space-between">
      <div class="d-flex align-center">
        <v-icon start color="primary">mdi-robot-outline</v-icon>
        Assistente Inteligente (IA)
      </div>
      <v-switch
        v-model="form.enabled"
        color="primary"
        density="compact"
        hide-details
        label="Ativo"
      />
    </v-card-title>

    <v-card-subtitle>
      Configuração do provedor de IA para diagnósticos de rede e dúvidas do sistema
    </v-card-subtitle>

    <v-divider class="my-2" />

    <v-card-text class="flex-grow-1">
      <!-- Alerta de erro ao salvar -->
      <v-alert
        v-if="localSaveError || aiStore.saveError"
        type="error"
        variant="tonal"
        density="compact"
        class="mb-3"
        icon="mdi-alert-circle-outline"
        closable
        @click:close="dismissSaveError"
      >
        <strong>Erro ao salvar:</strong> {{ localSaveError || aiStore.saveError }}
      </v-alert>

      <!-- Alerta de sucesso ao salvar -->
      <v-alert
        v-if="localSaveSuccess"
        type="success"
        variant="tonal"
        density="compact"
        class="mb-3"
        icon="mdi-check-circle-outline"
        closable
        @click:close="localSaveSuccess = null"
      >
        {{ localSaveSuccess }}
      </v-alert>

      <v-alert v-if="!form.enabled" type="info" variant="tonal" density="compact" class="mb-4">
        O assistente IA está desativado. Ative-o acima para habilitar o chat de diagnóstico e
        suporte.
      </v-alert>

      <v-row dense>
        <!-- Seleção do Driver -->
        <v-col cols="12">
          <v-select
            v-model="form.activeDriver"
            label="Provedor / Driver de IA"
            :items="driverOptions"
            item-title="title"
            item-value="value"
            variant="outlined"
            density="compact"
            prepend-inner-icon="mdi-swap-horizontal"
            hide-details="auto"
            class="mb-3"
          />
        </v-col>

        <!-- Campos: OpenCode Go / Zen -->
        <template v-if="form.activeDriver === 'opencode'">
          <v-col cols="12">
            <div
              class="d-flex align-center justify-space-between pa-2 px-3 rounded-lg bg-surface-variant border mb-2"
            >
              <div class="d-flex align-center ga-2">
                <v-icon color="primary" size="18">mdi-link-variant</v-icon>
                <span class="text-caption text-medium-emphasis">Endpoint Oficial Fixo:</span>
                <code class="text-caption font-mono font-weight-bold text-primary"
                  >https://opencode.ai/zen/v1</code
                >
              </div>
              <v-chip size="x-small" color="primary" variant="tonal">OpenCode Zen Gateway</v-chip>
            </div>
          </v-col>

          <v-col cols="12" md="6">
            <v-text-field
              v-model="form.opencodeApiKey"
              label="API Key do OpenCode Zen"
              placeholder="oc_sk_..."
              :type="showApiKey ? 'text' : 'password'"
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-key-outline"
              :append-inner-icon="showApiKey ? 'mdi-eye-off' : 'mdi-eye'"
              hide-details="auto"
              class="mb-3"
              @click:append-inner="showApiKey = !showApiKey"
              @blur="handleOpencodeApiKeyBlur"
            />
          </v-col>

          <v-col cols="12" md="6">
            <v-combobox
              v-model="form.opencodeModel"
              label="Modelo OpenCode"
              :items="opencodeModelOptions"
              item-title="id"
              item-value="id"
              :return-object="false"
              :custom-filter="customModelFilter"
              :loading="aiStore.loadingOpencodeModels"
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-cube-outline"
              placeholder="muse-spark-1.3-contributor-free"
              hide-details="auto"
              class="mb-1"
              @update:model-value="(val) => (form.opencodeModel = extractModelId(val))"
            >
              <template #append-inner>
                <v-btn
                  icon="mdi-magnify"
                  variant="text"
                  size="x-small"
                  color="primary"
                  title="Buscar e explorar modelos no catálogo"
                  @click.stop="openModelSearchDialog"
                />
                <v-btn
                  icon="mdi-refresh"
                  variant="text"
                  size="x-small"
                  :loading="aiStore.loadingOpencodeModels"
                  title="Atualizar lista de modelos do OpenCode"
                  @click.stop="refreshOpencodeModels"
                />
              </template>
              <template #item="{ item, props: itemProps }">
                <v-list-item
                  v-bind="itemProps"
                  :title="item.name || item.id"
                  :subtitle="item.id !== item.name ? item.id : undefined"
                >
                  <template #append>
                    <v-chip
                      v-if="item.isFree"
                      color="success"
                      size="x-small"
                      variant="tonal"
                      class="ms-1 font-weight-bold"
                    >
                      Gratuito (Free)
                    </v-chip>
                    <v-chip v-else color="primary" size="x-small" variant="outlined" class="ms-1">
                      Pro / Standard
                    </v-chip>
                    <v-chip
                      v-if="item.supportsTools"
                      color="warning"
                      size="x-small"
                      variant="tonal"
                      class="ms-1"
                      title="Suporta execução de ferramentas"
                    >
                      <v-icon start size="12">mdi-tools</v-icon>
                      Tools
                    </v-chip>
                  </template>
                </v-list-item>
              </template>
            </v-combobox>

            <div class="d-flex align-center justify-end">
              <v-btn
                variant="text"
                size="x-small"
                color="primary"
                prepend-icon="mdi-magnify"
                @click="openModelSearchDialog"
              >
                Buscar no catálogo completo ({{ aiStore.opencodeModels.length || 75 }} modelos)
              </v-btn>
            </div>
          </v-col>

          <v-col v-if="aiStore.opencodeError" cols="12">
            <v-alert
              type="info"
              variant="tonal"
              density="compact"
              class="mb-3"
              closable
              icon="mdi-information-outline"
              @click:close="aiStore.opencodeError = null"
            >
              {{ aiStore.opencodeError }}
            </v-alert>
          </v-col>
        </template>

        <!-- Campos: OpenRouter -->
        <template v-if="form.activeDriver === 'openrouter'">
          <v-col cols="12" md="6">
            <v-text-field
              v-model="form.openrouterApiKey"
              label="OpenRouter API Key"
              placeholder="sk-or-..."
              :type="showApiKey ? 'text' : 'password'"
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-key-outline"
              :append-inner-icon="showApiKey ? 'mdi-eye-off' : 'mdi-eye'"
              hide-details="auto"
              class="mb-3"
              @click:append-inner="showApiKey = !showApiKey"
              @blur="handleOpenrouterApiKeyBlur"
            />
          </v-col>
          <v-col cols="12" md="6">
            <v-combobox
              v-model="form.openrouterModel"
              label="Modelo OpenRouter"
              :items="openRouterModelOptions"
              item-title="id"
              item-value="id"
              :return-object="false"
              :custom-filter="customModelFilter"
              :loading="aiStore.loadingOpenrouterModels"
              placeholder="openrouter/free"
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-cube-outline"
              hide-details="auto"
              class="mb-1"
              @update:model-value="(val) => (form.openrouterModel = extractModelId(val))"
            >
              <template #append-inner>
                <v-btn
                  icon="mdi-magnify"
                  variant="text"
                  size="x-small"
                  color="primary"
                  title="Buscar e explorar modelos no catálogo completo"
                  @click.stop="openModelSearchDialog"
                />
                <v-btn
                  icon="mdi-refresh"
                  variant="text"
                  size="x-small"
                  :loading="aiStore.loadingOpenrouterModels"
                  title="Atualizar lista de modelos do OpenRouter"
                  @click.stop="refreshOpenrouterModels"
                />
              </template>
              <template #item="{ item, props: itemProps }">
                <v-list-item
                  v-bind="itemProps"
                  :title="item.name || item.id"
                  :subtitle="item.id !== item.name ? item.id : undefined"
                >
                  <template #append>
                    <v-chip
                      v-if="item.isFree"
                      color="success"
                      size="x-small"
                      variant="tonal"
                      class="ms-1 font-weight-bold"
                    >
                      {{ item.id === 'openrouter/free' ? 'Auto Free Router' : 'Gratuito' }}
                    </v-chip>
                    <v-chip v-else color="primary" size="x-small" variant="outlined" class="ms-1">
                      Standard / Créditos
                    </v-chip>
                    <v-chip
                      v-if="item.supportsTools"
                      color="warning"
                      size="x-small"
                      variant="tonal"
                      class="ms-1"
                      title="Suporta execução de ferramentas"
                    >
                      <v-icon start size="12">mdi-tools</v-icon>
                      Tools
                    </v-chip>
                  </template>
                </v-list-item>
              </template>
            </v-combobox>

            <div class="d-flex align-center justify-space-between flex-wrap ga-1">
              <div class="text-caption text-medium-emphasis">
                💡 <code>openrouter/free</code> roteia automaticamente para modelos gratuitos.
              </div>
              <v-btn
                variant="text"
                size="x-small"
                color="primary"
                prepend-icon="mdi-magnify"
                @click="openModelSearchDialog"
              >
                Buscar na lista ({{ aiStore.openrouterModels.length || 440 }} modelos)
              </v-btn>
            </div>
          </v-col>
          <v-col v-if="aiStore.openrouterError" cols="12">
            <v-alert
              type="info"
              variant="tonal"
              density="compact"
              class="mb-3"
              closable
              icon="mdi-information-outline"
              @click:close="aiStore.openrouterError = null"
            >
              {{ aiStore.openrouterError }}
            </v-alert>
          </v-col>
        </template>

        <!-- Campos: Ollama Local -->
        <template v-if="form.activeDriver === 'ollama'">
          <v-col cols="12" md="7">
            <v-text-field
              v-model="form.ollamaBaseUrl"
              label="URL Base do Ollama"
              placeholder="http://localhost:11434/v1"
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-server"
              hide-details="auto"
              class="mb-3"
              @blur="handleOllamaBaseUrlBlur"
            />
          </v-col>
          <v-col cols="12" md="5">
            <v-combobox
              v-model="form.ollamaModel"
              label="Modelo Ollama"
              :items="ollamaModelOptions"
              item-title="id"
              item-value="id"
              :return-object="false"
              :custom-filter="customModelFilter"
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-cube-outline"
              :loading="aiStore.loadingOllamaModels"
              hide-details="auto"
              class="mb-1"
              clearable
              @update:model-value="(val) => (form.ollamaModel = extractModelId(val))"
            >
              <template #append-inner>
                <v-btn
                  icon="mdi-magnify"
                  variant="text"
                  size="x-small"
                  color="primary"
                  title="Buscar e explorar catálogo do Ollama"
                  @click.stop="openModelSearchDialog"
                />
                <v-btn
                  icon="mdi-refresh"
                  variant="text"
                  size="x-small"
                  :loading="aiStore.loadingOllamaModels"
                  title="Atualizar lista de modelos do Ollama"
                  @click.stop="refreshOllamaModels"
                />
              </template>
              <template #item="{ item, props: itemProps }">
                <v-list-item
                  v-bind="itemProps"
                  :title="item.name"
                  :subtitle="item.description || undefined"
                >
                  <template #append>
                    <v-chip
                      v-if="item.isInstalled"
                      color="success"
                      size="x-small"
                      variant="tonal"
                      class="ms-1"
                    >
                      Instalado
                      <span v-if="item.size" class="ms-1 font-weight-regular">
                        ({{ item.size }})
                      </span>
                    </v-chip>
                    <v-chip
                      v-else-if="item.isRecommended"
                      color="primary"
                      size="x-small"
                      variant="outlined"
                      class="ms-1"
                    >
                      Recomendado
                    </v-chip>
                    <v-chip
                      v-if="item.supportsTools"
                      color="warning"
                      size="x-small"
                      variant="tonal"
                      class="ms-1"
                    >
                      <v-icon start size="12">mdi-tools</v-icon>
                      Tool Use
                    </v-chip>
                  </template>
                </v-list-item>
              </template>
            </v-combobox>

            <div class="d-flex align-center justify-end">
              <v-btn
                variant="text"
                size="x-small"
                color="primary"
                prepend-icon="mdi-magnify"
                @click="openModelSearchDialog"
              >
                Buscar na biblioteca de modelos Ollama
              </v-btn>
            </div>
          </v-col>

          <!-- Alerta se Ollama estiver offline / inacessível -->
          <v-col v-if="!aiStore.ollamaOnline" cols="12">
            <v-alert
              type="warning"
              variant="tonal"
              density="compact"
              class="mb-3"
              icon="mdi-server-off"
            >
              <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                <div>
                  <div class="font-weight-bold text-body-2">
                    Serviço Ollama offline ou inacessível
                  </div>
                  <div class="text-caption">
                    {{
                      aiStore.ollamaError ||
                      'Não foi possível conectar ao Ollama. Verifique se o serviço está em execução.'
                    }}
                  </div>
                </div>
                <v-btn
                  variant="outlined"
                  color="warning"
                  size="small"
                  :loading="aiStore.loadingOllamaModels"
                  @click="refreshOllamaModels"
                >
                  <v-icon start size="14">mdi-refresh</v-icon>
                  Tentar Novamente
                </v-btn>
              </div>
            </v-alert>
          </v-col>

          <!-- Alerta de Erro no Download / Instalação -->
          <v-col v-if="aiStore.pullError" cols="12">
            <v-alert
              type="error"
              variant="tonal"
              density="compact"
              class="mb-3"
              icon="mdi-alert-circle-outline"
              closable
              @click:close="aiStore.pullError = null"
            >
              <div class="font-weight-bold text-body-2 mb-1">
                Falha ao conectar ou instalar modelo no Ollama
              </div>
              <div class="text-body-2 mb-1">
                {{ aiStore.pullError }}
              </div>
              <div class="text-caption text-medium-emphasis">
                Certifique-se de que o Ollama está instalado na máquina e em execução (ex: execute
                <code>ollama serve</code> no terminal).
              </div>
            </v-alert>
          </v-col>

          <!-- Card de Progresso de Instalação de Modelo -->
          <v-col v-if="aiStore.pullingModelName" cols="12">
            <v-card variant="outlined" color="primary" class="pa-3 mb-3 bg-surface-variant">
              <div class="d-flex align-center justify-space-between mb-2">
                <div class="d-flex align-center ga-2">
                  <v-progress-circular indeterminate color="primary" size="20" width="2" />
                  <span class="font-weight-bold text-body-2">
                    Instalando modelo: {{ aiStore.pullingModelName }}
                  </span>
                </div>
                <v-chip size="x-small" color="primary" variant="flat" class="font-weight-bold">
                  {{
                    aiStore.pullProgress?.percentage != null
                      ? `${aiStore.pullProgress.percentage.toFixed(1)}%`
                      : 'Baixando'
                  }}
                </v-chip>
              </div>

              <v-progress-linear
                :model-value="aiStore.pullProgress?.percentage ?? 0"
                :indeterminate="aiStore.pullProgress?.percentage == null"
                color="primary"
                height="8"
                rounded
                striped
                class="mb-2"
              />

              <div
                class="d-flex align-center justify-space-between text-caption text-medium-emphasis"
              >
                <span>{{
                  aiStore.pullProgress?.status || 'Processando download no Ollama...'
                }}</span>
                <span v-if="aiStore.pullProgress?.completed && aiStore.pullProgress?.total">
                  {{ formatDecimalBytes(aiStore.pullProgress.completed) }} de
                  {{ formatDecimalBytes(aiStore.pullProgress.total) }}
                </span>
              </div>

              <div v-if="aiStore.pullProgress?.error" class="text-caption text-error mt-2">
                {{ aiStore.pullProgress.error }}
              </div>

              <div class="d-flex justify-end mt-2">
                <v-btn
                  color="error"
                  variant="text"
                  size="x-small"
                  prepend-icon="mdi-close-circle-outline"
                  @click="aiStore.cancelOllamaPull()"
                >
                  Cancelar download
                </v-btn>
              </div>
            </v-card>
          </v-col>

          <!-- Alerta quando o modelo selecionado/digitado não está instalado -->
          <v-col v-if="shouldShowInstallAlert" cols="12">
            <v-alert
              type="warning"
              variant="tonal"
              density="compact"
              class="mb-3"
              icon="mdi-cloud-download-outline"
            >
              <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                <div>
                  <div class="font-weight-bold text-body-2">
                    O modelo "{{ form.ollamaModel }}" não foi detectado no seu Ollama.
                  </div>
                  <div class="text-caption">
                    Deseja baixar e instalar este modelo na sua máquina agora?
                  </div>
                </div>
                <v-btn
                  color="warning"
                  variant="flat"
                  size="small"
                  :loading="aiStore.pullingModelName === form.ollamaModel"
                  :disabled="!!aiStore.pullingModelName"
                  @click="handleInstallModel(form.ollamaModel)"
                >
                  <v-icon start size="16">mdi-download</v-icon>
                  Instalar "{{ form.ollamaModel }}"
                </v-btn>
              </div>
            </v-alert>
          </v-col>

          <!-- Seção de Modelos Recomendados com Ações de Instalação e Seleção -->
          <v-col cols="12">
            <div class="d-flex align-center justify-space-between mb-2">
              <span class="text-subtitle-2 font-weight-bold text-medium-emphasis">
                <v-icon size="16" start color="primary">mdi-star-outline</v-icon>
                Modelos Recomendados para o NetMonitor
              </span>
              <div class="d-flex align-center ga-1">
                <v-btn
                  variant="tonal"
                  size="x-small"
                  color="primary"
                  prepend-icon="mdi-magnify"
                  @click="openModelSearchDialog"
                >
                  Buscar no Catálogo
                </v-btn>
                <v-btn
                  variant="text"
                  size="x-small"
                  color="primary"
                  @click="showRecommendedDetails = !showRecommendedDetails"
                >
                  {{ showRecommendedDetails ? 'Ocultar detalhes' : 'Ver detalhes e recursos' }}
                  <v-icon end size="14">
                    {{ showRecommendedDetails ? 'mdi-chevron-up' : 'mdi-chevron-down' }}
                  </v-icon>
                </v-btn>
              </div>
            </div>

            <v-row dense>
              <v-col
                v-for="rec in aiStore.recommendedOllamaModels"
                :key="rec.name"
                cols="12"
                :md="showRecommendedDetails ? 6 : 4"
              >
                <v-card
                  variant="outlined"
                  class="pa-2 fill-height d-flex flex-column"
                  :color="isModelActive(rec.name) ? 'primary' : undefined"
                >
                  <div class="d-flex align-center justify-space-between mb-1">
                    <span class="font-weight-bold text-body-2 text-truncate" :title="rec.name">
                      {{ rec.name }}
                    </span>
                    <div class="d-flex align-center ga-1">
                      <v-chip size="x-small" variant="tonal" color="default">
                        {{ rec.parameterSize }}
                      </v-chip>
                      <v-chip
                        v-if="rec.toolCallingOptimized"
                        size="x-small"
                        color="warning"
                        variant="tonal"
                        title="Otimizado para execução de ferramentas e diagnósticos"
                      >
                        <v-icon start size="12">mdi-tools</v-icon>
                        Tool Use
                      </v-chip>
                    </div>
                  </div>

                  <p
                    v-if="showRecommendedDetails"
                    class="text-caption text-medium-emphasis mb-2 flex-grow-1"
                  >
                    {{ rec.description }}
                  </p>

                  <div class="d-flex align-center justify-space-between mt-auto pt-1">
                    <v-chip v-if="rec.isInstalled" size="x-small" color="success" variant="tonal">
                      <v-icon start size="12">mdi-check</v-icon>
                      Instalado
                    </v-chip>
                    <span v-else class="text-caption text-disabled">Não instalado</span>

                    <div class="d-flex ga-1">
                      <template v-if="rec.isInstalled">
                        <v-chip
                          v-if="isModelActive(rec.name)"
                          size="x-small"
                          color="primary"
                          variant="flat"
                        >
                          Em uso
                        </v-chip>
                        <v-btn
                          v-else
                          size="x-small"
                          variant="outlined"
                          color="primary"
                          @click="selectModel(rec.name)"
                        >
                          Selecionar
                        </v-btn>
                      </template>
                      <template v-else>
                        <v-btn
                          size="x-small"
                          variant="flat"
                          color="primary"
                          prepend-icon="mdi-download"
                          :loading="aiStore.pullingModelName === rec.name"
                          :disabled="!!aiStore.pullingModelName"
                          @click="handleInstallModel(rec.name)"
                        >
                          Instalar
                        </v-btn>
                      </template>
                    </div>
                  </div>
                </v-card>
              </v-col>
            </v-row>
          </v-col>
        </template>

        <!-- Permissões de Ferramentas -->
        <v-col cols="12">
          <v-checkbox
            v-model="form.allowActiveTools"
            color="primary"
            density="compact"
            hide-details
            label="Permitir execução de ferramentas ativas de rede (Ping, Traceroute, Scan de Portas e Playbooks)"
          />
        </v-col>
      </v-row>

      <!-- Feedback de Teste de Conexão -->
      <v-alert
        v-if="aiStore.testResult"
        :type="aiStore.testResult.success ? 'success' : 'error'"
        variant="tonal"
        density="compact"
        class="mt-3"
        closable
        @click:close="aiStore.testResult = null"
      >
        {{ aiStore.testResult.message }}
        <span v-if="aiStore.testResult.success">
          (Latência: {{ aiStore.testResult.latencyMs }}ms)
        </span>
      </v-alert>
    </v-card-text>

    <v-card-actions class="pa-4 pt-0 justify-space-between">
      <v-btn
        variant="outlined"
        color="primary"
        size="small"
        :loading="aiStore.testingConnection"
        @click="handleTestConnection"
      >
        <v-icon start size="16">mdi-lan-connect</v-icon>
        Testar Conexão
      </v-btn>

      <v-btn
        color="primary"
        variant="flat"
        size="small"
        :loading="aiStore.savingSettings"
        @click="handleSave"
      >
        <v-icon start size="16">mdi-content-save-outline</v-icon>
        Salvar Configurações
      </v-btn>
    </v-card-actions>
  </v-card>

  <!-- Diálogo Modal de Catálogo e Busca de Modelos -->
  <AiModelSearchDialog
    v-model="showModelSearchDialog"
    :driver="currentDriver"
    :current-model="currentModelForDriver"
    :api-key="currentApiKeyForDriver"
    :base-url="currentBaseUrlForDriver"
    @select="handleModelSelectedFromDialog"
    @install="handleInstallModel"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useAiStore, type AiSettings } from '@/stores/ai'
import { formatDecimalBytes } from '@/utils/formatters'
import AiModelSearchDialog from './AiModelSearchDialog.vue'

const emit = defineEmits<{
  (e: 'saved', message: string, color?: string): void
}>()

const aiStore = useAiStore()
const showApiKey = ref(false)
const showRecommendedDetails = ref(false)
const showModelSearchDialog = ref(false)

const driverOptions = [
  { title: 'Ollama (Local / On-Premise)', value: 'ollama' },
  { title: 'OpenRouter (Multi-Model Gateway)', value: 'openrouter' },
  { title: 'OpenCode Go / Zen', value: 'opencode' },
]

const localSaveError = ref<string | null>(null)
const localSaveSuccess = ref<string | null>(null)

function dismissSaveError() {
  localSaveError.value = null
  aiStore.saveError = null
}

const form = reactive<AiSettings>({
  enabled: false,
  activeDriver: 'ollama',
  opencodeBaseUrl: 'https://opencode.ai/zen/v1',
  opencodeApiKey: '',
  opencodeModel: 'muse-spark-1.3-contributor-free',
  openrouterApiKey: '',
  openrouterModel: 'openrouter/free',
  ollamaBaseUrl: 'http://localhost:11434/v1',
  ollamaModel: 'llama3.2',
  allowActiveTools: true,
  requireToolConfirmation: false,
  customSystemPrompt: '',
})

interface ModelOption {
  id: string
  name: string
  isFree: boolean
  description?: string | null
  supportsTools?: boolean
}

interface OllamaOption {
  id: string
  name: string
  isInstalled: boolean
  isRecommended: boolean
  size?: string | null
  description?: string | null
  supportsTools?: boolean
}

// Extrai string pura (slug ou nome) de qualquer modelo recebido (seja string ou objeto com id/name/value)
function extractModelId(val: unknown): string {
  if (!val) return ''
  if (typeof val === 'string') return val.trim()
  if (typeof val === 'object' && val !== null) {
    const obj = val as Record<string, unknown>
    if (typeof obj.id === 'string' && obj.id.trim()) {
      return obj.id.trim()
    }
    if (typeof obj.name === 'string' && obj.name.trim()) {
      return obj.name.trim()
    }
    if (typeof obj.value === 'string' && obj.value.trim()) {
      return obj.value.trim()
    }
  }
  return String(val).trim()
}

// Filtro inteligente para busca nos campos autocomplete/combobox
function customModelFilter(value: string, query: string, item?: any): boolean {
  if (!query) return true
  const q = query.trim().toLowerCase()
  const raw = item?.raw
  if (!raw) return (value || '').toLowerCase().includes(q)
  const id = (raw.id || raw.name || '').toLowerCase()
  const name = (raw.name || '').toLowerCase()
  const desc = (raw.description || '').toLowerCase()
  return id.includes(q) || name.includes(q) || desc.includes(q)
}

const openRouterModelOptions = computed<ModelOption[]>(() => {
  if (aiStore.openrouterModels.length > 0) {
    return aiStore.openrouterModels.map((m) => ({
      id: m.id,
      name: m.name || m.id,
      isFree: m.isFree,
      description: m.description,
      supportsTools: m.supportsTools ?? undefined,
    }))
  }
  return [
    {
      id: 'openrouter/free',
      name: 'Free Models Router (Automático Gratuito)',
      isFree: true,
      description: 'Roteia para o melhor modelo gratuito',
      supportsTools: true,
    },
    {
      id: 'google/gemma-4-31b-it:free',
      name: 'Google: Gemma 4 31B (Gratuito)',
      isFree: true,
      description: 'Excelente raciocínio e suporte gratuito',
      supportsTools: true,
    },
    {
      id: 'qwen/qwen3.8-27b:free',
      name: 'Qwen: Qwen 3.8 27B (Gratuito)',
      isFree: true,
      description: 'Alta performance em código e raciocínio técnico',
      supportsTools: true,
    },
    {
      id: 'nvidia/nemotron-3.5-lightning:free',
      name: 'NVIDIA: Nemotron 3.5 Lightning (Gratuito)',
      isFree: true,
      description: 'Velocidade e precisão para diagnósticos de rede',
      supportsTools: true,
    },
    {
      id: 'meta-llama/llama-3.3-70b-instruct',
      name: 'Meta: Llama 3.3 70B Instruct (Padrão / Créditos)',
      isFree: false,
      description: 'Modelo recomendado da Meta para alta complexidade',
      supportsTools: true,
    },
    {
      id: 'openai/gpt-4o-mini',
      name: 'OpenAI: GPT-4o Mini',
      isFree: false,
      description: 'Rápido, econômico e altamente capaz',
      supportsTools: true,
    },
    {
      id: 'anthropic/claude-3.5-sonnet',
      name: 'Anthropic: Claude 3.5 Sonnet',
      isFree: false,
      description: 'Estado da arte em raciocínio e engenharia',
      supportsTools: true,
    },
    {
      id: 'deepseek/deepseek-chat',
      name: 'DeepSeek: DeepSeek Chat (V3)',
      isFree: false,
      description: 'Excelente custo-benefício em análise de sistemas',
      supportsTools: true,
    },
  ]
})

const opencodeModelOptions = computed<ModelOption[]>(() => {
  if (aiStore.opencodeModels.length > 0) {
    return aiStore.opencodeModels.map((m) => ({
      id: m.id,
      name: m.name || m.id,
      isFree: m.isFree,
      description: m.description,
      supportsTools: m.supportsTools ?? true,
    }))
  }
  return [
    {
      id: 'muse-spark-1.3-contributor-free',
      name: 'Muse Spark 1.3 Contributor (Gratuito)',
      isFree: true,
      description: 'Modelo recomendado gratuito com excelente raciocínio',
      supportsTools: true,
    },
    {
      id: 'mimo-v2.6-flash-free',
      name: 'Mimo v2.6 Flash (Gratuito)',
      isFree: true,
      description: 'Modelo ultra-rápido gratuito otimizado para chamadas e código',
      supportsTools: true,
    },
    {
      id: 'jev-1.13-free',
      name: 'Jev 1.13 (Gratuito)',
      isFree: true,
      description: 'Modelo gratuito de uso geral para diagnósticos',
      supportsTools: true,
    },
    {
      id: 'deepseek-v4.1-flash',
      name: 'DeepSeek v4.1 Flash',
      isFree: false,
      description: 'Alta performance em análise de rede e scripts',
      supportsTools: true,
    },
    {
      id: 'gemini-3.8-flash',
      name: 'Gemini 3.8 Flash',
      isFree: false,
      description: 'Latência reduzida e raciocínio avançado',
      supportsTools: true,
    },
    {
      id: 'glm-5.3-flash',
      name: 'GLM 5.3 Flash',
      isFree: false,
      description: 'Excelente para tarefas de diagnóstico e suporte',
      supportsTools: true,
    },
    {
      id: 'gpt-6-astra',
      name: 'GPT-6 Astra',
      isFree: false,
      description: 'Modelo de ponta para análise profunda e playbooks',
      supportsTools: true,
    },
    {
      id: 'qwen3.8-flash',
      name: 'Qwen 3.8 Flash',
      isFree: false,
      description: 'Modelo rápido para consultas e suporte operacional',
      supportsTools: true,
    },
  ]
})

const ollamaModelOptions = computed<OllamaOption[]>(() => {
  const map = new Map<string, OllamaOption>()

  for (const inst of aiStore.installedOllamaModels) {
    map.set(inst.name, {
      id: inst.name,
      name: inst.name,
      isInstalled: true,
      isRecommended: false,
      size: inst.size ? formatDecimalBytes(inst.size) : null,
      description: `Modelo instalado localmente (${inst.parameterSize || 'tamanho não informado'})`,
      supportsTools:
        inst.name.toLowerCase().includes('tool') || inst.name.toLowerCase().includes('groq'),
    })
  }

  for (const rec of aiStore.recommendedOllamaModels) {
    const existing = map.get(rec.name)
    if (existing) {
      existing.isRecommended = true
      existing.supportsTools = rec.toolCallingOptimized
      if (rec.description) existing.description = rec.description
    } else {
      map.set(rec.name, {
        id: rec.name,
        name: rec.name,
        isInstalled: rec.isInstalled,
        isRecommended: true,
        supportsTools: rec.toolCallingOptimized,
        description: rec.description,
      })
    }
  }

  if (form.ollamaModel && form.ollamaModel.trim() && !map.has(form.ollamaModel.trim())) {
    const custom = form.ollamaModel.trim()
    map.set(custom, {
      id: custom,
      name: custom,
      isInstalled: isModelInstalled(custom),
      isRecommended: false,
      description: 'Modelo customizado',
    })
  }

  return Array.from(map.values())
})

const currentDriver = computed<'openrouter' | 'opencode' | 'ollama'>(() => {
  if (form.activeDriver === 'openrouter') return 'openrouter'
  if (form.activeDriver === 'opencode') return 'opencode'
  return 'ollama'
})

const currentModelForDriver = computed<string>(() => {
  if (form.activeDriver === 'openrouter') return form.openrouterModel || ''
  if (form.activeDriver === 'opencode') return form.opencodeModel || ''
  if (form.activeDriver === 'ollama') return form.ollamaModel || ''
  return ''
})

const currentApiKeyForDriver = computed(() => {
  if (form.activeDriver === 'openrouter') return form.openrouterApiKey
  if (form.activeDriver === 'opencode') return form.opencodeApiKey
  return null
})

const currentBaseUrlForDriver = computed(() => {
  if (form.activeDriver === 'opencode') return form.opencodeBaseUrl
  if (form.activeDriver === 'ollama') return form.ollamaBaseUrl
  return null
})

function openModelSearchDialog() {
  showModelSearchDialog.value = true
}

function handleModelSelectedFromDialog(modelId: string) {
  const cleanId = extractModelId(modelId)
  if (form.activeDriver === 'openrouter') {
    form.openrouterModel = cleanId
  } else if (form.activeDriver === 'opencode') {
    form.opencodeModel = cleanId
  } else if (form.activeDriver === 'ollama') {
    form.ollamaModel = cleanId
  }
}

function isModelInstalled(modelName: string): boolean {
  if (!modelName) return false
  const target = modelName.trim().toLowerCase()
  return aiStore.installedOllamaModels.some((m) => {
    const n = m.name.toLowerCase()
    return (
      n === target ||
      n === `${target}:latest` ||
      (target.endsWith(':latest') && n === target.slice(0, -7)) ||
      n.startsWith(`${target}:`)
    )
  })
}

const isCurrentModelInstalled = computed(() => {
  if (!form.ollamaModel) return true
  return isModelInstalled(form.ollamaModel)
})

const shouldShowInstallAlert = computed(() => {
  if (form.activeDriver !== 'ollama' || !form.ollamaModel || !form.ollamaModel.trim()) {
    return false
  }
  // Se estiver baixando este modelo atualmente, não mostra o alerta (mostra o card de progresso)
  if (aiStore.pullingModelName === form.ollamaModel.trim()) {
    return false
  }
  return !isCurrentModelInstalled.value
})

function isModelActive(modelName: string): boolean {
  return form.ollamaModel?.trim().toLowerCase() === modelName.trim().toLowerCase()
}

function selectModel(modelName: string) {
  form.ollamaModel = modelName
}

async function refreshOllamaModels() {
  await aiStore.loadOllamaModels(form.ollamaBaseUrl)
}

function handleOllamaBaseUrlBlur() {
  if (form.activeDriver === 'ollama' && form.ollamaBaseUrl) {
    refreshOllamaModels()
  }
}

async function handleInstallModel(modelName?: string | null) {
  const target = (modelName || form.ollamaModel || '').trim()
  if (!target) return

  aiStore.pullError = null
  await aiStore.pullOllamaModel(target, form.ollamaBaseUrl, () => {
    form.ollamaModel = target
    emit('saved', `Modelo ${target} baixado e instalado com sucesso no Ollama!`)
  })
}

async function refreshOpencodeModels() {
  await aiStore.loadOpencodeModels(form.opencodeApiKey, form.opencodeBaseUrl)
}

function handleOpencodeApiKeyBlur() {
  if (form.activeDriver === 'opencode' && form.opencodeApiKey) {
    refreshOpencodeModels()
  }
}

async function refreshOpenrouterModels() {
  await aiStore.loadOpenrouterModels(form.openrouterApiKey)
}

function handleOpenrouterApiKeyBlur() {
  if (form.activeDriver === 'openrouter' && form.openrouterApiKey) {
    refreshOpenrouterModels()
  }
}

watch(
  () => form.activeDriver,
  (newDriver) => {
    if (newDriver === 'ollama') {
      refreshOllamaModels()
    } else if (newDriver === 'opencode') {
      if (!form.opencodeBaseUrl) form.opencodeBaseUrl = 'https://opencode.ai/zen/v1'
      refreshOpencodeModels()
    } else if (newDriver === 'openrouter') {
      if (
        !form.openrouterModel ||
        form.openrouterModel === 'meta-llama/llama-3.3-70b-instruct:free'
      ) {
        form.openrouterModel = 'openrouter/free'
      }
      refreshOpenrouterModels()
    }
  }
)

onMounted(async () => {
  await aiStore.loadSettings()
  if (aiStore.settings) {
    Object.assign(form, aiStore.settings)
  }
  if (!form.opencodeBaseUrl) {
    form.opencodeBaseUrl = 'https://opencode.ai/zen/v1'
  }
  if (!form.openrouterModel || form.openrouterModel === 'meta-llama/llama-3.3-70b-instruct:free') {
    form.openrouterModel = 'openrouter/free'
  }
  if (form.activeDriver === 'ollama') {
    await refreshOllamaModels()
  } else if (form.activeDriver === 'opencode') {
    await refreshOpencodeModels()
  } else if (form.activeDriver === 'openrouter') {
    await refreshOpenrouterModels()
  }
})

async function handleTestConnection() {
  form.opencodeModel = extractModelId(form.opencodeModel)
  form.openrouterModel = extractModelId(form.openrouterModel)
  form.ollamaModel = extractModelId(form.ollamaModel)

  let baseUrl: string | null = null
  let apiKey: string | null = null
  let model = ''

  if (form.activeDriver === 'opencode') {
    baseUrl = form.opencodeBaseUrl || 'https://opencode.ai/zen/v1'
    apiKey = form.opencodeApiKey || null
    model = form.opencodeModel || 'muse-spark-1.3-contributor-free'
  } else if (form.activeDriver === 'openrouter') {
    apiKey = form.openrouterApiKey || null
    model = form.openrouterModel || 'openrouter/free'
    if (model === 'meta-llama/llama-3.3-70b-instruct:free') {
      model = 'openrouter/free'
      form.openrouterModel = 'openrouter/free'
    }
  } else if (form.activeDriver === 'ollama') {
    baseUrl = form.ollamaBaseUrl || 'http://localhost:11434/v1'
    model = form.ollamaModel || 'llama3.2'
  }

  await aiStore.testConnection({
    driver: form.activeDriver,
    baseUrl,
    apiKey,
    model: extractModelId(model),
  })
}

async function handleSave() {
  localSaveError.value = null
  localSaveSuccess.value = null

  form.opencodeModel = extractModelId(form.opencodeModel)
  form.openrouterModel = extractModelId(form.openrouterModel)
  form.ollamaModel = extractModelId(form.ollamaModel)

  if (form.activeDriver === 'opencode' && !form.opencodeBaseUrl) {
    form.opencodeBaseUrl = 'https://opencode.ai/zen/v1'
  }
  if (
    form.activeDriver === 'openrouter' &&
    (!form.openrouterModel || form.openrouterModel === 'meta-llama/llama-3.3-70b-instruct:free')
  ) {
    form.openrouterModel = 'openrouter/free'
  }

  const res = await aiStore.saveSettings({ ...form })
  if (res.success) {
    localSaveSuccess.value = 'Configurações de IA salvas com sucesso!'
    emit('saved', 'Configurações de IA salvas com sucesso!', 'success')
  } else {
    const errorMsg = res.message || aiStore.saveError || 'Erro ao salvar configurações de IA'
    localSaveError.value = errorMsg
    emit('saved', errorMsg, 'error')
  }
}
</script>

<template>
  <div>
    <v-select
      :model-value="form.provider"
      :items="deciderStore.providers"
      item-title="label"
      item-value="id"
      label="Provedor"
      variant="outlined"
      density="compact"
      hide-details
      :prepend-inner-icon="providerIcon(provider)"
      @update:model-value="chooseProvider"
    >
      <template #item="{ props: itemProps, item }">
        <v-list-item
          v-bind="itemProps"
          :title="item.label"
          :subtitle="item.description"
          :prepend-icon="providerIcon(item)"
        />
      </template>
    </v-select>
    <div v-if="provider" class="text-body-2 mt-1">{{ provider.description }}</div>

    <v-row dense class="mt-2">
      <v-col v-if="form.provider === 'custom'" cols="12">
        <div class="text-body-2 mb-1">Formato da API</div>
        <v-btn-toggle
          v-model="form.customProtocol"
          mandatory
          color="primary"
          variant="outlined"
          density="compact"
          divided
        >
          <v-btn value="systemone">System One (TypeSafe)</v-btn>
          <v-btn value="openai">Decisions (OpenAI)</v-btn>
        </v-btn-toggle>
      </v-col>

      <v-col v-if="showUrlField" cols="12" md="6">
        <v-text-field
          v-model="form.baseUrl"
          :label="managesModels ? 'URL do Ollama' : 'URL da API'"
          :placeholder="provider?.defaultBaseUrl || 'https://…'"
          :hint="urlHint"
          :color="customUrl ? 'warning' : undefined"
          :base-color="customUrl ? 'warning' : undefined"
          persistent-hint
          variant="outlined"
          density="compact"
          prepend-inner-icon="mdi-link-variant"
          :append-inner-icon="urlAction?.icon"
          :loading="managesModels && deciderStore.loadingModels"
          @blur="emit('refresh')"
          @keydown.enter.prevent="emit('refresh')"
          @click:append-inner="urlAction?.run()"
        >
          <template v-if="customUrl" #message>
            <span class="text-warning">{{ urlHint }}</span>
          </template>
        </v-text-field>
      </v-col>

      <v-col cols="12" md="6">
        <v-text-field
          v-model="form.apiKey"
          :type="showKey ? 'text' : 'password'"
          :label="provider?.requiresKey ? 'Chave de acesso' : 'Chave de acesso (opcional)'"
          :hint="keyHint"
          persistent-hint
          autocomplete="off"
          variant="outlined"
          density="compact"
          prepend-inner-icon="mdi-key-variant"
          :append-inner-icon="showKey ? 'mdi-eye-off' : 'mdi-eye'"
          @click:append-inner="showKey = !showKey"
        />
        <a
          v-if="provider?.keyUrl"
          :href="provider.keyUrl"
          target="_blank"
          rel="noopener"
          class="d-inline-block text-body-2 text-primary mt-1 ms-4"
        >
          Criar uma chave em {{ provider.label }}
          <v-icon size="14">mdi-open-in-new</v-icon>
        </a>
      </v-col>
      <v-col v-if="!showUrlField" cols="12" md="6" class="d-flex flex-column justify-center">
        <div class="text-body-2">
          <v-icon size="16" color="success" class="me-1">mdi-check-circle</v-icon>
          Endereço da API já configurado: <code>{{ provider?.defaultBaseUrl }}</code>
        </div>
        <a href="#" class="text-body-2 text-primary mt-1 ms-5" @click.prevent="editingUrl = true">
          Passar por um proxy ou gateway
        </a>
      </v-col>
      <v-col v-if="provider?.requiresAccount" cols="12" md="6">
        <v-text-field
          v-model="form.accountId"
          label="ID da conta"
          hint="No painel da Cloudflare: Workers AI, ou a barra lateral da conta."
          persistent-hint
          variant="outlined"
          density="compact"
          prepend-inner-icon="mdi-identifier"
        />
      </v-col>
    </v-row>

    <!-- Estado -->
    <div class="mt-3">
      <template v-if="managesModels">
        <v-chip
          v-if="deciderStore.models?.online"
          color="success"
          variant="tonal"
          prepend-icon="mdi-check-circle"
        >
          Conectado — {{ deciderStore.models.installed.length }} modelo(s) instalado(s)
        </v-chip>
        <v-alert v-else-if="deciderStore.models" type="error" variant="tonal" density="compact">
          {{ deciderStore.models.errorMessage }}
          <div class="mt-1">
            Nesta máquina, o serviço <code>ollama</code> sobe junto com o
            <code>docker compose up</code> (é o mesmo do chat). Com a URL padrão, o servidor o
            procura pelo nome do serviço, em <code>127.0.0.1:11434</code> e em
            <code>host.docker.internal:11434</code>. Em outra máquina, use o endereço dela (porta
            11434).
          </div>
        </v-alert>
      </template>
      <template v-else>
        <v-chip
          v-if="missing"
          color="warning"
          variant="tonal"
          prepend-icon="mdi-alert-circle-outline"
        >
          {{ missing }}
        </v-chip>
        <div v-else class="d-flex flex-wrap align-center ga-2">
          <v-btn
            color="primary"
            variant="tonal"
            prepend-icon="mdi-connection"
            :loading="deciderStore.testing"
            @click="emit('test')"
          >
            Testar conexão
          </v-btn>
          <v-chip
            v-if="deciderStore.testResult"
            :color="deciderStore.testResult.success ? 'success' : 'error'"
            variant="tonal"
            :prepend-icon="
              deciderStore.testResult.success ? 'mdi-check-circle' : 'mdi-alert-circle'
            "
          >
            {{ connectionSummary }}
          </v-chip>
        </div>
        <div
          v-if="deciderStore.testResult && !deciderStore.testResult.success"
          class="text-body-2 text-error mt-1"
        >
          {{ deciderStore.testResult.message }}
        </div>
        <v-alert type="info" variant="tonal" density="compact" class="mt-2 text-body-2">
          Na nuvem, o texto de cada decisão — perguntas do chat, logs, nomes e IPs de dispositivos —
          vai para {{ destination }}, e o uso é cobrado na sua conta (por token de entrada). Para
          nada sair da rede, use o Ollama.
        </v-alert>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { AiDeciderSettings } from '@/bindings/AiDeciderSettings'
import type { DeciderProviderView } from '@/bindings/DeciderProviderView'
import { defaultDeciderSettings, useAiDeciderStore } from '@/stores/aiDecider'
import { formatLatency } from '@/utils/formatters'
import { formForProvider, providerMissing, usesCustomUrl } from '@/utils/decider'

const form = defineModel<AiDeciderSettings>('form', { required: true })
const emit = defineEmits<{ refresh: []; test: [] }>()

const deciderStore = useAiDeciderStore()
const showKey = ref(false)
const editingUrl = ref(false)
/** URL digitada para cada provedor nesta tela, para voltar a ela ao trocar de novo. */
const typedUrls = new Map<AiDeciderSettings['provider'], string>()

const provider = computed(() => deciderStore.provider(form.value.provider))
const managesModels = computed(() => provider.value?.managesModels ?? true)
const missing = computed(() => providerMissing(form.value, provider.value))

/** Para onde os textos vão, no aviso de privacidade. */
const destination = computed(() =>
  form.value.provider === 'custom'
    ? 'o serviço configurado'
    : (provider.value?.label ?? 'o provedor')
)

/** O teste em uma linha: quem respondeu e em quanto tempo. */
const connectionSummary = computed(() => {
  const result = deciderStore.testResult
  if (!result) return ''
  if (!result.success) return 'Não conectou'
  return `Respondeu em ${formatLatency(result.latencyMs)}${result.model ? ` — ${result.model}` : ''}`
})

/** A URL foge do padrão do provedor (proxy, gateway — ou sobra de outro provedor). */
const customUrl = computed(() => usesCustomUrl(form.value, provider.value))

/**
 * O endereço de um provedor de nuvem conhecido já vem pronto: o campo só
 * aparece se o usuário pedir ou se a URL gravada não for a padrão.
 */
const showUrlField = computed(
  () =>
    managesModels.value || form.value.provider === 'custom' || editingUrl.value || customUrl.value
)

/** O botão dentro do campo: buscar o Ollama ou voltar ao endereço padrão. */
const urlAction = computed(() => {
  if (managesModels.value) return { icon: 'mdi-refresh', run: () => emit('refresh') }
  if (customUrl.value) return { icon: 'mdi-restore', run: restoreDefaultUrl }
  return null
})

const urlHint = computed(() => {
  if (managesModels.value)
    return 'Nesta máquina: deixe http://ollama:11434 — o servidor acha o Ollama do compose (o mesmo do chat). Em outra: o IP dela, porta 11434.'
  if (form.value.provider === 'custom')
    return 'Sem o caminho final: o NetMonitor acrescenta o do formato.'
  if (customUrl.value)
    return `Diferente do endereço de ${provider.value?.label} (${provider.value?.defaultBaseUrl}). Só mude para um proxy ou gateway — clique em ↺ para voltar ao padrão.`
  return 'Endereço de um proxy ou gateway que repasse para o provedor. Vazio, vale o padrão.'
})

function restoreDefaultUrl() {
  form.value = { ...form.value, baseUrl: provider.value?.defaultBaseUrl ?? '' }
  editingUrl.value = false
  deciderStore.testResult = null
}

const keyHint = computed(() => {
  if (form.value.apiKey === '********') return 'Gravada. Digite outra para trocar.'
  if (managesModels.value) return 'Só se o Ollama estiver atrás de um proxy que exige chave.'
  return 'Fica guardada no servidor e nunca volta inteira para a tela.'
})

function providerIcon(item: DeciderProviderView | null): string {
  if (!item) return 'mdi-server-network'
  return item.cloud ? 'mdi-cloud-outline' : 'mdi-server-network'
}

/** Troca de provedor: padrões do novo, sem levar chave nem conta do anterior. */
function chooseProvider(id: AiDeciderSettings['provider'] | null) {
  const next = id ? deciderStore.provider(id) : null
  if (!next || next.id === form.value.provider) return
  const previousTimeout = deciderStore.spec(form.value.model) ?? provider.value
  typedUrls.set(form.value.provider, form.value.baseUrl)
  form.value = formForProvider(
    form.value,
    previousTimeout,
    next,
    defaultDeciderSettings().timeoutMs,
    typedUrls.get(next.id)
  )
  showKey.value = false
  editingUrl.value = false
  // O resultado era do provedor anterior.
  deciderStore.testResult = null
  emit('refresh')
}
</script>

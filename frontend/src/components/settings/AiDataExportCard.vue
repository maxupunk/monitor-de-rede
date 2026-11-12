<template>
  <v-card v-if="auth.isAdmin" elevation="2" class="rounded-lg">
    <v-card-title class="font-weight-bold d-flex align-center flex-wrap ga-2">
      <v-icon color="primary">mdi-database-export-outline</v-icon>
      Dados do Assistente
    </v-card-title>
    <v-card-subtitle class="text-wrap">
      Baixe as conversas com a IA, os comandos que ela deu e o que os equipamentos responderam —
      para treinar um modelo e revisar onde ela errou e onde acertou. Inclui as conversas de todos
      os usuários.
    </v-card-subtitle>

    <v-card-text>
      <div class="text-subtitle-2 font-weight-bold mb-2">Período</div>
      <v-chip-group
        :model-value="periodKey(store.days)"
        mandatory
        selected-class="text-primary"
        :disabled="store.downloading"
        @update:model-value="choosePeriod"
      >
        <v-chip
          v-for="option in PERIODS"
          :key="option.label"
          :value="periodKey(option.days)"
          color="primary"
          variant="outlined"
          filter
        >
          {{ option.label }}
        </v-chip>
      </v-chip-group>

      <v-row dense class="mt-2">
        <v-col v-for="tile in tiles" :key="tile.label" cols="6" md="3">
          <v-sheet border rounded="lg" class="pa-3 h-100">
            <div class="d-flex align-center ga-2">
              <v-icon :color="tile.color" size="20">{{ tile.icon }}</v-icon>
              <span class="text-h6 font-weight-bold">
                <v-progress-circular
                  v-if="store.loading"
                  indeterminate
                  size="18"
                  width="2"
                  color="primary"
                />
                <template v-else>{{ formatCompactCount(tile.value) }}</template>
              </span>
            </div>
            <div class="text-body-2 text-high-emphasis">{{ tile.label }}</div>
          </v-sheet>
        </v-col>
      </v-row>
      <div v-if="range" class="text-body-2 text-high-emphasis mt-2">
        <v-icon size="16" class="me-1">mdi-calendar-range</v-icon>
        Conversas de {{ range }}
      </div>

      <div class="text-subtitle-2 font-weight-bold mt-5 mb-1">O que vai no pacote (.zip)</div>
      <v-list density="compact" class="py-0 bg-transparent">
        <v-list-item
          v-for="item in CONTENTS"
          :key="item.title"
          :title="item.title"
          :subtitle="item.subtitle"
          class="px-0"
        >
          <template #prepend>
            <v-icon :color="item.color">{{ item.icon }}</v-icon>
          </template>
        </v-list-item>
      </v-list>

      <v-alert type="warning" variant="tonal" density="compact" class="mt-4">
        O pacote traz dados da sua rede: perguntas, nomes de pessoas e equipamentos, endereços e
        saídas de comandos. A senha de acesso aos equipamentos não vai, mas a saída de um comando
        pode conter outros segredos (uma chave de Wi-Fi, por exemplo). Revise antes de enviar a um
        serviço externo.
      </v-alert>

      <v-alert v-if="store.error" type="error" variant="tonal" density="compact" class="mt-3">
        {{ store.error }}
      </v-alert>
    </v-card-text>

    <v-card-actions class="px-4 pb-4 flex-wrap ga-2">
      <span v-if="isEmpty" class="text-body-2 text-warning">
        Nada para exportar neste período — escolha um período maior.
      </span>
      <v-spacer />
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-download"
        :loading="store.downloading"
        :disabled="store.loading || isEmpty"
        @click="download"
      >
        Baixar pacote
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useAiDataExportStore, type AiDataExportPeriod } from '@/stores/aiDataExport'
import { countNoun, formatCompactCount, formatDate } from '@/utils/formatters'

const emit = defineEmits<{
  (e: 'saved', message: string, color?: string): void
}>()

const auth = useAuthStore()
const store = useAiDataExportStore()

const PERIODS: { label: string; days: AiDataExportPeriod }[] = [
  { label: '7 dias', days: 7 },
  { label: '30 dias', days: 30 },
  { label: '90 dias', days: 90 },
  { label: 'Tudo', days: null },
]

const CONTENTS = [
  {
    icon: 'mdi-forum-outline',
    color: 'primary',
    title: 'Conversas completas',
    subtitle: 'Cada pergunta, resposta e ferramenta usada, com onde houve erro.',
  },
  {
    icon: 'mdi-school-outline',
    color: 'success',
    title: 'Exemplos de treino',
    subtitle:
      'Separados em "sem falhas" e "com falhas", no formato de chat lido por OpenAI, Unsloth e Axolotl.',
  },
  {
    icon: 'mdi-console-line',
    color: 'info',
    title: 'Comandos em equipamentos',
    subtitle: 'O que a IA executou em cada equipamento e o que ele respondeu.',
  },
  {
    icon: 'mdi-file-document-outline',
    color: 'secondary',
    title: 'LEIAME e manifesto',
    subtitle: 'O que é cada arquivo e cada campo, e os totais do pacote.',
  },
]

const tiles = computed(() => [
  {
    label: 'Conversas',
    icon: 'mdi-forum-outline',
    color: 'primary',
    value: store.summary?.conversations,
  },
  {
    label: 'Mensagens',
    icon: 'mdi-message-text-outline',
    color: 'primary',
    value: store.summary?.messages,
  },
  {
    label: 'Pessoas',
    icon: 'mdi-account-multiple-outline',
    color: 'secondary',
    value: store.summary?.users,
  },
  {
    label: 'Comandos em equipamentos',
    icon: 'mdi-console-line',
    color: 'info',
    value: store.summary?.deviceRuns,
  },
])

const isEmpty = computed(
  () =>
    !store.loading &&
    store.summary !== null &&
    store.summary.conversations === 0 &&
    store.summary.deviceRuns === 0
)

const range = computed(() => {
  const summary = store.summary
  if (!summary?.oldest || !summary.newest) return ''
  const from = formatDate(summary.oldest)
  const to = formatDate(summary.newest)
  return from === to ? from : `${from} a ${to}`
})

/** O grupo de chips não aceita `null` como valor; "Tudo" vira `all`. */
function periodKey(days: AiDataExportPeriod): string {
  return days === null ? 'all' : String(days)
}

function choosePeriod(key: unknown) {
  const option = PERIODS.find((item) => periodKey(item.days) === key)
  if (option && option.days !== store.days) void store.setPeriod(option.days)
}

async function download() {
  if (!(await store.download())) return
  const total = store.summary?.conversations ?? 0
  emit('saved', `Pacote baixado com ${countNoun(total, 'conversa')}.`, 'success')
}

onMounted(() => {
  if (auth.isAdmin) void store.loadSummary()
})
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    max-width="900"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-item class="px-6 pt-5">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-shield-search</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Vulnerabilidades</v-card-title>
        <v-card-subtitle class="text-high-emphasis">
          {{ version ? version.tags.join(', ') : '' }} · {{ scanSentence(version?.scan) }}
        </v-card-subtitle>
        <template #append>
          <v-btn
            color="primary"
            variant="tonal"
            size="small"
            prepend-icon="mdi-refresh"
            :loading="scanning"
            @click="rescan"
          >
            Analisar de novo
          </v-btn>
        </template>
      </v-card-item>
      <v-card-text class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">
          {{ error }}
        </v-alert>
        <v-progress-linear
          v-if="loading"
          indeterminate
          color="primary"
          class="mb-3"
        ></v-progress-linear>
        <v-alert
          v-else-if="!findings.length"
          :type="emptyState.type"
          variant="tonal"
          density="compact"
        >
          {{ emptyState.text }}
        </v-alert>
        <v-table v-else density="compact">
          <thead>
            <tr>
              <th>Severidade</th>
              <th>Aviso</th>
              <th>Pacote</th>
              <th class="text-no-wrap">Instalado</th>
              <th class="text-no-wrap">Corrige em</th>
              <th v-if="canAdmin" class="text-end">Ação</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="finding in findings" :key="`${finding.advisory}-${finding.package}`">
              <td>
                <v-chip :color="SEVERITY_COLOR[finding.severity]" size="small" variant="flat" label>
                  {{ SEVERITY_LABEL[finding.severity] ?? finding.severity }}
                </v-chip>
                <v-chip
                  v-if="finding.excepted"
                  size="small"
                  color="info"
                  variant="tonal"
                  label
                  class="ml-1"
                >
                  Exceção
                </v-chip>
              </td>
              <td>
                <a
                  :href="`https://osv.dev/vulnerability/${finding.advisory}`"
                  target="_blank"
                  rel="noopener"
                  class="text-primary font-weight-bold"
                >
                  {{ finding.advisory }}
                </a>
                <div v-if="finding.summary" class="text-body-2">{{ finding.summary }}</div>
              </td>
              <td>{{ finding.package }}</td>
              <td>
                <code class="text-no-wrap">{{ finding.installed }}</code>
              </td>
              <td>
                <code v-if="finding.fixed" class="text-no-wrap">{{ finding.fixed }}</code>
                <span v-else>sem correção ainda</span>
              </td>
              <td v-if="canAdmin" class="text-end">
                <v-btn
                  v-if="!finding.excepted"
                  color="warning"
                  variant="tonal"
                  size="small"
                  prepend-icon="mdi-shield-check"
                  :loading="excepting === finding.advisory"
                  @click="except(finding)"
                >
                  Aceitar risco
                </v-btn>
              </td>
            </tr>
          </tbody>
        </v-table>
      </v-card-text>
      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" @click="emit('update:modelValue', false)"
          >Fechar</v-btn
        >
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { prompt } from '@/composables/useConfirm'
import type { RegistryFindingView } from '@/bindings/RegistryFindingView'
import type { RegistryVersionView } from '@/bindings/RegistryVersionView'
import { useRegistryStore } from '@/stores/registry'
import { SEVERITY_COLOR, SEVERITY_LABEL, scanSentence } from '@/utils/registryPresentation'

const props = defineProps<{
  modelValue: boolean
  version: RegistryVersionView | null
  /** A aplicação do repositório: a exceção vale só para ela. */
  applicationId: number | null
  canAdmin: boolean
}>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const registry = useRegistryStore()
const findings = ref<RegistryFindingView[]>([])
const loading = ref(false)
const scanning = ref(false)
const error = ref<string | null>(null)
const excepting = ref<string | null>(null)

/** Lista vazia só é "limpa" quando a análise terminou. */
const emptyState = computed((): { type: 'success' | 'info' | 'warning'; text: string } => {
  switch (props.version?.scan?.status) {
    case 'done':
      return {
        type: 'success',
        text: 'Nenhuma vulnerabilidade conhecida nos pacotes desta imagem.',
      }
    case 'pending':
      return { type: 'info', text: 'A análise ainda está em andamento.' }
    case 'no_sbom':
      return {
        type: 'warning',
        text: 'A imagem não trouxe lista de pacotes (SBOM), então não há o que comparar. Compile com SBOM para a análise valer.',
      }
    case 'failed':
      return {
        type: 'warning',
        text: 'A base de vulnerabilidades não respondeu. Use "Analisar de novo" quando ela voltar.',
      }
    default:
      return { type: 'info', text: 'Esta versão ainda não foi analisada.' }
  }
})

const DAYS_RULE = (value: string) => {
  const days = Number(value)
  return (Number.isInteger(days) && days >= 1 && days <= 365) || 'De 1 a 365 dias'
}

/** Aceita o risco de um aviso por um prazo, com motivo (fica na auditoria). */
async function except(finding: RegistryFindingView) {
  const version = props.version
  if (!version) return
  const reason = await prompt({
    title: `Aceitar o risco de ${finding.advisory}`,
    message: `O aviso deixa de bloquear o deploy${props.applicationId === null ? ' de todas as aplicações' : ' desta aplicação'}. Diga por quê: o motivo fica registrado.`,
    inputLabel: 'Motivo',
    placeholder: 'O pacote vulnerável não é usado em produção',
    confirmText: 'Continuar',
    icon: 'mdi-shield-check',
    iconColor: 'warning',
    rules: [(value) => value.trim().length >= 5 || 'Explique em poucas palavras'],
  })
  if (!reason) return
  const days = await prompt({
    title: 'Por quanto tempo?',
    message: 'Depois do prazo o aviso volta a valer e é bom revisar.',
    inputLabel: 'Dias',
    inputType: 'number',
    defaultValue: '30',
    confirmText: 'Aceitar risco',
    confirmColor: 'warning',
    icon: 'mdi-calendar-clock',
    rules: [DAYS_RULE],
  })
  if (!days) return
  excepting.value = finding.advisory
  error.value = null
  try {
    await registry.addException({
      applicationId: props.applicationId,
      advisory: finding.advisory,
      reason: reason.trim(),
      days: Number(days),
    })
    findings.value = await registry.findings(version.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível registrar a exceção'
  } finally {
    excepting.value = null
  }
}

async function load() {
  if (!props.version) return
  loading.value = true
  error.value = null
  try {
    findings.value = await registry.findings(props.version.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível ler as vulnerabilidades'
  } finally {
    loading.value = false
  }
}

async function rescan() {
  if (!props.version) return
  scanning.value = true
  error.value = null
  try {
    findings.value = await registry.rescan(props.version.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'A análise não concluiu'
  } finally {
    scanning.value = false
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) void load()
  }
)
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    :fullscreen="smAndDown"
    max-width="860"
    scrollable
    @update:model-value="(open) => !open && close()"
  >
    <v-card :rounded="smAndDown ? 0 : 'lg'">
      <v-card-title class="d-flex align-center ga-2 pt-4">
        <v-icon color="primary">mdi-calendar-clock</v-icon>
        <span class="flex-grow-1">{{
          task ? 'Editar tarefa agendada' : 'Nova tarefa agendada'
        }}</span>
        <v-btn
          icon="mdi-close"
          variant="text"
          size="small"
          aria-label="Fechar"
          @click="close"
        ></v-btn>
      </v-card-title>
      <v-divider></v-divider>

      <v-card-text>
        <div class="text-title-small font-weight-bold mb-2">O que fazer</div>
        <v-alert
          v-if="store.catalogError"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-3"
        >
          {{ store.catalogError }}
        </v-alert>
        <v-autocomplete
          :model-value="tool"
          :items="toolItems"
          :loading="store.catalogLoading"
          :custom-filter="matchTool"
          label="Ferramenta"
          placeholder="Ex.: ping, resumo, silenciar alerta, reiniciar container…"
          prepend-inner-icon="mdi-magnify"
          variant="outlined"
          density="comfortable"
          :autofocus="!task"
          @update:model-value="(value) => chooseTool(value as string | null)"
        >
          <template #item="{ props: itemProps, item }">
            <v-list-item v-bind="itemProps" :title="item.title" :subtitle="item.subtitle">
              <template #prepend>
                <v-icon :color="item.color">{{ item.icon }}</v-icon>
              </template>
            </v-list-item>
          </template>
        </v-autocomplete>

        <v-sheet v-if="selected" border rounded="lg" class="pa-3 mb-4">
          <div class="d-flex align-center flex-wrap ga-2 mb-1">
            <v-icon :color="selectedMeta.color">{{ selectedMeta.icon }}</v-icon>
            <span class="font-weight-bold">{{ selectedMeta.label }}</span>
            <v-chip :color="kindLook.color" size="x-small" variant="flat" label>
              <v-icon start size="14">{{ kindLook.icon }}</v-icon>
              {{ kindLook.label }}
            </v-chip>
            <v-chip v-if="selected.adminOnly" color="error" size="x-small" variant="flat" label>
              Só administrador
            </v-chip>
          </div>
          <p class="text-body-medium mb-0 description">{{ selected.description }}</p>
        </v-sheet>

        <template v-if="selected">
          <ToolArgumentsForm v-model="args" :schema="selected.parameters" />
          <v-divider class="my-4"></v-divider>
        </template>

        <CronScheduleField v-model="cron" />

        <v-divider class="my-4"></v-divider>
        <v-text-field
          :model-value="name"
          label="Nome"
          :placeholder="suggestedName || 'Ex.: Ping no gateway'"
          variant="outlined"
          density="comfortable"
          prepend-inner-icon="mdi-tag-outline"
          :hint="nameTouched ? undefined : 'Sugerido a partir da ferramenta e da agenda'"
          persistent-hint
          class="mb-2"
          @update:model-value="setName"
        ></v-text-field>
        <v-text-field
          v-model="description"
          label="Observação (opcional)"
          placeholder="Por que esta tarefa existe"
          variant="outlined"
          density="comfortable"
          prepend-inner-icon="mdi-note-text-outline"
          class="mb-1"
        ></v-text-field>
        <v-switch
          v-model="enabled"
          color="success"
          inset
          hide-details
          :label="enabled ? 'Ativa' : 'Pausada'"
        ></v-switch>

        <v-sheet border rounded="lg" class="pa-3 mt-4 review" aria-live="polite">
          <div class="d-flex align-center ga-2 mb-2">
            <v-icon color="primary">mdi-clipboard-check-outline</v-icon>
            <span class="font-weight-bold flex-grow-1">Conferência</span>
            <v-progress-circular
              v-if="previewLoading"
              indeterminate
              size="16"
              width="2"
              color="primary"
            ></v-progress-circular>
          </div>
          <template v-if="preview">
            <div v-if="preview.scheduleLabel" class="mb-1">
              <strong>Quando:</strong> {{ preview.scheduleLabel }}
            </div>
            <div v-if="preview.nextRuns.length > 0" class="mb-1">
              <strong>Próximas:</strong>
              {{ preview.nextRuns.map((at) => formatDateTime(at)).join(' · ') }}
            </div>
            <div v-if="preview.summary" class="mb-1">
              <strong>O que faz:</strong> {{ preview.summary }}
            </div>
            <div v-if="preview.risk" class="d-flex align-center flex-wrap ga-2 mb-1">
              <v-chip :color="risk.color" size="x-small" variant="flat" label>
                <v-icon start size="14">{{ risk.icon }}</v-icon>
                {{ risk.label }}
              </v-chip>
              <span class="text-body-small">{{ preview.riskReason }}</span>
            </div>
          </template>
          <p v-else class="text-body-medium mb-1">
            Escolha a ferramenta e a agenda para conferir o que vai rodar.
          </p>
          <p class="text-body-small mb-0 mt-2">
            <v-icon size="16" color="info">mdi-account-key-outline</v-icon>
            Roda sozinha, sem pedir confirmação, com as permissões de
            {{ auth.user?.fullName || auth.user?.email || 'quem salvar' }}. Quem salvar a tarefa por
            último passa a assiná-la.
          </p>
        </v-sheet>

        <v-alert
          v-if="visibleErrors.length > 0"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-3"
        >
          <ul class="errors">
            <li v-for="(error, index) in visibleErrors" :key="index">{{ error }}</li>
          </ul>
        </v-alert>
        <v-alert v-if="saveError" type="error" variant="tonal" density="compact" class="mt-3">
          {{ saveError }}
        </v-alert>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="px-4 py-3">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" @click="close">Cancelar</v-btn>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-content-save"
          :loading="saving"
          @click="save"
        >
          {{ task ? 'Salvar' : 'Agendar' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Cria ou edita uma tarefa: ferramenta do catálogo, parâmetros pelo contrato
 * dela, agenda e a conferência do backend (frase da agenda, próximas
 * execuções, o que a ação faz e o risco) enquanto o usuário monta.
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import type { ScheduledTask } from '@/bindings/ScheduledTask'
import type { ScheduledTaskInput } from '@/bindings/ScheduledTaskInput'
import type { ScheduledTaskPreview } from '@/bindings/ScheduledTaskPreview'
import { aiToolMeta } from '@/components/ai/aiToolMeta'
import { useAuthStore } from '@/stores/auth'
import { useScheduledTasksStore } from '@/stores/scheduledTasks'
import { aiRiskPresentation } from '@/utils/aiRisk'
import { formatDateTime } from '@/utils/formatters'
import { cronDraftErrors, fromCron } from '@/utils/scheduledTasks/cron'
import { TOOL_KIND, type ScheduledTaskTemplate } from '@/utils/scheduledTasks/presentation'
import { missingRequired, schemaFields } from '@/utils/scheduledTasks/schemaForm'
import CronScheduleField from './CronScheduleField.vue'
import ToolArgumentsForm from './ToolArgumentsForm.vue'

/** Espera depois da última mudança antes de pedir a conferência. */
const PREVIEW_DEBOUNCE_MS = 350

const props = defineProps<{
  modelValue: boolean
  task: ScheduledTask | null
  template?: ScheduledTaskTemplate | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: [task: ScheduledTask]
}>()

const { smAndDown } = useDisplay()
const store = useScheduledTasksStore()
const auth = useAuthStore()

const tool = ref<string | null>(null)
const args = ref<Record<string, unknown>>({})
const cron = ref('0 3 * * *')
const name = ref('')
const nameTouched = ref(false)
const description = ref('')
const enabled = ref(true)
const preview = ref<ScheduledTaskPreview | null>(null)
const previewLoading = ref(false)
const showErrors = ref(false)
const saving = ref(false)
const saveError = ref<string | null>(null)
let previewTimer: ReturnType<typeof setTimeout> | null = null
let previewSeq = 0

interface ToolItem {
  value: string
  title: string
  subtitle: string
  description: string
  icon: string
  color: string
  props: { disabled: boolean }
}

function normalize(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
}

/** Busca no rótulo, no nome, no grupo e na descrição — "reiniciar" acha o container. */
function matchTool(_value: string, query: string, item?: { raw: unknown }): boolean {
  const raw = item?.raw as ToolItem | undefined
  if (!raw) return false
  const wanted = normalize(query.trim())
  return [raw.title, raw.value, raw.subtitle, raw.description].some((text) =>
    normalize(text).includes(wanted)
  )
}

const toolItems = computed<ToolItem[]>(() =>
  store.catalog.map((item) => {
    const meta = aiToolMeta(item.name)
    const blocked = item.adminOnly && !auth.isAdmin
    return {
      value: item.name,
      title: meta.label,
      subtitle: `${item.groupLabel}${item.adminOnly ? ' · só administrador' : ''}`,
      description: item.description,
      icon: meta.icon,
      color: meta.color,
      props: { disabled: blocked },
    }
  })
)

const selected = computed(() => (tool.value ? store.toolOf(tool.value) : undefined))
const selectedMeta = computed(() => aiToolMeta(tool.value ?? ''))
const kindLook = computed(() => TOOL_KIND[selected.value?.kind ?? 'query'])
const risk = computed(() => aiRiskPresentation(preview.value?.risk ?? 'low'))

const suggestedName = computed(() => {
  if (!tool.value) return ''
  const when = preview.value?.scheduleLabel
  return when ? `${selectedMeta.value.label} — ${when}` : selectedMeta.value.label
})

watch(suggestedName, (suggestion) => {
  if (!nameTouched.value) name.value = suggestion
})

function setName(value: string) {
  name.value = value
  nameTouched.value = value.trim() !== ''
}

function chooseTool(value: string | null) {
  if (value === tool.value) return
  tool.value = value
  args.value = props.template?.tool === value ? { ...props.template.arguments } : {}
}

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    void store.fetchCatalog()
    const current = props.task
    const template = props.template ?? null
    tool.value = current?.tool ?? template?.tool ?? null
    args.value = { ...(current?.arguments ?? template?.arguments ?? {}) }
    cron.value = current?.cron ?? template?.cron ?? '0 3 * * *'
    name.value = current?.name ?? template?.name ?? ''
    nameTouched.value = Boolean(current || template)
    description.value = current?.description ?? ''
    enabled.value = current?.enabled ?? true
    preview.value = null
    showErrors.value = false
    saveError.value = null
  },
  { immediate: true }
)

async function refreshPreview() {
  if (!tool.value) {
    preview.value = null
    return
  }
  const seq = ++previewSeq
  previewLoading.value = true
  try {
    const result = await store.preview({
      tool: tool.value,
      arguments: args.value,
      cron: cron.value,
    })
    if (seq === previewSeq) preview.value = result
  } catch {
    // Sem conferência agora: o salvar confere de novo e mostra o erro.
  } finally {
    if (seq === previewSeq) previewLoading.value = false
  }
}

watch(
  [tool, args, cron, () => props.modelValue],
  () => {
    if (!props.modelValue) return
    if (previewTimer) clearTimeout(previewTimer)
    previewTimer = setTimeout(() => void refreshPreview(), PREVIEW_DEBOUNCE_MS)
  },
  { deep: true, immediate: true }
)

onBeforeUnmount(() => {
  if (previewTimer) clearTimeout(previewTimer)
})

const localErrors = computed(() => {
  const errors: string[] = []
  if (!tool.value) errors.push('Escolha o que a tarefa vai fazer')
  if (selected.value)
    errors.push(...missingRequired(schemaFields(selected.value.parameters), args.value))
  errors.push(...cronDraftErrors(fromCron(cron.value)))
  if (!name.value.trim()) errors.push('Dê um nome à tarefa')
  return errors
})

/** Os da tela e os do backend, sem repetir; só depois de tentar salvar ou quando a conferência acusa. */
const visibleErrors = computed(() => {
  const remote = preview.value?.errors ?? []
  const all = showErrors.value ? [...localErrors.value, ...remote] : remote
  return [...new Set(all)]
})

const input = computed<ScheduledTaskInput>(() => ({
  name: name.value.trim(),
  description: description.value.trim() || undefined,
  tool: tool.value ?? '',
  arguments: args.value,
  cron: cron.value,
  enabled: enabled.value,
}))

async function save() {
  showErrors.value = true
  if (localErrors.value.length > 0) return
  saving.value = true
  saveError.value = null
  try {
    const saved = props.task
      ? await store.update(props.task.id, input.value)
      : await store.create(input.value)
    emit('saved', saved)
    emit('update:modelValue', false)
  } catch (err: unknown) {
    saveError.value =
      err instanceof Error && err.message ? err.message : 'Não foi possível salvar a tarefa.'
  } finally {
    saving.value = false
  }
}

function close() {
  emit('update:modelValue', false)
}
</script>

<style scoped>
.description,
.review {
  color: rgb(var(--v-theme-on-surface));
}
.errors {
  padding-left: 18px;
  margin: 0;
}
</style>

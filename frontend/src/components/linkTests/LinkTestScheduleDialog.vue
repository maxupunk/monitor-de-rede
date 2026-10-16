<template>
  <v-dialog
    :model-value="modelValue"
    :fullscreen="smAndDown"
    max-width="820"
    scrollable
    @update:model-value="(open) => !open && close()"
  >
    <v-card :rounded="smAndDown ? 0 : 'lg'">
      <v-card-title class="d-flex align-center ga-2 pt-4">
        <v-icon color="primary">mdi-calendar-clock</v-icon>
        <span class="flex-grow-1">{{ schedule ? 'Editar agendamento' : 'Novo agendamento' }}</span>
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
        <v-text-field
          v-model="name"
          label="Nome"
          placeholder="Ex.: Rádio da torre — madrugada"
          variant="outlined"
          density="comfortable"
          prepend-inner-icon="mdi-tag-outline"
          autofocus
          class="mb-2"
        ></v-text-field>

        <LinkTestTargetFields v-model="draft" />

        <v-divider class="my-4"></v-divider>
        <LinkTestSpecFields v-model="spec" />

        <v-divider class="my-4"></v-divider>
        <div id="lt-interval-label" class="text-title-small font-weight-bold mb-2">Frequência</div>
        <div class="d-flex align-center flex-wrap ga-3 mb-2">
          <v-btn-toggle
            :model-value="presetValue"
            color="primary"
            variant="outlined"
            divided
            density="comfortable"
            aria-labelledby="lt-interval-label"
            @update:model-value="(value) => typeof value === 'number' && (intervalMinutes = value)"
          >
            <v-btn v-for="preset in INTERVAL_PRESETS" :key="preset.value" :value="preset.value">
              {{ preset.title }}
            </v-btn>
          </v-btn-toggle>
          <v-text-field
            :model-value="intervalMinutes"
            label="Minutos"
            type="number"
            :min="MIN_INTERVAL_MINUTES"
            variant="outlined"
            density="compact"
            hide-details
            class="interval-field"
            @update:model-value="(value) => (intervalMinutes = Math.round(Number(value)))"
          ></v-text-field>
        </div>
        <p class="hint text-body-small mb-4">
          Roda {{ intervalLabel(intervalMinutes || MIN_INTERVAL_MINUTES) }} (mínimo de
          {{ MIN_INTERVAL_MINUTES }} minutos — cada teste satura o enlace).
        </p>

        <v-switch
          v-model="useWindow"
          color="primary"
          inset
          hide-details
          label="Só dentro de uma janela de horário"
          class="mb-2"
        ></v-switch>
        <div v-if="useWindow" class="d-flex flex-wrap ga-3 mb-2">
          <v-text-field
            v-model="windowStart"
            label="Das"
            type="time"
            variant="outlined"
            density="comfortable"
            hide-details
            class="time-field"
          ></v-text-field>
          <v-text-field
            v-model="windowEnd"
            label="Até"
            type="time"
            variant="outlined"
            density="comfortable"
            hide-details
            class="time-field"
          ></v-text-field>
        </div>
        <p v-if="useWindow" class="hint text-body-small mb-4">
          Janela que passa da meia-noite vale (ex.: das 22:00 às 06:00).
        </p>

        <v-switch
          v-model="enabled"
          color="success"
          inset
          hide-details
          :label="enabled ? 'Ativo' : 'Pausado'"
        ></v-switch>

        <v-alert
          v-if="showErrors && errors.length > 0"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-3"
        >
          <ul class="errors">
            <li v-for="(error, index) in errors" :key="index">{{ error }}</li>
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
          Salvar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/** Cria ou edita um agendamento: mesmo alvo e parâmetros do assistente, mais frequência. */
import { computed, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTestSchedule, LinkTestScheduleInput, LinkTestSpec } from '@/types/linkTests'
import {
  INTERVAL_PRESETS,
  MIN_INTERVAL_MINUTES,
  intervalLabel,
  normalizeSpec,
  validateSchedule,
} from '@/utils/linkTests/spec'
import { draftFromRequest, targetErrors, type LinkTestTargetDraft } from '@/utils/linkTests/wizard'
import LinkTestSpecFields from './LinkTestSpecFields.vue'
import LinkTestTargetFields from './LinkTestTargetFields.vue'

const props = defineProps<{
  modelValue: boolean
  schedule: LinkTestSchedule | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: [schedule: LinkTestSchedule]
}>()

const { smAndDown } = useDisplay()
const store = useLinkTestsStore()

const name = ref('')
const draft = ref<LinkTestTargetDraft>(draftFromRequest(null))
const spec = ref<LinkTestSpec>(normalizeSpec())
const intervalMinutes = ref(60)
const useWindow = ref(false)
const windowStart = ref('22:00')
const windowEnd = ref('06:00')
const enabled = ref(true)
const showErrors = ref(false)
const saving = ref(false)
const saveError = ref<string | null>(null)

const presetValue = computed(() =>
  INTERVAL_PRESETS.some((preset) => preset.value === intervalMinutes.value)
    ? intervalMinutes.value
    : null
)

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    const current = props.schedule
    name.value = current?.name ?? ''
    draft.value = draftFromRequest(
      current
        ? {
            kind: current.kind,
            linkId: current.linkId ?? undefined,
            sourceDeviceId: current.sourceDeviceId ?? undefined,
            targetDeviceId: current.targetDeviceId ?? undefined,
          }
        : null
    )
    spec.value = normalizeSpec(current?.spec)
    intervalMinutes.value = current?.intervalMinutes ?? 60
    useWindow.value = Boolean(current?.windowStart && current?.windowEnd)
    windowStart.value = current?.windowStart ?? '22:00'
    windowEnd.value = current?.windowEnd ?? '06:00'
    enabled.value = current?.enabled ?? true
    showErrors.value = false
    saveError.value = null
  },
  { immediate: true }
)

const input = computed<LinkTestScheduleInput>(() => ({
  name: name.value.trim(),
  kind: draft.value.kind,
  linkId: draft.value.kind === 'link' ? draft.value.linkId : null,
  sourceDeviceId: draft.value.kind === 'server' ? null : draft.value.sourceDeviceId,
  targetDeviceId: draft.value.targetDeviceId,
  spec: spec.value,
  intervalMinutes: intervalMinutes.value,
  windowStart: useWindow.value ? windowStart.value || null : null,
  windowEnd: useWindow.value ? windowEnd.value || null : null,
  enabled: enabled.value,
}))

const errors = computed(() => [...targetErrors(draft.value), ...validateSchedule(input.value)])

async function save() {
  showErrors.value = true
  if (errors.value.length > 0) return
  saving.value = true
  saveError.value = null
  try {
    const saved = props.schedule
      ? await store.updateSchedule(props.schedule.id, input.value)
      : await store.createSchedule(input.value)
    emit('saved', saved)
    emit('update:modelValue', false)
  } catch (err: unknown) {
    saveError.value =
      err instanceof Error && err.message ? err.message : 'Não foi possível salvar o agendamento.'
  } finally {
    saving.value = false
  }
}

function close() {
  emit('update:modelValue', false)
}
</script>

<style scoped>
.hint {
  color: rgb(var(--v-theme-on-surface));
  margin: 0;
}
.interval-field {
  max-width: 130px;
}
.time-field {
  max-width: 160px;
}
.errors {
  padding-left: 18px;
  margin: 0;
}
</style>

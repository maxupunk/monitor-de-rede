<template>
  <div>
    <div id="st-mode-label" class="text-title-small font-weight-bold mb-2">Quando</div>
    <v-chip-group
      :model-value="draft.mode"
      mandatory
      selected-class="text-primary"
      color="primary"
      column
      aria-labelledby="st-mode-label"
      class="mb-2"
      @update:model-value="(value) => setMode(value as CronMode)"
    >
      <v-chip
        v-for="mode in CRON_MODES"
        :key="mode.value"
        :value="mode.value"
        :prepend-icon="mode.icon"
        variant="outlined"
        filter
      >
        {{ mode.title }}
      </v-chip>
    </v-chip-group>

    <div v-if="draft.mode === 'minutes'" class="d-flex align-center flex-wrap ga-3">
      <v-btn-toggle
        :model-value="minutePreset"
        color="primary"
        variant="outlined"
        divided
        density="comfortable"
        aria-label="Intervalo em minutos"
        @update:model-value="(value) => typeof value === 'number' && patch({ everyMinutes: value })"
      >
        <v-btn v-for="preset in MINUTE_PRESETS" :key="preset" :value="preset">
          {{ preset }} min
        </v-btn>
      </v-btn-toggle>
      <v-text-field
        :model-value="draft.everyMinutes"
        label="Minutos"
        type="number"
        min="1"
        max="59"
        variant="outlined"
        density="compact"
        hide-details
        class="number-field"
        @update:model-value="(value) => patch({ everyMinutes: toInt(value) })"
      ></v-text-field>
    </div>

    <div v-else-if="draft.mode === 'hourly'" class="d-flex align-center flex-wrap ga-3">
      <v-text-field
        :model-value="draft.everyHours"
        label="A cada (horas)"
        type="number"
        min="1"
        max="23"
        variant="outlined"
        density="compact"
        hide-details
        class="number-field"
        @update:model-value="(value) => patch({ everyHours: toInt(value) })"
      ></v-text-field>
      <v-text-field
        :model-value="draft.minute"
        label="No minuto"
        type="number"
        min="0"
        max="59"
        variant="outlined"
        density="compact"
        hide-details
        class="number-field"
        @update:model-value="(value) => patch({ minute: toInt(value) })"
      ></v-text-field>
    </div>

    <template v-else-if="draft.mode === 'custom'">
      <v-text-field
        :model-value="draft.expression"
        label="Expressão cron"
        placeholder="0 3 * * *"
        variant="outlined"
        density="comfortable"
        prepend-inner-icon="mdi-code-braces"
        class="mono"
        hide-details="auto"
        @update:model-value="(value) => patch({ expression: String(value ?? '') })"
      ></v-text-field>
      <p class="hint text-body-small mt-2 mb-0">
        Cinco campos: <strong>minuto hora dia mês dia-da-semana</strong> (0 = domingo). Exemplos:
        <code>0 3 * * *</code> todo dia às 03:00 · <code>*/10 8-18 * * 1-5</code> a cada 10 min no
        horário comercial · <code>0 0 1,15 * *</code> dias 1 e 15.
      </p>
    </template>

    <template v-else>
      <div v-if="draft.mode === 'weekly'" class="mb-3">
        <v-chip-group
          :model-value="draft.weekdays"
          multiple
          column
          color="primary"
          aria-label="Dias da semana"
          @update:model-value="(value) => patch({ weekdays: (value as number[]) ?? [] })"
        >
          <v-chip
            v-for="day in WEEKDAYS"
            :key="day.value"
            :value="day.value"
            :aria-label="day.long"
            variant="outlined"
            filter
          >
            {{ day.short }}
          </v-chip>
        </v-chip-group>
        <div class="d-flex ga-2 mt-1">
          <v-btn
            size="small"
            variant="tonal"
            color="primary"
            @click="patch({ weekdays: [...WORKDAYS] })"
          >
            Dias úteis
          </v-btn>
          <v-btn
            size="small"
            variant="tonal"
            color="primary"
            @click="patch({ weekdays: [...WEEKEND] })"
          >
            Fim de semana
          </v-btn>
        </div>
      </div>
      <div class="d-flex align-center flex-wrap ga-3">
        <v-text-field
          v-if="draft.mode === 'monthly'"
          :model-value="draft.dayOfMonth"
          label="Dia do mês"
          type="number"
          min="1"
          max="31"
          variant="outlined"
          density="compact"
          hide-details
          class="number-field"
          @update:model-value="(value) => patch({ dayOfMonth: toInt(value) })"
        ></v-text-field>
        <v-text-field
          :model-value="draft.time"
          label="Às"
          type="time"
          variant="outlined"
          density="compact"
          hide-details
          class="time-field"
          @update:model-value="(value) => patch({ time: String(value ?? '') })"
        ></v-text-field>
      </div>
      <p
        v-if="draft.mode === 'monthly' && draft.dayOfMonth > 28"
        class="hint text-body-small mt-2 mb-0"
      >
        Nos meses sem o dia {{ draft.dayOfMonth }}, a tarefa não roda.
      </p>
    </template>

    <p class="hint text-body-small mt-3 mb-0">
      Hora local do servidor<template v-if="draft.mode !== 'custom'">
        · expressão <code>{{ modelValue }}</code></template
      >.
    </p>
  </div>
</template>

<script setup lang="ts">
/**
 * Agenda da tarefa: modos amigáveis que montam a expressão cron, e o
 * "Avançado" para escrevê-la direto. O `v-model` é a expressão.
 */
import { computed, ref, watch } from 'vue'
import {
  CRON_MODES,
  MINUTE_PRESETS,
  WEEKDAYS,
  WEEKEND,
  WORKDAYS,
  fromCron,
  toCron,
  type CronDraft,
  type CronMode,
} from '@/utils/scheduledTasks/cron'

const props = defineProps<{ modelValue: string }>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()

const draft = ref<CronDraft>(fromCron(props.modelValue))

// Expressão trocada por fora (abrir outra tarefa, um modelo pronto): o
// rascunho segue. A que nós mesmos emitimos não reabre o modo.
watch(
  () => props.modelValue,
  (value) => {
    if (value !== toCron(draft.value)) draft.value = fromCron(value)
  }
)

const minutePreset = computed(() =>
  MINUTE_PRESETS.includes(draft.value.everyMinutes) ? draft.value.everyMinutes : null
)

function toInt(value: unknown): number {
  const number = Number(value)
  return Number.isFinite(number) ? Math.trunc(number) : 0
}

function patch(change: Partial<CronDraft>) {
  draft.value = { ...draft.value, ...change }
  emit('update:modelValue', toCron(draft.value))
}

/** Ao ir para o "Avançado", leva a expressão de agora para editar. */
function setMode(mode: CronMode) {
  if (!mode || mode === draft.value.mode) return
  const expression = mode === 'custom' ? toCron(draft.value) : draft.value.expression
  patch({ mode, expression })
}
</script>

<style scoped>
.hint {
  color: rgb(var(--v-theme-on-surface));
}
.number-field {
  max-width: 140px;
}
.time-field {
  max-width: 160px;
}
.mono :deep(input),
code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
</style>

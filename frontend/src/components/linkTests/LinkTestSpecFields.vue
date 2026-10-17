<template>
  <div class="spec-fields">
    <div class="d-flex align-center justify-space-between flex-wrap ga-2">
      <label class="text-title-small font-weight-bold" for="lt-duration">Duração por sentido</label>
      <v-chip color="primary" variant="flat" size="small" label> {{ spec.durationS }} s </v-chip>
    </div>
    <v-slider
      id="lt-duration"
      :model-value="spec.durationS"
      :min="DURATION_MIN_S"
      :max="DURATION_MAX_S"
      :step="1"
      color="primary"
      thumb-label
      :readonly="readonly"
      hide-details
      aria-label="Duração por sentido, em segundos"
      @update:model-value="(value) => patch({ durationS: Math.round(Number(value)) })"
    ></v-slider>
    <p class="hint text-body-small mb-4">
      Mais longo = mais preciso, mas ocupa o enlace por mais tempo. 5 s costuma bastar.
    </p>

    <div id="lt-direction-label" class="text-title-small font-weight-bold mb-2">Sentido</div>
    <v-btn-toggle
      :model-value="spec.direction"
      mandatory
      divided
      color="primary"
      variant="outlined"
      density="comfortable"
      class="mb-1 direction"
      aria-labelledby="lt-direction-label"
      :disabled="readonly"
      @update:model-value="setDirection"
    >
      <v-btn
        v-for="option in DIRECTION_OPTIONS"
        :key="option.value"
        :value="option.value"
        :prepend-icon="option.icon"
      >
        {{ option.title }}
      </v-btn>
    </v-btn-toggle>
    <p class="hint text-body-small mb-4">
      Ida é de A para B; volta, de B para A. Medir os dois mostra enlaces assimétricos (rádio, ADSL,
      limite de banda).
    </p>

    <v-expansion-panels variant="accordion" class="mb-2">
      <v-expansion-panel>
        <v-expansion-panel-title>
          <v-icon start color="primary">mdi-tune-variant</v-icon>
          Avançado
          <span class="ml-2 text-body-small">{{ advancedSummary }}</span>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <div class="d-flex align-center justify-space-between flex-wrap ga-2">
            <label class="text-title-small font-weight-bold" for="lt-streams">
              Fluxos paralelos
            </label>
            <v-chip color="primary" variant="flat" size="small" label>{{ spec.streams }}</v-chip>
          </div>
          <v-slider
            id="lt-streams"
            :model-value="spec.streams"
            :min="STREAMS_MIN"
            :max="STREAMS_MAX"
            :step="1"
            color="primary"
            show-ticks="always"
            thumb-label
            :readonly="readonly"
            hide-details
            aria-label="Fluxos paralelos"
            @update:model-value="(value) => patch({ streams: Math.round(Number(value)) })"
          ></v-slider>
          <p class="hint text-body-small mb-4">
            Só no iperf3: vários fluxos enchem enlaces rápidos ou com latência alta. O btest do
            MikroTik usa um.
          </p>
          <v-text-field
            :model-value="spec.limitMbps ?? ''"
            label="Limite de banda (Mbps)"
            type="number"
            min="1"
            variant="outlined"
            density="comfortable"
            prepend-inner-icon="mdi-speedometer-medium"
            clearable
            :readonly="readonly"
            hint="Vazio = sem limite. Use para medir sem derrubar quem está usando o enlace."
            persistent-hint
            :error-messages="limitError"
            @update:model-value="setLimit"
          ></v-text-field>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </div>
</template>

<script setup lang="ts">
/** Passo "Como": duração, sentido e, no avançado, fluxos e limite. */
import { computed } from 'vue'
import type { LinkTestSpec } from '@/types/linkTests'
import { DIRECTION_OPTIONS } from '@/utils/linkTests/presentation'
import {
  DURATION_MAX_S,
  DURATION_MIN_S,
  STREAMS_MAX,
  STREAMS_MIN,
  validateSpec,
} from '@/utils/linkTests/spec'

const props = defineProps<{
  modelValue: LinkTestSpec
  readonly?: boolean
}>()

const emit = defineEmits<{ 'update:modelValue': [value: LinkTestSpec] }>()

const spec = computed(() => props.modelValue)

function patch(change: Partial<LinkTestSpec>) {
  emit('update:modelValue', { ...props.modelValue, ...change })
}

function setDirection(value: unknown) {
  const option = DIRECTION_OPTIONS.find((item) => item.value === value)
  if (option) patch({ direction: option.value })
}

function setLimit(value: string | number | null) {
  if (value === null || value === '') {
    patch({ limitMbps: null })
    return
  }
  const parsed = Number(value)
  patch({ limitMbps: Number.isFinite(parsed) ? parsed : null })
}

const limitError = computed(() =>
  validateSpec({ limitMbps: props.modelValue.limitMbps }).slice(0, 1)
)

const advancedSummary = computed(() => {
  const parts = [`${spec.value.streams} ${spec.value.streams === 1 ? 'fluxo' : 'fluxos'}`]
  parts.push(spec.value.limitMbps ? `até ${spec.value.limitMbps} Mbps` : 'sem limite')
  return parts.join(' · ')
})
</script>

<style scoped>
.hint {
  color: rgb(var(--v-theme-on-surface));
  /* O espaço até o próximo campo: `margin: 0` aqui anulava o `mb-4`. */
  margin: 0 0 16px;
}
.direction {
  flex-wrap: wrap;
  height: auto;
}
</style>

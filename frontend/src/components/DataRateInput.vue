<template>
  <div class="d-flex data-rate-input" style="gap: 8px">
    <v-text-field
      :model-value="displayValue"
      type="number"
      :label="label"
      :variant="variant"
      :density="density"
      :hide-details="hideDetails"
      :bg-color="bgColor"
      :rules="rules"
      :disabled="disabled"
      style="flex: 1 1 auto; min-width: 0"
      @update:model-value="onValueInput"
    ></v-text-field>
    <v-select
      :model-value="unit"
      :items="UNITS"
      :density="density"
      :variant="variant"
      :hide-details="hideDetails"
      :bg-color="bgColor"
      :disabled="disabled"
      style="flex: 0 0 104px"
      @update:model-value="onUnitChange"
    ></v-select>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

type RateUnit = 'bps' | 'Kbps' | 'Mbps' | 'Gbps'

const UNITS: RateUnit[] = ['bps', 'Kbps', 'Mbps', 'Gbps']
const FACTORS: Record<RateUnit, number> = {
  bps: 1,
  Kbps: 1_000,
  Mbps: 1_000_000,
  Gbps: 1_000_000_000,
}

const props = withDefaults(
  defineProps<{
    /** Valor canônico sempre em bps — mesma unidade usada pelo backend */
    modelValue: number | null
    label?: string
    variant?:
      | 'outlined'
      | 'filled'
      | 'underlined'
      | 'solo'
      | 'solo-filled'
      | 'solo-inverted'
      | 'plain'
    density?: 'default' | 'comfortable' | 'compact'
    hideDetails?: boolean | 'auto'
    bgColor?: string
    rules?: Array<(value: unknown) => true | string>
    disabled?: boolean
  }>(),
  {
    label: 'Valor de referência',
    variant: 'outlined',
    density: 'default',
    hideDetails: false,
    rules: () => [],
    disabled: false,
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: number | null): void
}>()

const unit = ref<RateUnit>('Mbps')
const displayValue = ref<number | null>(null)

// Distingue alterações vindas de fora (nova métrica, edição carregada) das que
// o próprio componente acabou de emitir — só re-escolhemos a unidade "ideal"
// nas externas, senão a unidade trocaria sozinha enquanto o usuário digita.
let suppressWatch = false

function pickUnit(bps: number): RateUnit {
  const magnitude = Math.abs(bps)
  if (magnitude === 0) return unit.value
  const index = Math.min(Math.floor(Math.log(magnitude) / Math.log(1000)), UNITS.length - 1)
  return UNITS[Math.max(index, 0)]
}

function syncFromModelValue(value: number | null | undefined) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    displayValue.value = null
    return
  }
  unit.value = pickUnit(value)
  displayValue.value = Number.parseFloat((value / FACTORS[unit.value]).toFixed(6))
}

watch(
  () => props.modelValue,
  (value) => {
    if (suppressWatch) {
      suppressWatch = false
      return
    }
    syncFromModelValue(value)
  },
  { immediate: true }
)

function onValueInput(raw: string | number) {
  const num = raw === '' || raw === null || raw === undefined ? NaN : Number(raw)
  displayValue.value = Number.isFinite(num) ? num : null

  suppressWatch = true
  emit('update:modelValue', displayValue.value === null ? null : Math.round(displayValue.value * FACTORS[unit.value]))
}

function onUnitChange(newUnit: RateUnit) {
  unit.value = newUnit
  if (props.modelValue === null || props.modelValue === undefined) return
  displayValue.value = Number.parseFloat((props.modelValue / FACTORS[newUnit]).toFixed(6))
}
</script>

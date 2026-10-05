<template>
  <div v-if="result" class="mac-vendor d-flex flex-wrap align-center ga-2 mt-2">
    <template v-if="result.vendor">
      <v-chip
        size="small"
        color="info"
        variant="tonal"
        prepend-icon="mdi-domain"
        :title="result.organization ?? undefined"
      >
        {{ result.vendor }}
        <span class="ms-1 font-weight-regular">· {{ SOURCE_LABELS[result.source] }}</span>
      </v-chip>
      <v-btn
        v-if="!sameText(currentVendor, result.vendor)"
        size="small"
        color="primary"
        variant="flat"
        prepend-icon="mdi-check"
        @click="emit('apply-vendor', result.vendor)"
      >
        Usar fabricante
      </v-btn>
    </template>
    <v-chip
      v-else-if="result.locallyAdministered"
      size="small"
      color="warning"
      variant="tonal"
      prepend-icon="mdi-incognito"
      title="Celulares e notebooks trocam o MAC por um aleatório para proteger a privacidade; o fabricante não pode ser identificado por ele."
    >
      MAC aleatório (privacidade)
    </v-chip>
    <v-chip v-else size="small" color="warning" variant="tonal" prepend-icon="mdi-help-circle">
      Fabricante não encontrado
    </v-chip>

    <template v-if="hintMeta">
      <v-chip
        size="small"
        :color="hintMeta.color"
        variant="tonal"
        :prepend-icon="hintMeta.icon"
        :title="result.hint?.reasons.join(' · ')"
      >
        Parece: {{ hintMeta.label }}
      </v-chip>
      <v-btn
        v-if="currentType !== hintMeta.id"
        size="small"
        color="primary"
        variant="tonal"
        @click="emit('apply-type', hintMeta.id)"
      >
        Usar tipo
      </v-btn>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { MIN_MAC_DIGITS, macDigits, useVendorsStore, type MacVendorLookup } from '@/stores/vendors'
import { deviceTypeMeta } from '@/utils/deviceTypes'

/**
 * Fabricante do MAC digitado e o tipo que ele sugere, com um clique para
 * aplicar. Nada é trocado sozinho: o operador vê de onde veio e decide.
 */
const props = defineProps<{
  mac: string | null | undefined
  currentVendor?: string | null
  currentType?: string | null
}>()

const emit = defineEmits<{
  'apply-vendor': [vendor: string]
  'apply-type': [type: string]
}>()

const SOURCE_LABELS: Record<MacVendorLookup['source'], string> = {
  ieee: 'registro IEEE',
  builtin: 'base embutida',
  none: '',
}

/** Espera o operador parar de digitar antes de perguntar ao servidor. */
const TYPING_PAUSE_MS = 350

const vendors = useVendorsStore()
const result = ref<MacVendorLookup | null>(null)
let pending: ReturnType<typeof setTimeout> | null = null

const hintMeta = computed(() => {
  const hint = result.value?.hint
  if (!hint) return null
  const meta = deviceTypeMeta(hint.deviceType)
  return meta.isKnown ? meta : null
})

function sameText(a?: string | null, b?: string | null): boolean {
  return (a ?? '').trim().toLowerCase() === (b ?? '').trim().toLowerCase()
}

watch(
  () => props.mac,
  (mac) => {
    if (pending) clearTimeout(pending)
    const digits = macDigits(mac ?? '')
    if (digits.length < MIN_MAC_DIGITS) {
      result.value = null
      return
    }
    pending = setTimeout(async () => {
      pending = null
      const found = await vendors.lookup(digits)
      // O campo pode ter mudado enquanto a resposta vinha.
      if (macDigits(props.mac ?? '') === digits) result.value = found
    }, TYPING_PAUSE_MS)
  },
  { immediate: true }
)

onBeforeUnmount(() => {
  if (pending) clearTimeout(pending)
})
</script>

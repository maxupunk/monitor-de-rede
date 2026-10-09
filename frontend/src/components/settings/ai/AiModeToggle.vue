<template>
  <div>
    <div class="font-weight-bold mb-2">{{ title }}</div>
    <v-btn-toggle
      v-model="mode"
      mandatory
      divided
      color="primary"
      variant="outlined"
      density="comfortable"
      class="mode-toggle"
    >
      <v-btn v-for="option in options" :key="option.value" :value="option.value">
        <v-icon start size="18">{{ option.icon }}</v-icon>
        {{ option.title }}
      </v-btn>
    </v-btn-toggle>
    <div class="text-body-2 mt-2">
      {{ selected?.hint }}
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts" generic="T extends string">
import { computed } from 'vue'
import type { AiModeOption } from './aiAutomationModes'

/** Escolha única em botões com a explicação da opção marcada logo abaixo. */
const props = defineProps<{
  title: string
  options: AiModeOption<T>[]
}>()

const mode = defineModel<T>({ required: true })

const selected = computed(() => props.options.find((option) => option.value === mode.value))
</script>

<style scoped>
/* No celular as opções não cabem lado a lado: empilham na largura toda em vez
   de quebrar a linha com a borda partida. */
@media (max-width: 599px) {
  .mode-toggle {
    flex-direction: column;
    width: 100%;
    height: auto !important;
  }
  .mode-toggle :deep(.v-btn) {
    justify-content: flex-start;
    width: 100%;
    min-height: 40px;
  }
  .mode-toggle.v-btn-group--divided :deep(.v-btn:not(:last-child)) {
    border-inline-end: none;
    border-bottom: thin solid rgba(var(--v-border-color), var(--v-border-opacity));
  }
}
</style>

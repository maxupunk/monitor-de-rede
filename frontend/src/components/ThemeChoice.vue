<template>
  <div class="px-4 py-2" @click.stop>
    <div id="theme-choice-label" class="text-body-2 font-weight-bold mb-1">Tema</div>
    <v-btn-toggle
      :model-value="preference"
      mandatory
      divided
      density="compact"
      color="primary"
      variant="outlined"
      class="w-100"
      aria-labelledby="theme-choice-label"
      @update:model-value="onChoose"
    >
      <v-btn
        v-for="option in THEME_OPTIONS"
        :key="option.value"
        :value="option.value"
        :aria-label="option.label"
        :title="option.label"
        class="flex-grow-1"
        min-width="0"
        :data-test="`theme-${option.value}`"
      >
        <v-icon :icon="option.icon"></v-icon>
      </v-btn>
    </v-btn-toggle>
  </div>
</template>

<script setup lang="ts">
/** Escolha de tema no menu do usuário: escuro, claro ou o do sistema. */
import { useThemePreference } from '@/composables/useThemePreference'
import { THEME_OPTIONS, isPreference } from '@/utils/themePreference'

const { preference, choose } = useThemePreference()

function onChoose(value: unknown) {
  if (isPreference(value)) choose(value)
}
</script>

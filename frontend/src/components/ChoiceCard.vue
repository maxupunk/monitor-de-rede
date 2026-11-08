<template>
  <v-card
    border
    rounded="lg"
    :variant="selected ? 'tonal' : 'flat'"
    :color="selected ? color : undefined"
    :disabled="disabled"
    class="choice-card"
    :class="{ 'choice-card--selected': selected }"
    :style="{ '--choice-color': `var(--v-theme-${color})` }"
    role="button"
    :tabindex="disabled ? -1 : 0"
    :aria-pressed="selected"
    :aria-disabled="disabled || undefined"
    @keydown.enter.space.self.prevent="activate"
  >
    <slot></slot>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Card clicável: as escolhas de "qual tipo?", "qual SGBD?", "qual perfil?" e
 * os atalhos que abrem um detalhe. Borda do tema em repouso, a cor ao passar
 * o mouse ou focar pelo teclado e, quando escolhido, fundo tonal com a borda
 * reforçada — igual em todas as telas. Enter e Espaço ativam como o clique.
 * O `@click` e o resto dos atributos passam direto para o `<v-card>`.
 */
const props = withDefaults(
  defineProps<{
    /** Escolhido agora. Sem a prop, o card é só um atalho (sem estado). */
    selected?: boolean
    /** Cor do tema para o destaque. */
    color?: string
    disabled?: boolean
  }>(),
  { selected: undefined, color: 'primary', disabled: false }
)

function activate(event: KeyboardEvent) {
  if (!props.disabled && event.currentTarget instanceof HTMLElement) event.currentTarget.click()
}
</script>

<style scoped>
.choice-card {
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease;
}
.choice-card:hover,
.choice-card:focus-visible {
  border-color: rgb(var(--choice-color));
}
.choice-card--selected {
  border-color: rgb(var(--choice-color));
  box-shadow: 0 0 0 1px rgb(var(--choice-color));
}
</style>

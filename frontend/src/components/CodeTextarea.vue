<template>
  <v-textarea
    v-model="model"
    variant="outlined"
    no-resize
    spellcheck="false"
    autocomplete="off"
    autocapitalize="off"
    class="code-textarea"
    :style="{ '--code-textarea-height': height }"
  ></v-textarea>
</template>

<script setup lang="ts">
/**
 * Campo de código (JSON, Rhai, Markdown…) com altura fixa e rolagem interna.
 *
 * Nunca usa `auto-grow`: um conteúdo grande rola dentro do campo, e o que está
 * em volta (abas, ações do diálogo) continua no lugar. Os demais atributos
 * (`hint`, `readonly`, `error-messages`, `placeholder`…) vão direto ao
 * `v-textarea`.
 */
withDefaults(
  defineProps<{
    /** Altura do campo; o padrão (`--code-editor-height`) acompanha a janela. */
    height?: string
  }>(),
  { height: 'var(--code-editor-height)' }
)

const model = defineModel<string>({ required: true })
</script>

<style scoped>
.code-textarea :deep(textarea) {
  height: var(--code-textarea-height);
  overflow-y: auto;
  font-family:
    'JetBrains Mono', 'Fira Code', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.82rem;
  line-height: 1.45;
  white-space: pre;
  overflow-x: auto;
  tab-size: 2;
}
</style>

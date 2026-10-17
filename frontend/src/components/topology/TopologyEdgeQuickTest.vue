<template>
  <div
    class="edge-quick-test"
    :style="{ left: `${x}px`, top: `${y}px` }"
    @pointerenter="$emit('hold')"
    @pointerleave="$emit('release')"
  >
    <v-btn
      color="info"
      variant="flat"
      size="small"
      rounded="pill"
      prepend-icon="mdi-speedometer"
      class="elevation-6"
      @click="$emit('test')"
    >
      Testar velocidade
    </v-btn>
  </div>
</template>

<script setup lang="ts">
/**
 * Atalho que aparece ao passar o mouse num enlace: o teste de velocidade é a
 * ação mais pedida sobre um enlace e não deveria morar só no menu de contexto.
 * Só posiciona e avisa — quem decide se cabe (perfil, tipo do enlace) é a página.
 */
defineProps<{ x: number; y: number }>()
defineEmits<{ test: []; hold: []; release: [] }>()
</script>

<style scoped>
.edge-quick-test {
  position: absolute;
  z-index: 6;
  transform: translate(-50%, 14px);
}
</style>

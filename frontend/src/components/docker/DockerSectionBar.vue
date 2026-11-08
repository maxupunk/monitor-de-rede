<template>
  <div class="docker-section-bar d-flex flex-wrap align-center ga-3 mb-4">
    <p class="text-body-2 text-high-emphasis flex-grow-1 ma-0">{{ description }}</p>
    <div
      v-if="$slots.default"
      class="docker-section-bar__actions d-flex flex-wrap align-center ga-2"
    >
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Topo de cada aba do `/docker`: o que a aba mostra, em uma frase, e as
 * ações dela. O título e o host já estão no cabeçalho único do layout — aqui
 * não se repete nenhum dos dois.
 */
defineProps<{ description: string }>()
</script>

<style scoped>
.docker-section-bar p {
  min-width: 220px;
}

@media (max-width: 599.98px) {
  .docker-section-bar__actions {
    width: 100%;
  }

  .docker-section-bar__actions > :deep(*) {
    flex: 1 1 0;
  }
}
</style>

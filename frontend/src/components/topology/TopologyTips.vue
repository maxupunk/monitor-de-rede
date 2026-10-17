<template>
  <section class="topology-tips" :class="{ narrow }" role="note" aria-label="Como usar o mapa">
    <div class="d-flex align-center ga-2 mb-1">
      <v-icon color="primary" size="20">mdi-lightbulb-on-outline</v-icon>
      <span class="font-weight-bold flex-grow-1">Como usar o mapa</span>
    </div>
    <ul>
      <li>
        <v-icon size="16" color="primary">mdi-gesture-tap</v-icon>
        <span>
          {{ narrow ? 'Toque' : 'Clique' }} num dispositivo ou enlace para ver os detalhes;
          {{ narrow ? 'toque longo' : 'botão direito' }} abre as ações.
        </span>
      </li>
      <li>
        <v-icon size="16" color="info">mdi-speedometer</v-icon>
        <span>
          <strong>Teste de velocidade:</strong>
          {{ narrow ? 'toque longo' : 'passe o mouse ou clique com o botão direito' }} num enlace e
          escolha "Testar velocidade".
        </span>
      </li>
      <li v-if="canWrite">
        <v-icon size="16" color="warning">mdi-pencil</v-icon>
        <span>
          <strong>Editar mapa</strong>{{ narrow ? '' : ' (E)' }}: arraste dispositivos e puxe cabos;
          as posições valem para todos.
        </span>
      </li>
    </ul>
    <div class="d-flex justify-end ga-2 mt-1">
      <v-btn
        v-if="!narrow"
        color="primary"
        variant="text"
        size="small"
        prepend-icon="mdi-keyboard-outline"
        @click="$emit('shortcuts')"
      >
        Atalhos
      </v-btn>
      <v-btn color="primary" variant="flat" size="small" @click="$emit('dismiss')">Entendi</v-btn>
    </div>
  </section>
</template>

<script setup lang="ts">
/** Dica de primeiro uso do mapa — some de vez com "Entendi" (preferência do visitante). */
defineProps<{ narrow: boolean; canWrite: boolean }>()
defineEmits<{ dismiss: []; shortcuts: [] }>()
</script>

<style scoped>
.topology-tips {
  width: 400px;
  max-width: calc(100vw - 24px);
  padding: 10px 12px;
  border-radius: 12px;
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  border: 2px solid rgb(var(--v-theme-primary));
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  font-size: 0.8125rem;
}
.topology-tips.narrow {
  width: auto;
}
ul {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
li {
  display: flex;
  gap: 8px;
  align-items: flex-start;
  line-height: 1.35;
}
li .v-icon {
  margin-top: 1px;
  flex-shrink: 0;
}
</style>

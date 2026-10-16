<template>
  <div class="topology-status" role="status">
    <span class="item counts">
      <v-icon size="16" color="primary">mdi-devices</v-icon>
      <strong>{{ summary.nodes }}</strong> dispositivos
    </span>
    <span class="item counts">
      <v-icon size="16" color="primary">mdi-lan</v-icon>
      <strong>{{ summary.edges }}</strong> enlaces
    </span>
    <button
      v-if="summary.offline + summary.warning + summary.linksDown + summary.saturated > 0"
      type="button"
      class="item problems"
      :class="{ active: problemsOnly }"
      :aria-pressed="problemsOnly"
      @click="$emit('toggle-problems')"
    >
      <v-icon size="16" color="error">mdi-alert-circle</v-icon>
      <span v-if="summary.offline"
        ><strong>{{ summary.offline }}</strong> offline</span
      >
      <span v-if="summary.warning"
        ><strong>{{ summary.warning }}</strong> em alerta</span
      >
      <span v-if="summary.linksDown"
        ><strong>{{ summary.linksDown }}</strong> enlaces fora</span
      >
      <span v-if="summary.saturated"
        ><strong>{{ summary.saturated }}</strong> saturados</span
      >
      <span class="hint">{{ problemsOnly ? 'mostrar tudo' : 'ver só problemas' }}</span>
    </button>
    <span v-else class="item ok">
      <v-icon size="16" color="success">mdi-check-circle</v-icon>
      Tudo operando
    </span>
  </div>
</template>

<script setup lang="ts">
export interface TopologySummary {
  nodes: number
  edges: number
  offline: number
  warning: number
  linksDown: number
  saturated: number
}

defineProps<{ summary: TopologySummary; problemsOnly: boolean }>()
defineEmits<{ 'toggle-problems': [] }>()
</script>

<style scoped>
.topology-status {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px 14px;
  padding: 6px 12px;
  border-radius: 10px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.22);
  color: rgb(var(--v-theme-on-surface));
  font-size: 0.8rem;
}
.item {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}
.problems {
  flex-wrap: wrap;
  white-space: normal;
  gap: 2px 8px;
  padding: 2px 8px;
  border-radius: 6px;
  border: 1px solid rgb(var(--v-theme-error));
  color: rgb(var(--v-theme-on-surface));
}
.problems.active,
.problems:hover {
  background: rgba(var(--v-theme-error), 0.16);
}
.hint {
  font-weight: 700;
  color: rgb(var(--v-theme-error));
}
@media (max-width: 599px) {
  .counts {
    display: none;
  }
}
</style>

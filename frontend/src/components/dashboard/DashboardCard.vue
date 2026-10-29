<template>
  <v-card variant="flat" rounded="lg" class="dashboard-card dashboard-surface" :loading="loading">
    <header class="dashboard-card__header">
      <div class="dashboard-card__heading">
        <v-avatar v-if="icon" :color="iconColor" variant="tonal" size="32" rounded="lg">
          <v-icon size="18">{{ icon }}</v-icon>
        </v-avatar>
        <div class="dashboard-card__titles">
          <h2 class="dashboard-card__title">{{ title }}</h2>
          <div v-if="subtitle || $slots.subtitle" class="dashboard-card__subtitle">
            <slot name="subtitle">{{ subtitle }}</slot>
          </div>
        </div>
      </div>
      <div v-if="$slots.actions || to" class="dashboard-card__actions">
        <slot name="actions"></slot>
        <v-btn
          v-if="to"
          :to="to"
          variant="text"
          color="primary"
          size="small"
          append-icon="mdi-arrow-right"
        >
          {{ toLabel }}
        </v-btn>
      </div>
    </header>

    <div class="dashboard-card__body" :class="{ 'dashboard-card__body--flush': flush }">
      <slot></slot>
    </div>

    <footer v-if="$slots.footer" class="dashboard-card__footer">
      <slot name="footer"></slot>
    </footer>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Casco único de todo widget do Dashboard: superfície lisa, sem borda,
 * cabeçalho com ícone, título, subtítulo e ações sempre no mesmo lugar.
 * O widget cuida só do conteúdo — padding, tipografia e altura são daqui.
 */
withDefaults(
  defineProps<{
    title: string
    icon?: string
    iconColor?: string
    subtitle?: string
    /** Link "ver tudo" no canto do cabeçalho. */
    to?: string
    toLabel?: string
    loading?: boolean
    /** Corpo sem padding lateral (listas que vão de borda a borda). */
    flush?: boolean
  }>(),
  {
    icon: undefined,
    iconColor: 'primary',
    subtitle: undefined,
    to: undefined,
    toLabel: 'Ver tudo',
    loading: false,
    flush: false,
  }
)
</script>

<style scoped>
.dashboard-card {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.dashboard-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px 12px;
  padding: 16px 20px 8px;
}

.dashboard-card__heading {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.dashboard-card__titles {
  min-width: 0;
}

.dashboard-card__title {
  font-size: 1rem;
  font-weight: 700;
  line-height: 1.3;
  margin: 0;
}

.dashboard-card__subtitle {
  font-size: 0.8rem;
  line-height: 1.35;
  color: rgba(var(--v-theme-on-surface), 0.78);
}

.dashboard-card__actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-left: auto;
}

.dashboard-card__body {
  flex: 1 1 auto;
  min-height: 0;
  padding: 8px 20px 16px;
}

.dashboard-card__body--flush {
  padding: 4px 0 8px;
}

.dashboard-card__footer {
  padding: 0 20px 14px;
}
</style>

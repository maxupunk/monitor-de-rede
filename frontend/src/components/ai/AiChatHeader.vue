<template>
  <header class="ai-chat-header" :class="{ 'ai-chat-header--compact': compact }">
    <slot name="leading">
      <v-avatar size="32" color="primary">
        <v-icon size="18" color="white">mdi-robot-outline</v-icon>
      </v-avatar>
    </slot>

    <div class="ai-chat-header__title">
      <component
        :is="compact ? 'div' : 'h1'"
        :class="compact ? 'text-title-small' : 'text-title-medium'"
        class="font-weight-bold text-truncate"
      >
        {{ title }}
      </component>
      <div v-if="subtitle" class="text-body-small font-mono text-truncate">{{ subtitle }}</div>
    </div>

    <div class="ai-chat-header__actions">
      <slot name="actions" />
    </div>
  </header>
</template>

<script setup lang="ts">
/**
 * Cabeçalho do chat da IA: avatar (ou o que vier em `leading`), título,
 * modelo em uso e as ações de quem hospeda. `compact` é o painel lateral;
 * sem ele, a tela cheia, onde o título é o `h1` da página.
 */
withDefaults(
  defineProps<{
    title: string
    subtitle?: string
    compact?: boolean
  }>(),
  { subtitle: '', compact: false }
)
</script>

<style scoped>
.ai-chat-header {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
  min-height: 56px;
  padding: 6px 8px 6px 12px;
  border-bottom: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  background: rgb(var(--v-theme-surface));
}

.ai-chat-header--compact {
  min-height: 0;
  padding: 8px 8px 8px 12px;
}

.ai-chat-header__title {
  flex: 1 1 auto;
  min-width: 0;
  line-height: 1.25;
}

.ai-chat-header__title h1 {
  font-size: inherit;
  margin: 0;
}

.ai-chat-header__actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 0 0 auto;
}

.ai-chat-header--compact .ai-chat-header__actions {
  gap: 0;
}
</style>

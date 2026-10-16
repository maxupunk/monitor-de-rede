<template>
  <v-list :density="dense ? 'compact' : 'comfortable'" class="py-0 action-list" nav>
    <template v-for="(section, index) in sections" :key="section.group">
      <v-divider v-if="index > 0" class="my-1"></v-divider>
      <v-list-subheader class="font-weight-bold">{{ section.label }}</v-list-subheader>
      <v-tooltip
        v-for="action in section.actions"
        :key="action.id"
        :text="action.disabledReason ?? ''"
        :disabled="!action.disabledReason"
        location="start"
        max-width="280"
      >
        <template #activator="{ props: tip }">
          <div v-bind="tip">
            <v-list-item
              :disabled="Boolean(action.disabledReason)"
              :title="action.title"
              :subtitle="action.subtitle"
              :base-color="action.color ?? 'primary'"
              @click="run(action)"
            >
              <template #prepend>
                <v-icon :color="action.disabledReason ? undefined : (action.color ?? 'primary')">
                  {{ action.icon }}
                </v-icon>
              </template>
              <template v-if="action.shortcut || action.disabledReason" #append>
                <v-icon v-if="action.disabledReason" size="16" color="info">mdi-lock</v-icon>
                <kbd v-else>{{ action.shortcut }}</kbd>
              </template>
            </v-list-item>
          </div>
        </template>
      </v-tooltip>
    </template>
    <div v-if="actions.length === 0" class="pa-4 text-body-2 empty">Nenhuma ação disponível.</div>
  </v-list>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { groupActions, type ResolvedAction } from '@/utils/topology/inspectorRegistry'

const props = defineProps<{ actions: ResolvedAction[]; dense?: boolean }>()
const emit = defineEmits<{ ran: [id: string] }>()

const sections = computed(() => groupActions(props.actions))

function run(action: ResolvedAction) {
  if (action.disabledReason) return
  emit('ran', action.id)
  void action.run()
}
</script>

<style scoped>
.action-list :deep(.v-list-item-title) {
  font-weight: 600;
}
kbd {
  font-family: inherit;
  font-size: 0.7rem;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.4);
  color: rgb(var(--v-theme-on-surface));
}
.empty {
  color: rgb(var(--v-theme-on-surface));
}
</style>

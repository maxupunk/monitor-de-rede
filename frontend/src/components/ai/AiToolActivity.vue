<template>
  <div>
    <template v-for="item in items" :key="item.type === 'tool' ? item.tool.id : 'lookups'">
      <AiToolCard v-if="item.type === 'tool'" :tool="item.tool" :message-id="messageId" />
      <div v-else class="lookups my-2">
        <button
          type="button"
          class="lookups__line d-flex align-center ga-2 text-body-small"
          :aria-expanded="lookupsExpanded"
          :title="lookupsExpanded ? 'Ocultar as consultas' : 'Ver o que foi consultado'"
          @click="lookupsExpanded = !lookupsExpanded"
        >
          <v-icon size="16" color="primary">mdi-database-search-outline</v-icon>
          <span class="lookups__text">
            <span class="font-weight-bold">Consultou:</span>
            {{ describeLookups(item.tools) }}
          </span>
          <v-icon size="16" color="primary" class="ms-auto">
            {{ lookupsExpanded ? 'mdi-chevron-up' : 'mdi-chevron-down' }}
          </v-icon>
        </button>
        <v-expand-transition>
          <div v-if="lookupsExpanded">
            <AiToolCard
              v-for="tool in item.tools"
              :key="tool.id"
              :tool="tool"
              :message-id="messageId"
            />
          </div>
        </v-expand-transition>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { AiToolCallState } from '@/utils/aiChatStream'
import AiToolCard from './AiToolCard.vue'
import { describeLookups, groupToolActivity } from './toolActivity'

const props = defineProps<{
  tools: AiToolCallState[]
  messageId: string
}>()

const items = computed(() => groupToolActivity(props.tools))
const lookupsExpanded = ref(false)
</script>

<style scoped>
.lookups__line {
  width: 100%;
  padding: 4px 10px;
  border: thin solid rgba(var(--v-theme-primary), 0.35);
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.06);
  color: rgb(var(--v-theme-on-surface));
  text-align: start;
  cursor: pointer;
}
.lookups__line:hover {
  background: rgba(var(--v-theme-primary), 0.12);
}
.lookups__line:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: 2px;
}
.lookups__text {
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>

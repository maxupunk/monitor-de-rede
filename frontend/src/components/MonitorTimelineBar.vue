<template>
  <div class="timeline-bar-wrapper d-inline-flex align-center ga-1">
    <div v-for="block in formattedBlocks" :key="block.id" class="timeline-block-container">
      <v-tooltip
        location="top"
        :disabled="!block.hasData"
        color="#0F172A"
        :open-delay="60"
        :close-delay="40"
        offset="6"
      >
        <template #activator="{ props: tooltipProps }">
          <div
            v-bind="tooltipProps"
            class="timeline-block"
            :class="{ 'empty-block': !block.hasData }"
            :style="{
              backgroundColor: block.color,
              height: `${height}px`,
              width: `${width}px`,
            }"
          ></div>
        </template>
        <div v-if="block.hasData" class="custom-tooltip-content pa-2">
          <div class="d-flex align-center ga-2 mb-1">
            <span class="status-dot" :style="{ backgroundColor: block.color }"></span>
            <span class="font-weight-bold" style="font-size: 13px; color: #ffffff">
              {{ block.statusText.toUpperCase() }}
            </span>
            <span
              v-if="block.latencyMs !== null"
              class="font-weight-bold"
              style="font-size: 13px; color: #38bdf8"
            >
              ({{ formatLatency(block.latencyMs) }})
            </span>
          </div>
          <div style="font-size: 11px; color: #94a3b8">
            {{ block.formattedTime }}
          </div>
          <div
            v-if="block.message"
            class="mt-1 pt-1"
            style="font-size: 11px; color: #e2e8f0; border-top: 1px solid #334155"
          >
            {{ block.message }}
          </div>
        </div>
      </v-tooltip>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { MonitorResult } from '@/stores/monitors'
import { getStatusHexColor } from '@/utils/monitorPresentation'
import { formatLatency, formatShortDateTime } from '@/utils/formatters'

const props = withDefaults(
  defineProps<{
    results?: MonitorResult[]
    maxBlocks?: number
    height?: number
    width?: number
  }>(),
  {
    results: () => [],
    maxBlocks: 30,
    height: 24,
    width: 6,
  }
)

interface BlockItem {
  id: string
  hasData: boolean
  statusText: string
  color: string
  latencyMs: number | null
  formattedTime: string
  message?: string
}

const formattedBlocks = computed<BlockItem[]>(() => {
  const list = props.results || []
  const count = props.maxBlocks

  // Pegar os últimos maxBlocks resultados
  const sliced = list.slice(-count)
  const padLength = Math.max(0, count - sliced.length)

  const items: BlockItem[] = []

  // Preencher slots vazios à esquerda se houver menos execuções
  for (let i = 0; i < padLength; i++) {
    items.push({
      id: `empty-${i}`,
      hasData: false,
      statusText: 'Sem dados',
      color: '#E0E0E0',
      latencyMs: null,
      formattedTime: '',
    })
  }

  // Adicionar dados reais
  for (let i = 0; i < sliced.length; i++) {
    const item = sliced[i]
    items.push({
      id: `real-${item.id || i}-${item.finishedAt || item.startedAt || i}`,
      hasData: true,
      statusText: item.status || 'unknown',
      color: getStatusHexColor(item.status),
      latencyMs: item.latencyMs ?? null,
      formattedTime: formatShortDateTime(item.finishedAt || item.startedAt),
      message: item.message || undefined,
    })
  }

  return items
})
</script>

<style scoped>
.timeline-bar-wrapper {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  user-select: none;
  padding: 4px 2px;
  box-sizing: border-box;
}

.timeline-block-container {
  display: flex;
  align-items: center;
  justify-content: center;
}

.timeline-block {
  border-radius: 3px;
  transition:
    transform 0.15s cubic-bezier(0.4, 0, 0.2, 1),
    opacity 0.15s ease,
    filter 0.15s ease;
  cursor: pointer;
  transform-origin: center center;
}

.timeline-block:hover {
  transform: scaleY(1.15) scaleX(1.1);
  opacity: 0.95;
  filter: brightness(1.2);
}

.empty-block {
  cursor: default;
  opacity: 0.4;
}

.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.custom-tooltip-content {
  max-width: 280px;
  pointer-events: none;
}
</style>

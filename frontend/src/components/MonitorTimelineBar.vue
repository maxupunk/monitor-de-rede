<template>
  <div class="timeline-bar-wrapper d-inline-flex align-center gap-1">
    <div
      v-for="(block, idx) in formattedBlocks"
      :key="idx"
      class="timeline-block-container"
    >
      <v-tooltip location="top" :disabled="!block.hasData" color="#0F172A">
        <template #activator="{ props }">
          <div
            v-bind="props"
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
          <div class="d-flex align-center gap-2 mb-1">
            <span class="status-dot" :style="{ backgroundColor: block.color }"></span>
            <span class="font-weight-bold" style="font-size: 13px; color: #FFFFFF">
              {{ block.statusText.toUpperCase() }}
            </span>
            <span v-if="block.latencyMs !== null" class="font-weight-bold" style="font-size: 13px; color: #38BDF8">
              ({{ block.latencyMs }} ms)
            </span>
          </div>
          <div style="font-size: 11px; color: #94A3B8">
            {{ block.formattedTime }}
          </div>
          <div v-if="block.message" class="mt-1 pt-1" style="font-size: 11px; color: #E2E8F0; border-top: 1px solid #334155">
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
  hasData: boolean
  statusText: string
  color: string
  latencyMs: number | null
  formattedTime: string
  message?: string
}

function getStatusColor(status?: string): string {
  switch (status) {
    case 'up':
    case 'online':
      return '#4CAF50'
    case 'down':
    case 'offline':
      return '#F44336'
    case 'warning':
      return '#FF9800'
    case 'disabled':
      return '#9E9E9E'
    default:
      return '#B0BEC5'
  }
}

function formatDate(dateStr?: string): string {
  if (!dateStr) return ''
  try {
    const d = new Date(dateStr)
    return d.toLocaleString('pt-BR', {
      day: '2-digit',
      month: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    })
  } catch {
    return dateStr
  }
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
      hasData: false,
      statusText: 'Sem dados',
      color: '#E0E0E0',
      latencyMs: null,
      formattedTime: '',
    })
  }

  // Adicionar dados reais
  for (const item of sliced) {
    items.push({
      hasData: true,
      statusText: item.status || 'unknown',
      color: getStatusColor(item.status),
      latencyMs: item.latencyMs ?? null,
      formattedTime: formatDate(item.finishedAt || item.startedAt),
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
}

.timeline-block-container {
  display: flex;
  align-items: center;
}

.timeline-block {
  border-radius: 3px;
  transition: transform 0.15s ease, opacity 0.15s ease;
  cursor: pointer;
}

.timeline-block:hover {
  transform: scaleY(1.25);
  opacity: 0.85;
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
}
</style>

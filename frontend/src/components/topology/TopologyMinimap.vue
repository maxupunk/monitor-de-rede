<template>
  <div
    class="topology-minimap"
    role="img"
    aria-label="Visão geral do mapa — clique para navegar"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
  >
    <canvas ref="canvas" :width="width * ratio" :height="height * ratio"></canvas>
  </div>
</template>

<script setup lang="ts">
/**
 * Minimapa: redesenha só quando o canvas avisa que a cena mudou (zoom, pan,
 * nó movido, elemento novo) — nenhum temporizador.
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { MinimapData, MinimapSource } from '@/utils/topology/canvasTypes'
import type { TopologyPalette } from '@/utils/topology/style'

const props = withDefaults(
  defineProps<{
    source: MinimapSource | null
    palette: TopologyPalette
    width?: number
    height?: number
  }>(),
  { width: 184, height: 124 }
)

const canvas = ref<HTMLCanvasElement | null>(null)
const ratio = Math.min(2, window.devicePixelRatio || 1)
let unsubscribe: (() => void) | null = null
let transform: { scale: number; offsetX: number; offsetY: number } | null = null
let pointerActive = false

function colorFor(status: string): string {
  const p = props.palette
  if (status === 'offline') return p.error
  if (status === 'warning') return p.warning
  if (status === 'online') return p.success
  return p.info
}

function draw() {
  const ctx = canvas.value?.getContext('2d')
  const data: MinimapData | null = props.source?.getMinimapData() ?? null
  if (!ctx) return
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0)
  ctx.clearRect(0, 0, props.width, props.height)
  if (!data) {
    transform = null
    return
  }
  const pad = 8
  const x1 = Math.min(data.bounds.x1, data.viewport.x1)
  const y1 = Math.min(data.bounds.y1, data.viewport.y1)
  const x2 = Math.max(data.bounds.x2, data.viewport.x2)
  const y2 = Math.max(data.bounds.y2, data.viewport.y2)
  const scale = Math.min(
    (props.width - pad * 2) / Math.max(1, x2 - x1),
    (props.height - pad * 2) / Math.max(1, y2 - y1)
  )
  const offsetX = pad - x1 * scale + (props.width - pad * 2 - (x2 - x1) * scale) / 2
  const offsetY = pad - y1 * scale + (props.height - pad * 2 - (y2 - y1) * scale) / 2
  transform = { scale, offsetX, offsetY }

  const radius = data.nodes.length > 300 ? 1.6 : 2.6
  for (const node of data.nodes) {
    ctx.fillStyle = node.system ? props.palette.primary : colorFor(node.status)
    ctx.beginPath()
    ctx.arc(
      node.x * scale + offsetX,
      node.y * scale + offsetY,
      node.system ? radius + 1.5 : radius,
      0,
      Math.PI * 2
    )
    ctx.fill()
  }

  const vx = data.viewport.x1 * scale + offsetX
  const vy = data.viewport.y1 * scale + offsetY
  const vw = (data.viewport.x2 - data.viewport.x1) * scale
  const vh = (data.viewport.y2 - data.viewport.y1) * scale
  ctx.fillStyle = `${props.palette.primary}22`
  ctx.fillRect(vx, vy, vw, vh)
  ctx.strokeStyle = props.palette.primary
  ctx.lineWidth = 1.5
  ctx.strokeRect(vx, vy, vw, vh)
}

function panTo(event: PointerEvent) {
  if (!transform || !props.source || !canvas.value) return
  const rect = canvas.value.getBoundingClientRect()
  const x = (event.clientX - rect.left - transform.offsetX) / transform.scale
  const y = (event.clientY - rect.top - transform.offsetY) / transform.scale
  props.source.panToModel({ x, y })
}

function onPointerDown(event: PointerEvent) {
  pointerActive = true
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
  panTo(event)
}
function onPointerMove(event: PointerEvent) {
  if (pointerActive) panTo(event)
}
function onPointerUp() {
  pointerActive = false
}

function subscribe() {
  unsubscribe?.()
  unsubscribe = props.source?.onSceneChange(draw) ?? null
  draw()
}

onMounted(subscribe)
watch(() => props.source, subscribe)
watch(() => props.palette, draw)
onBeforeUnmount(() => unsubscribe?.())
</script>

<style scoped>
.topology-minimap {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.25);
  border-radius: 8px;
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25);
  cursor: pointer;
  touch-action: none;
  overflow: hidden;
  line-height: 0;
}
canvas {
  width: v-bind('`${width}px`');
  height: v-bind('`${height}px`');
}
</style>

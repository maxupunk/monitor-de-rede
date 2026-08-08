<template>
  <div
    class="dashboard-widget-wrapper fill-height"
    :class="{ 'is-editing': isEditMode, 'is-dragging': isDragging }"
    :draggable="isEditMode"
    @dragstart="onDragStart"
    @dragover.prevent="onDragOver"
    @drop.prevent="onDrop"
    @dragend="onDragEnd"
  >
    <div
      v-if="isEditMode"
      class="widget-edit-header d-flex align-center justify-space-between pa-2 border-b bg-surface-variant rounded-t-lg"
    >
      <div class="d-flex align-center ga-2 text-truncate">
        <v-icon class="drag-handle cursor-grab" color="primary">mdi-drag-vertical</v-icon>
        <v-icon size="18" color="primary">{{ widget.icon }}</v-icon>
        <span class="text-caption font-weight-bold text-truncate">{{ widget.title }}</span>
      </div>

      <div class="d-flex align-center ga-1">
        <v-tooltip text="Mover para cima">
          <template #activator="{ props: tooltipProps }">
            <v-btn
              v-bind="tooltipProps"
              icon
              size="x-small"
              variant="text"
              :disabled="isFirst"
              @click="$emit('move-up')"
            >
              <v-icon size="18">mdi-chevron-up</v-icon>
            </v-btn>
          </template>
        </v-tooltip>

        <v-tooltip text="Mover para baixo">
          <template #activator="{ props: tooltipProps }">
            <v-btn
              v-bind="tooltipProps"
              icon
              size="x-small"
              variant="text"
              :disabled="isLast"
              @click="$emit('move-down')"
            >
              <v-icon size="18">mdi-chevron-down</v-icon>
            </v-btn>
          </template>
        </v-tooltip>

        <v-tooltip text="Ocultar do Dashboard">
          <template #activator="{ props: tooltipProps }">
            <v-btn
              v-bind="tooltipProps"
              icon
              size="x-small"
              variant="text"
              color="error"
              @click="$emit('remove')"
            >
              <v-icon size="18">mdi-eye-off-outline</v-icon>
            </v-btn>
          </template>
        </v-tooltip>
      </div>
    </div>

    <div class="widget-body fill-height">
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { WidgetConfig } from '@/stores/dashboard'

const props = defineProps<{
  widget: WidgetConfig
  isEditMode: boolean
  isFirst?: boolean
  isLast?: boolean
}>()

const emit = defineEmits<{
  (e: 'move-up'): void
  (e: 'move-down'): void
  (e: 'remove'): void
  (e: 'reorder', draggedId: string, targetId: string): void
}>()

const isDragging = ref(false)

function onDragStart(e: DragEvent) {
  if (!props.isEditMode) return
  isDragging.value = true
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', props.widget.id)
  }
}

function onDragOver(e: DragEvent) {
  if (!props.isEditMode) return
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'move'
  }
}

function onDrop(e: DragEvent) {
  if (!props.isEditMode) return
  const draggedId = e.dataTransfer?.getData('text/plain')
  if (draggedId && draggedId !== props.widget.id) {
    emit('reorder', draggedId, props.widget.id)
  }
}

function onDragEnd() {
  isDragging.value = false
}
</script>

<style scoped>
.dashboard-widget-wrapper {
  position: relative;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
  border-radius: 8px;
}

.dashboard-widget-wrapper.is-editing {
  border: 2px dashed rgba(var(--v-theme-primary), 0.5);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
}

.dashboard-widget-wrapper.is-dragging {
  opacity: 0.5;
  border-color: rgba(var(--v-theme-primary), 0.9);
}

.cursor-grab {
  cursor: grab;
}

.cursor-grab:active {
  cursor: grabbing;
}

.ga-1 {
  gap: 4px;
}

.ga-2 {
  gap: 8px;
}
</style>

<template>
  <div
    class="widget-wrapper"
    :class="{ 'widget-wrapper--editing': isEditMode, 'widget-wrapper--dragging': isDragging }"
    :draggable="isEditMode"
    @dragstart="onDragStart"
    @dragover.prevent="onDragOver"
    @drop.prevent="onDrop"
    @dragend="isDragging = false"
  >
    <div v-if="isEditMode" class="widget-wrapper__toolbar">
      <v-icon class="widget-wrapper__handle" color="primary" size="20">mdi-drag</v-icon>
      <span class="widget-wrapper__title">{{ widget.title }}</span>

      <v-btn
        icon
        size="x-small"
        variant="text"
        color="primary"
        :disabled="isFirst"
        aria-label="Mover para antes"
        @click="emit('move-up')"
      >
        <v-icon>mdi-arrow-left-top</v-icon>
        <v-tooltip activator="parent" location="top">Mover para antes</v-tooltip>
      </v-btn>
      <v-btn
        icon
        size="x-small"
        variant="text"
        color="primary"
        :disabled="isLast"
        aria-label="Mover para depois"
        @click="emit('move-down')"
      >
        <v-icon>mdi-arrow-right-bottom</v-icon>
        <v-tooltip activator="parent" location="top">Mover para depois</v-tooltip>
      </v-btn>

      <v-menu location="bottom end">
        <template #activator="{ props: menuProps }">
          <v-btn
            v-bind="menuProps"
            icon
            size="x-small"
            variant="text"
            color="primary"
            aria-label="Largura do card"
          >
            <v-icon>mdi-arrow-expand-horizontal</v-icon>
            <v-tooltip activator="parent" location="top">Largura</v-tooltip>
          </v-btn>
        </template>
        <v-list density="compact" rounded="lg">
          <v-list-item
            v-for="(option, size) in WIDGET_SIZES"
            :key="size"
            :prepend-icon="option.icon"
            :title="option.label"
            :active="currentSize === size"
            color="primary"
            @click="emit('resize', size)"
          />
        </v-list>
      </v-menu>

      <v-btn
        icon
        size="x-small"
        variant="text"
        color="error"
        :aria-label="isCustomCard ? 'Remover card' : 'Ocultar card'"
        @click="emit('remove')"
      >
        <v-icon>{{ isCustomCard ? 'mdi-delete-outline' : 'mdi-eye-off-outline' }}</v-icon>
        <v-tooltip activator="parent" location="top">
          {{ isCustomCard ? 'Remover card' : 'Ocultar (volta pelo catálogo)' }}
        </v-tooltip>
      </v-btn>
    </div>

    <div class="widget-wrapper__body">
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  DEFAULT_WIDGETS,
  WIDGET_SIZES,
  widgetSizeOf,
  type WidgetConfig,
  type WidgetSize,
} from '@/stores/dashboard'

const props = defineProps<{
  widget: WidgetConfig
  isEditMode: boolean
  isFirst?: boolean
  isLast?: boolean
}>()

const emit = defineEmits<{
  (e: 'move-up'): void
  (e: 'move-down'): void
  (e: 'resize', size: WidgetSize): void
  (e: 'remove'): void
  (e: 'reorder', draggedId: string, targetId: string): void
}>()

const isDragging = ref(false)
const currentSize = computed(() => widgetSizeOf(props.widget))
const isCustomCard = computed(() => !DEFAULT_WIDGETS.some((def) => def.id === props.widget.id))

function onDragStart(e: DragEvent) {
  if (!props.isEditMode) return
  isDragging.value = true
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', props.widget.id)
  }
}

function onDragOver(e: DragEvent) {
  if (props.isEditMode && e.dataTransfer) e.dataTransfer.dropEffect = 'move'
}

function onDrop(e: DragEvent) {
  if (!props.isEditMode) return
  const draggedId = e.dataTransfer?.getData('text/plain')
  if (draggedId && draggedId !== props.widget.id) emit('reorder', draggedId, props.widget.id)
}
</script>

<style scoped>
.widget-wrapper {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  border-radius: 12px;
  transition:
    outline-color 0.15s ease,
    opacity 0.15s ease;
}

.widget-wrapper__body {
  flex: 1 1 auto;
  min-height: 0;
}

/* Na edição, contorno tracejado só como "alça" — sai junto com o modo. */
.widget-wrapper--editing {
  outline: 2px dashed rgba(var(--v-theme-primary), 0.55);
  outline-offset: 4px;
  cursor: grab;
}

.widget-wrapper--editing .widget-wrapper__body {
  pointer-events: none;
}

.widget-wrapper--dragging {
  opacity: 0.45;
}

.widget-wrapper__toolbar {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-bottom: 8px;
  padding: 2px 4px 2px 8px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.1);
}

.widget-wrapper__handle {
  cursor: grab;
}

.widget-wrapper__title {
  flex: 1 1 auto;
  min-width: 0;
  margin-left: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.8rem;
  font-weight: 700;
}
</style>

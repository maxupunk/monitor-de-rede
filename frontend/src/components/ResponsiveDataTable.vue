<template>
  <div>
    <!-- Desktop: tabela tradicional -->
    <VDataTable
      v-if="$vuetify.display.mdAndUp"
      :headers="props.headers"
      :items="props.items"
      :search="props.search"
      :loading="props.loading"
      :items-per-page="props.itemsPerPage"
      :hide-default-footer="props.hideDefaultFooter"
      :no-data-text="props.noDataText"
      class="elevation-0"
      @click:row="onClickRow"
    >
      <template v-for="name in desktopSlotNames" :key="name" #[name]="slotProps">
        <slot :name="name" v-bind="slotProps || {}" />
      </template>
    </VDataTable>

    <!-- Mobile: lista de cards -->
    <div v-else class="mobile-card-list">
      <template v-if="items.length > 0">
        <v-card
          v-for="(item, index) in items"
          :key="itemKey ? itemKey(item) : index"
          class="mobile-card rounded-0"
          :class="{ 'cursor-pointer': clickable }"
          @click="clickable ? onCardClick(item) : undefined"
        >
          <slot name="mobile-item" :item="item" />
        </v-card>
      </template>

      <v-card
        v-else-if="!loading"
        class="mobile-card pa-6 text-center text-grey"
        variant="outlined"
      >
        <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-inbox-outline</v-icon>
        <div class="text-subtitle-2 font-weight-medium">{{ noDataText }}</div>
      </v-card>

      <div v-else class="pa-4 text-center">
        <v-progress-circular indeterminate color="primary"></v-progress-circular>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, useSlots } from 'vue'
import { VDataTable } from 'vuetify/components'
import type { VNode } from 'vue'

const props = defineProps<{
  headers: { title: string; key: string; width?: string; sortable?: boolean }[]
  items: any[]
  loading?: boolean
  search?: string
  noDataText?: string
  itemsPerPage?: number
  hideDefaultFooter?: boolean
  clickable?: boolean
  itemKey?: (item: any) => string | number
}>()

const emit = defineEmits<{
  (e: 'click:row', event: MouseEvent, row: { item: any }): void
}>()

defineSlots<{
  default?: (props: Record<string, never>) => VNode[]
  'mobile-item'?: (props: { item: any }) => VNode[]
  [key: string]: ((props: any) => VNode[]) | undefined
}>()

const slots = useSlots()

const desktopSlotNames = computed(() => Object.keys(slots).filter((key) => key !== 'mobile-item'))

function onCardClick(item: any) {
  emit('click:row', new MouseEvent('click'), { item })
}

function onClickRow(event: MouseEvent, row: { item: any }) {
  emit('click:row', event, row)
}
</script>

<style scoped>
.mobile-card-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.mobile-card {
  padding: 12px;
}
@media (min-width: 960px) {
  .mobile-card-list {
    gap: 12px;
  }
  .mobile-card {
    border-radius: 8px;
    padding: 12px 16px;
  }
}
.cursor-pointer {
  cursor: pointer;
}
</style>

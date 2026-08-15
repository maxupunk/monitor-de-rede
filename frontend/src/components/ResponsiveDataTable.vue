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
    <div v-else class="d-flex flex-column ga-2">
      <template v-if="items.length > 0">
        <!-- `border` porque o card do item fica dentro do card da página, com a
             mesma cor de surface: sem contorno os dois viram um bloco só, e com
             um único item não há nem o espaçamento da lista para separá-los. -->
        <v-card
          v-for="(item, index) in items"
          :key="itemKey ? itemKey(item) : index"
          border
          rounded="0"
          class="pa-3"
          :class="{ 'cursor-pointer': clickable }"
          @click="clickable ? onCardClick(item) : undefined"
        >
          <slot name="mobile-item" :item="item" />
        </v-card>
      </template>

      <v-card
        v-else-if="!loading"
        variant="outlined"
        rounded="0"
        class="pa-6 text-center text-grey"
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

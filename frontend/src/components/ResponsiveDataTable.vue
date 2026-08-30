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
      :hover="props.clickable"
      :show-select="props.showSelect"
      :item-value="props.itemValue || 'id'"
      :model-value="props.modelValue"
      class="elevation-0"
      :class="{ 'linha-clicavel': props.clickable }"
      @update:model-value="emit('update:modelValue', $event)"
      @click:row="onClickRow"
    >
      <template v-for="name in desktopSlotNames" :key="name" #[name]="slotProps">
        <slot :name="name" v-bind="slotProps || {}" />
      </template>
    </VDataTable>

    <!-- Mobile: lista de cards -->
    <div v-else class="d-flex flex-column ga-2 pa-2">
      <template v-if="items.length > 0">
        <!-- `border` porque o card do item fica dentro do card da página, com a
             mesma cor de surface: sem contorno os dois viram um bloco só, e com
             um único item não há nem o espaçamento da lista para separá-los. -->
        <v-card
          v-for="(item, index) in items"
          :key="itemKey ? itemKey(item) : index"
          border
          rounded="lg"
          class="pa-3 transition-swing"
          :class="{
            'cursor-pointer': clickable,
            'selected-card border-primary': showSelect && isMobileSelected(item),
          }"
          @click="clickable ? onCardClick(item) : undefined"
        >
          <div class="d-flex align-start ga-2">
            <div v-if="showSelect" class="pt-0.5" @click.stop>
              <v-checkbox-btn
                :model-value="isMobileSelected(item)"
                color="primary"
                density="compact"
                hide-details
                @update:model-value="toggleMobileSelect(item, $event)"
              ></v-checkbox-btn>
            </div>
            <div class="flex-grow-1 min-w-0">
              <slot name="mobile-item" :item="item" />
            </div>
          </div>
        </v-card>
      </template>

      <v-card
        v-else-if="!loading"
        variant="outlined"
        rounded="lg"
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
  showSelect?: boolean
  modelValue?: any[]
  itemValue?: string
  itemKey?: (item: any) => string | number
}>()

const emit = defineEmits<{
  (e: 'click:row', event: MouseEvent, row: { item: any }): void
  (e: 'update:modelValue', value: any[]): void
}>()

function isMobileSelected(item: any): boolean {
  if (!props.modelValue) return false
  const valKey = props.itemValue || 'id'
  const val = item[valKey] !== undefined ? item[valKey] : item
  return props.modelValue.includes(val)
}

function toggleMobileSelect(item: any, isSelected: boolean | null) {
  const current = Array.isArray(props.modelValue) ? [...props.modelValue] : []
  const valKey = props.itemValue || 'id'
  const val = item[valKey] !== undefined ? item[valKey] : item
  if (isSelected) {
    if (!current.includes(val)) current.push(val)
  } else {
    const idx = current.indexOf(val)
    if (idx !== -1) current.splice(idx, 1)
  }
  emit('update:modelValue', current)
}

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
  // O `VDataTable` emite `click:row` sempre; quem não declarou `clickable` não
  // pode ganhar comportamento de clique só por montar a tabela.
  if (!props.clickable) return
  emit('click:row', event, row)
}
</script>

<style scoped>
/* O cursor é o que diz que a linha inteira é o alvo. Sem ele, o operador
   continua mirando o texto do nome, que é o que a linha clicável veio
   resolver. */
.linha-clicavel :deep(tbody tr) {
  cursor: pointer;
}

.selected-card {
  background-color: rgba(var(--v-theme-primary), 0.08) !important;
}
</style>

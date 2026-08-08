<template>
  <v-dialog v-model="isOpen" max-width="800" scrollable>
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
        <div class="d-flex align-center ga-2">
          <v-avatar color="primary" variant="tonal" size="36">
            <v-icon color="primary">mdi-plus-box-multiple-outline</v-icon>
          </v-avatar>
          <div>
            <div class="text-h6 font-weight-bold">Catálogo de Widgets</div>
            <div class="text-caption text-grey">
              Personalize seu Dashboard adicionando ou reativando painéis
            </div>
          </div>
        </div>
        <v-btn icon variant="text" size="small" @click="isOpen = false">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-divider></v-divider>

      <v-card-text class="pa-4">
        <div class="d-flex flex-column flex-sm-row align-center justify-space-between ga-3 mb-4">
          <v-tabs v-model="selectedTab" color="primary" density="compact" class="w-100 w-sm-auto">
            <v-tab value="all">Todos ({{ dashboardStore.sortedWidgets.length }})</v-tab>
            <v-tab value="summary">Resumo</v-tab>
            <v-tab value="lists">Listas & Eventos</v-tab>
            <v-tab value="charts">Gráficos Grafana</v-tab>
          </v-tabs>

          <v-text-field
            v-model="searchQuery"
            density="compact"
            variant="outlined"
            placeholder="Buscar widget..."
            prepend-inner-icon="mdi-magnify"
            hide-details
            clearable
            style="max-width: 260px"
            class="w-100 w-sm-auto"
          ></v-text-field>
        </div>

        <v-row v-if="filteredWidgets.length > 0">
          <v-col v-for="widget in filteredWidgets" :key="widget.id" cols="12" sm="6">
            <v-card
              variant="outlined"
              class="h-100 d-flex flex-column rounded-lg transition-all"
              :class="{ 'border-primary': widget.visible }"
            >
              <v-card-item class="pb-2">
                <template #prepend>
                  <v-avatar
                    :color="widget.visible ? 'primary' : 'grey-lighten-1'"
                    variant="tonal"
                    size="40"
                    class="mr-3"
                  >
                    <v-icon>{{ widget.icon }}</v-icon>
                  </v-avatar>
                </template>
                <v-card-title class="text-subtitle-1 font-weight-bold">
                  {{ widget.title }}
                </v-card-title>
                <v-card-subtitle class="mt-1">
                  <v-chip size="x-small" :color="categoryColor(widget.category)" variant="tonal">
                    {{ categoryLabel(widget.category) }}
                  </v-chip>
                </v-card-subtitle>
              </v-card-item>

              <v-card-text class="pt-0 text-caption text-grey-darken-1 flex-grow-1">
                {{ widget.description }}
              </v-card-text>

              <v-divider></v-divider>

              <v-card-actions class="pa-3 justify-end bg-surface-light">
                <v-btn
                  v-if="!widget.visible"
                  color="primary"
                  variant="flat"
                  size="small"
                  prepend-icon="mdi-plus"
                  @click="dashboardStore.toggleWidgetVisibility(widget.id, true)"
                >
                  Adicionar ao Dashboard
                </v-btn>
                <v-chip v-else color="success" variant="tonal" size="small">
                  <v-icon start size="14">mdi-check-circle-outline</v-icon>
                  Já no Dashboard
                </v-chip>
              </v-card-actions>
            </v-card>
          </v-col>
        </v-row>

        <div v-else class="pa-8 text-center text-grey">
          <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-magnify-remove</v-icon>
          <div class="text-subtitle-1 font-weight-medium">Nenhum widget encontrado</div>
          <div class="text-caption">Tente buscar por outro termo ou selecione outra categoria.</div>
        </div>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 justify-space-between">
        <v-btn
          color="warning"
          variant="outlined"
          size="small"
          prepend-icon="mdi-restore"
          @click="dashboardStore.resetToDefaultLayout()"
        >
          Restaurar Padrão
        </v-btn>
        <v-btn color="primary" variant="flat" size="small" @click="isOpen = false">
          Concluir
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useDashboardStore, type WidgetCategory } from '@/stores/dashboard'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
}>()

const dashboardStore = useDashboardStore()
const selectedTab = ref<'all' | WidgetCategory>('all')
const searchQuery = ref('')

const isOpen = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
})

const filteredWidgets = computed(() => {
  return dashboardStore.sortedWidgets.filter((widget) => {
    const matchesTab = selectedTab.value === 'all' || widget.category === selectedTab.value
    const q = searchQuery.value.trim().toLowerCase()
    const matchesSearch =
      !q || widget.title.toLowerCase().includes(q) || widget.description.toLowerCase().includes(q)
    return matchesTab && matchesSearch
  })
})

function categoryLabel(cat: WidgetCategory): string {
  switch (cat) {
    case 'summary':
      return 'Resumo'
    case 'lists':
      return 'Listas & Eventos'
    case 'charts':
      return 'Gráficos Grafana'
    default:
      return 'Geral'
  }
}

function categoryColor(cat: WidgetCategory): string {
  switch (cat) {
    case 'summary':
      return 'primary'
    case 'lists':
      return 'info'
    case 'charts':
      return 'deep-purple'
    default:
      return 'grey'
  }
}
</script>

<style scoped>
.ga-2 {
  gap: 8px;
}
.ga-3 {
  gap: 12px;
}
.transition-all {
  transition: all 0.2s ease-in-out;
}
</style>

<template>
  <div class="topology-toolbar px-2 px-md-3" role="toolbar" aria-label="Ferramentas do mapa">
    <div class="d-none d-xl-flex align-center mr-2 flex-shrink-0">
      <v-icon color="primary" size="22" class="mr-2">mdi-sitemap</v-icon>
      <span class="text-subtitle-1 font-weight-bold">Topologia</span>
    </div>

    <v-select
      :model-value="siteId"
      :items="siteItems"
      item-title="title"
      item-value="value"
      density="compact"
      variant="outlined"
      hide-details
      :prepend-inner-icon="compact ? undefined : 'mdi-office-building-marker'"
      class="site-select flex-shrink-0"
      aria-label="Site exibido"
      @update:model-value="$emit('update:siteId', $event ?? null)"
    ></v-select>

    <button type="button" class="search-trigger" @click="$emit('search')">
      <v-icon size="18" color="primary">mdi-magnify</v-icon>
      <span class="search-text">Buscar dispositivo, IP…</span>
      <kbd class="d-none d-md-inline">Ctrl K</kbd>
    </button>

    <template v-if="!compact">
      <v-menu :close-on-content-click="false" location="bottom start" max-width="440">
        <template #activator="{ props: menu }">
          <v-btn v-bind="menu" color="primary" variant="tonal" prepend-icon="mdi-filter-variant">
            Filtros
            <v-badge
              v-if="filterCount > 0"
              :content="filterCount"
              color="error"
              inline
              class="ml-1"
            ></v-badge>
          </v-btn>
        </template>
        <v-card class="pa-4" min-width="360">
          <TopologyFilterPanel
            :model-value="filters"
            :nodes="nodes"
            @update:model-value="$emit('update:filters', $event)"
          />
        </v-card>
      </v-menu>

      <v-menu location="bottom start">
        <template #activator="{ props: menu }">
          <v-btn
            v-bind="menu"
            color="primary"
            variant="tonal"
            :prepend-icon="computing ? undefined : 'mdi-auto-fix'"
            :loading="computing !== null"
          >
            Organizar
          </v-btn>
        </template>
        <TopologyLayoutList
          :can-write="canWrite"
          :new-count="newCount"
          @layout="$emit('layout', $event)"
          @arrange-new="$emit('arrange-new')"
        />
      </v-menu>

      <v-menu :close-on-content-click="false" location="bottom start">
        <template #activator="{ props: menu }">
          <v-btn v-bind="menu" color="primary" variant="tonal" prepend-icon="mdi-eye-settings">
            Exibir
          </v-btn>
        </template>
        <TopologyViewOptions
          :legend-open="legendOpen"
          :group-by-site="groupBySite"
          :always-detail="alwaysDetail"
          :orthogonal-edges="orthogonalEdges"
          @update:legend-open="$emit('update:legendOpen', $event)"
          @update:group-by-site="$emit('update:groupBySite', $event)"
          @update:always-detail="$emit('update:alwaysDetail', $event)"
          @update:orthogonal-edges="$emit('update:orthogonalEdges', $event)"
        />
      </v-menu>

      <v-menu v-if="canWrite" location="bottom start">
        <template #activator="{ props: menu }">
          <v-btn v-bind="menu" color="primary" variant="tonal" prepend-icon="mdi-plus">
            Adicionar
          </v-btn>
        </template>
        <TopologyAddList @add="$emit('add', $event)" />
      </v-menu>
      <v-tooltip v-else :text="READ_ONLY_REASON" location="bottom">
        <template #activator="{ props: tip }">
          <span v-bind="tip">
            <v-btn color="primary" variant="tonal" prepend-icon="mdi-plus" disabled>
              Adicionar
            </v-btn>
          </span>
        </template>
      </v-tooltip>
    </template>

    <v-spacer></v-spacer>

    <v-tooltip :text="editTooltip" location="bottom">
      <template #activator="{ props: tip }">
        <span v-bind="tip">
          <v-btn
            v-if="compact"
            :color="editMode ? 'warning' : 'primary'"
            variant="flat"
            :icon="editMode ? 'mdi-lock-open-variant' : 'mdi-pencil'"
            :disabled="!canWrite"
            :aria-pressed="editMode"
            aria-label="Editar mapa"
            @click="$emit('update:editMode', !editMode)"
          ></v-btn>
          <v-btn
            v-else
            :color="editMode ? 'warning' : 'primary'"
            variant="flat"
            :prepend-icon="editMode ? 'mdi-lock-open-variant' : 'mdi-pencil'"
            :disabled="!canWrite"
            :aria-pressed="editMode"
            @click="$emit('update:editMode', !editMode)"
          >
            {{ editMode ? 'Editando' : 'Editar mapa' }}
          </v-btn>
        </span>
      </template>
    </v-tooltip>

    <v-menu v-if="!compact" location="bottom end">
      <template #activator="{ props: menu }">
        <v-btn
          v-bind="menu"
          icon="mdi-dots-vertical"
          color="primary"
          variant="flat"
          size="small"
          aria-label="Mais ações do mapa"
        ></v-btn>
      </template>
      <v-list density="comfortable" min-width="240">
        <v-list-item
          prepend-icon="mdi-image-move"
          title="Exportar imagem (PNG)"
          base-color="primary"
          @click="$emit('export')"
        ></v-list-item>
        <v-list-item
          :prepend-icon="fullscreen ? 'mdi-fullscreen-exit' : 'mdi-fullscreen'"
          :title="fullscreen ? 'Sair da tela cheia' : 'Tela cheia'"
          base-color="primary"
          @click="$emit('fullscreen')"
        ></v-list-item>
        <v-list-item
          prepend-icon="mdi-keyboard-outline"
          title="Atalhos de teclado"
          subtitle="Tecla ?"
          base-color="primary"
          @click="$emit('help')"
        ></v-list-item>
      </v-list>
    </v-menu>

    <v-btn
      v-else
      icon
      color="primary"
      variant="flat"
      aria-label="Mais opções do mapa"
      @click="$emit('more')"
    >
      <v-badge v-if="filterCount > 0" :content="filterCount" color="error" floating>
        <v-icon>mdi-tune-variant</v-icon>
      </v-badge>
      <v-icon v-else>mdi-tune-variant</v-icon>
    </v-btn>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { Site } from '@/stores/sites'
import type { TopologyNode } from '@/stores/topology'
import type { TopologyFilters } from '@/utils/topology/filters'
import type { LayoutKind } from '@/utils/topology/layouts'
import { READ_ONLY_REASON } from '@/utils/topology/inspectorRegistry'
import TopologyFilterPanel from './TopologyFilterPanel.vue'
import TopologyLayoutList from './TopologyLayoutList.vue'
import TopologyViewOptions from './TopologyViewOptions.vue'
import TopologyAddList, { type AddKind } from './TopologyAddList.vue'

const props = defineProps<{
  sites: Site[]
  siteId: number | null
  nodes: readonly TopologyNode[]
  filters: TopologyFilters
  filterCount: number
  editMode: boolean
  canWrite: boolean
  legendOpen: boolean
  groupBySite: boolean
  alwaysDetail: boolean
  orthogonalEdges: boolean
  computing: LayoutKind | null
  newCount: number
  fullscreen: boolean
  /** Celular/tablet estreito: o resto vai para a folha "Mais opções". */
  compact: boolean
}>()

defineEmits<{
  'update:siteId': [value: number | null]
  'update:filters': [value: TopologyFilters]
  'update:editMode': [value: boolean]
  'update:legendOpen': [value: boolean]
  'update:groupBySite': [value: boolean]
  'update:alwaysDetail': [value: boolean]
  'update:orthogonalEdges': [value: boolean]
  search: []
  layout: [kind: LayoutKind]
  'arrange-new': []
  add: [kind: AddKind]
  export: []
  fullscreen: []
  help: []
  more: []
}>()

const siteItems = computed(() => [
  { title: 'Todos os sites', value: null },
  ...props.sites.map((site) => ({ title: site.name, value: site.id })),
])

const editTooltip = computed(() => {
  if (!props.canWrite) return READ_ONLY_REASON
  return props.editMode
    ? 'Arrastar nós e puxar cabos está ligado (E para desligar)'
    : 'Ligue para arrastar nós e puxar cabos (E)'
})
</script>

<style scoped>
.topology-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 56px;
  background: rgb(var(--v-theme-surface));
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.14);
  position: relative;
  z-index: 4;
}
.site-select {
  flex: 0 1 210px;
  min-width: 140px;
}
.search-trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 40px;
  flex: 0 1 250px;
  min-width: 120px;
  padding: 0 10px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.38);
  color: rgb(var(--v-theme-on-surface));
  background: rgb(var(--v-theme-surface));
  cursor: text;
}
.search-trigger:hover,
.search-trigger:focus-visible {
  border-color: rgb(var(--v-theme-primary));
  outline: none;
}
.search-text {
  flex: 1;
  text-align: left;
  font-size: 0.875rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
kbd {
  font-family: inherit;
  font-size: 0.7rem;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.4);
  color: rgb(var(--v-theme-on-surface));
}
@media (max-width: 1279px) {
  .search-trigger {
    flex: 1 1 auto;
  }
}
@media (max-width: 599px) {
  .site-select {
    flex-basis: 128px;
    min-width: 112px;
  }
  .search-trigger {
    min-width: 0;
  }
}
</style>

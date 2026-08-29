<template>
  <div
    class="topology-controls-bar d-flex align-center flex-wrap gap-2 pa-2 rounded-xl elevation-4"
  >
    <!-- Ações Primárias de Gestão de Topologia -->
    <template v-if="canWrite !== false">
      <!-- Botão Adicionar Conexão -->
      <v-tooltip text="Criar nova conexão entre dispositivos" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            color="primary"
            variant="elevated"
            class="rounded-lg px-3 font-weight-bold font-sm shadow-sm"
            prepend-icon="mdi-plus"
            @click="$emit('add-link')"
          >
            <span class="d-none d-sm-inline">Conexão</span>
          </v-btn>
        </template>
      </v-tooltip>

      <!-- Botão Adicionar Switch -->
      <v-tooltip text="Adicionar Switch ou Hub não gerenciável" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            color="indigo"
            variant="tonal"
            class="rounded-lg px-3 font-weight-medium font-sm"
            prepend-icon="mdi-hub"
            @click="$emit('add-switch')"
          >
            <span class="d-none d-md-inline">Switch</span>
          </v-btn>
        </template>
      </v-tooltip>
    </template>

    <!-- Botão Recalcular Topologia -->
    <v-tooltip text="Reconstruir topologia a partir das rotas e SNMP" location="bottom">
      <template #activator="{ props: tooltipProps }">
        <v-btn
          v-bind="tooltipProps"
          color="secondary"
          variant="tonal"
          class="rounded-lg px-3 font-weight-medium font-sm"
          prepend-icon="mdi-calculator"
          :loading="recalculating"
          @click="$emit('recalculate')"
        >
          <span class="d-none d-md-inline">Recalcular</span>
        </v-btn>
      </template>
    </v-tooltip>

    <v-divider vertical class="mx-1 my-1"></v-divider>

    <!-- Ferramenta de Conexão com o Mouse (Cabo) -->
    <v-tooltip
      v-if="canWrite !== false"
      text="Ferramenta de cabo: ligue dois equipamentos clicando neles"
      location="bottom"
    >
      <template #activator="{ props: tooltipProps }">
        <v-btn
          v-bind="tooltipProps"
          :color="isConnectMode ? 'primary' : 'surface'"
          :variant="isConnectMode ? 'flat' : 'tonal'"
          class="rounded-lg px-3 font-weight-bold font-sm shadow-sm"
          prepend-icon="mdi-vector-polyline"
          @click="$emit('toggle-connect-mode')"
        >
          <span class="d-none d-sm-inline">
            {{ isConnectMode ? 'Conectando...' : 'Ligar com Mouse' }}
          </span>
          <span class="d-inline d-sm-none">Ligar</span>
          <v-badge v-if="isConnectMode" color="error" dot floating class="ml-1"></v-badge>
        </v-btn>
      </template>
    </v-tooltip>

    <v-divider v-if="canWrite !== false" vertical class="mx-1 my-1"></v-divider>

    <!-- Controles de Zoom e Enquadramento -->
    <div class="d-flex align-center bg-surface-variant-subtle rounded-lg pa-1">
      <v-tooltip text="Aumentar Zoom (+)" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon="mdi-plus"
            variant="text"
            density="compact"
            size="small"
            @click="$emit('zoom-in')"
          ></v-btn>
        </template>
      </v-tooltip>

      <span class="text-caption font-weight-bold px-1 user-select-none font-mono">
        {{ Math.round(zoomLevel * 100) }}%
      </span>

      <v-tooltip text="Diminuir Zoom (-)" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon="mdi-minus"
            variant="text"
            density="compact"
            size="small"
            @click="$emit('zoom-out')"
          ></v-btn>
        </template>
      </v-tooltip>

      <v-tooltip text="Resetar Zoom (100%)" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon="mdi-restore"
            variant="text"
            density="compact"
            size="small"
            @click="$emit('zoom-reset')"
          ></v-btn>
        </template>
      </v-tooltip>

      <v-tooltip text="Centralizar e Enquadrar Nós (Fit)" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon="mdi-fit-to-screen-outline"
            variant="text"
            density="compact"
            size="small"
            color="primary"
            @click="$emit('fit-screen')"
          ></v-btn>
        </template>
      </v-tooltip>
    </div>

    <v-divider vertical class="mx-1 my-1"></v-divider>

    <!-- Menu de Layouts Automáticos -->
    <v-menu location="bottom end">
      <template #activator="{ props: menuProps }">
        <v-btn
          v-bind="menuProps"
          variant="tonal"
          class="rounded-lg px-2 text-capitalize"
          prepend-icon="mdi-graph-outline"
          append-icon="mdi-chevron-down"
        >
          <span class="d-none d-md-inline">Auto-Layout</span>
        </v-btn>
      </template>
      <v-list density="compact" class="rounded-lg elevation-4">
        <v-list-item
          prepend-icon="mdi-file-tree"
          title="Hierárquico (Árvore)"
          subtitle="Gateway no topo, switches no meio e estações"
          @click="$emit('apply-layout', 'hierarchical')"
        ></v-list-item>
        <v-list-item
          prepend-icon="mdi-molecule"
          title="Orgânico (Força Dirigida)"
          subtitle="Distribui nós conectados equilibradamente"
          @click="$emit('apply-layout', 'force')"
        ></v-list-item>
        <v-list-item
          prepend-icon="mdi-circle-slice-8"
          title="Radial / Circular"
          subtitle="Organização em anel ao redor do centro"
          @click="$emit('apply-layout', 'radial')"
        ></v-list-item>
        <v-list-item
          prepend-icon="mdi-view-grid-outline"
          title="Grade / Alinhamento"
          subtitle="Organiza os nós em grade uniforme"
          @click="$emit('apply-layout', 'grid')"
        ></v-list-item>
      </v-list>
    </v-menu>

    <!-- Filtro de Tipo de Dispositivo -->
    <v-menu location="bottom end">
      <template #activator="{ props: menuProps }">
        <v-btn
          v-bind="menuProps"
          variant="tonal"
          class="rounded-lg px-2 text-capitalize"
          prepend-icon="mdi-filter-variant"
        >
          <span class="d-none d-lg-inline">Filtros</span>
          <v-badge v-if="activeTypeFilter" color="primary" dot class="ml-1"></v-badge>
        </v-btn>
      </template>
      <v-list density="compact" class="rounded-lg elevation-4">
        <v-list-item
          :active="!activeTypeFilter"
          title="Mostrar Todos"
          prepend-icon="mdi-devices"
          @click="$emit('filter-type', null)"
        ></v-list-item>
        <v-divider class="my-1"></v-divider>
        <v-list-item
          :active="activeTypeFilter === 'router'"
          title="Roteadores & Gateways"
          prepend-icon="mdi-router"
          @click="$emit('filter-type', 'router')"
        ></v-list-item>
        <v-list-item
          :active="activeTypeFilter === 'switch'"
          title="Switches Gerenciáveis"
          prepend-icon="mdi-expansion-card"
          @click="$emit('filter-type', 'switch')"
        ></v-list-item>
        <v-list-item
          :active="activeTypeFilter === 'unmanaged_switch'"
          title="Switches Burros (Hubs)"
          prepend-icon="mdi-hub"
          @click="$emit('filter-type', 'unmanaged_switch')"
        ></v-list-item>
        <v-list-item
          :active="activeTypeFilter === 'server'"
          title="Servidores & Hosts"
          prepend-icon="mdi-server"
          @click="$emit('filter-type', 'server')"
        ></v-list-item>
        <v-list-item
          :active="activeTypeFilter === 'ap'"
          title="Pontos de Acesso Wi-Fi"
          prepend-icon="mdi-access-point"
          @click="$emit('filter-type', 'ap')"
        ></v-list-item>
      </v-list>
    </v-menu>

    <!-- Legenda Rápida -->
    <v-menu location="bottom end">
      <template #activator="{ props: menuProps }">
        <v-btn
          v-bind="menuProps"
          icon="mdi-help-circle-outline"
          variant="text"
          density="comfortable"
          size="small"
        ></v-btn>
      </template>
      <v-card class="pa-4 rounded-lg elevation-6" max-width="320">
        <div class="font-weight-bold text-subtitle-2 mb-2">Legenda de Conexões</div>
        <div class="d-flex align-center gap-2 mb-2">
          <div class="legend-line" style="background: #2196f3"></div>
          <span class="text-caption">Cabo Ethernet / UTP Manual</span>
        </div>
        <div class="d-flex align-center gap-2 mb-2">
          <div class="legend-line" style="background: #9c27b0"></div>
          <span class="text-caption">Fibra Óptica (SFP)</span>
        </div>
        <div class="d-flex align-center gap-2 mb-2">
          <div class="legend-line" style="background: #00bcd4"></div>
          <span class="text-caption">LLDP / CDP (SNMP)</span>
        </div>
        <div class="d-flex align-center gap-2 mb-2">
          <div class="legend-line" style="background: #ff9800; border-style: dashed"></div>
          <span class="text-caption">Hierarquia de Pai / Sub-rede</span>
        </div>
        <v-divider class="my-2"></v-divider>
        <div class="text-caption text-medium-emphasis">
          <strong>Dica:</strong> Arraste nós livremente. Pressione e arraste o fundo para mover a
          tela (Pan). Use a rolagem do mouse para dar Zoom.
        </div>
      </v-card>
    </v-menu>

    <!-- Indicador de Modo Somente Leitura se aplicável -->
    <v-chip
      v-if="canWrite === false"
      size="small"
      color="info"
      variant="tonal"
      class="font-weight-medium"
    >
      <v-icon start size="14">mdi-eye-outline</v-icon>
      Modo Leitura
    </v-chip>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  zoomLevel: number
  isConnectMode: boolean
  activeTypeFilter?: string | null
  recalculating?: boolean
  canWrite?: boolean
}>()

defineEmits<{
  (e: 'zoom-in'): void
  (e: 'zoom-out'): void
  (e: 'zoom-reset'): void
  (e: 'fit-screen'): void
  (e: 'toggle-connect-mode'): void
  (e: 'apply-layout', layout: 'hierarchical' | 'force' | 'radial' | 'grid'): void
  (e: 'filter-type', type: string | null): void
  (e: 'add-link'): void
  (e: 'add-switch'): void
  (e: 'recalculate'): void
}>()
</script>

<style scoped>
.topology-controls-bar {
  background: rgba(var(--v-theme-surface), 0.9);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(var(--v-theme-outline), 0.15);
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.2);
}
.bg-surface-variant-subtle {
  background: rgba(var(--v-theme-surface-variant), 0.35);
}
.gap-2 {
  gap: 8px;
}
.font-mono {
  font-family: monospace;
}
.legend-line {
  width: 24px;
  height: 4px;
  border-radius: 2px;
}
</style>

<template>
  <div
    class="topology-controls-bar d-flex align-center flex-nowrap pa-2 px-3 rounded-xl elevation-6"
  >
    <!-- Grupo 1: Ações de Gestão e Conexão -->
    <div class="controls-group">
      <template v-if="canWrite !== false">
        <!-- Botão Adicionar Conexão -->
        <v-tooltip text="Criar nova conexão entre dispositivos" location="bottom">
          <template #activator="{ props: tooltipProps }">
            <v-btn
              v-bind="tooltipProps"
              color="primary"
              variant="elevated"
              class="control-btn btn-primary font-weight-bold"
              aria-label="Criar nova conexão"
              @click="$emit('add-link')"
            >
              <v-icon size="20" class="mr-lg-1">mdi-plus</v-icon>
              <span class="d-none d-lg-inline text-caption font-weight-bold">Conexão</span>
            </v-btn>
          </template>
        </v-tooltip>

        <!-- Botão Adicionar Switch -->
        <v-tooltip text="Adicionar Switch ou Hub não gerenciável" location="bottom">
          <template #activator="{ props: tooltipProps }">
            <v-btn
              v-bind="tooltipProps"
              class="control-btn btn-switch font-weight-medium"
              aria-label="Adicionar switch"
              @click="$emit('add-switch')"
            >
              <v-icon size="20" class="mr-xl-1">mdi-hub</v-icon>
              <span class="d-none d-xl-inline text-caption">Switch</span>
            </v-btn>
          </template>
        </v-tooltip>
      </template>

      <!-- Botão Recalcular Topologia -->
      <v-tooltip text="Reconstruir topologia a partir das rotas e SNMP" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            class="control-btn btn-recalculate font-weight-medium"
            :loading="recalculating"
            aria-label="Recalcular topologia"
            @click="$emit('recalculate')"
          >
            <v-icon size="20" class="mr-xl-1">mdi-calculator</v-icon>
            <span class="d-none d-xl-inline text-caption">Recalcular</span>
          </v-btn>
        </template>
      </v-tooltip>

      <!-- Ferramenta de Conexão com Mouse / Toque (Cabo) -->
      <v-tooltip
        v-if="canWrite !== false"
        text="Ferramenta de cabo: ligue dois equipamentos tocando ou clicando neles"
        location="bottom"
      >
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            class="control-btn btn-cable font-weight-bold"
            :class="{ 'btn-cable-active': isConnectMode }"
            aria-label="Modo de conexão por cabo"
            @click="$emit('toggle-connect-mode')"
          >
            <v-icon size="20" class="mr-xl-1">mdi-vector-polyline</v-icon>
            <span class="d-none d-xl-inline text-caption">
              {{ isConnectMode ? 'Conectando...' : 'Ligar' }}
            </span>
            <v-badge v-if="isConnectMode" color="error" dot floating class="ml-1"></v-badge>
          </v-btn>
        </template>
      </v-tooltip>
    </div>

    <v-divider vertical class="custom-divider mx-2 mx-sm-3"></v-divider>

    <!-- Grupo 2: Controles de Zoom e Enquadramento -->
    <div class="zoom-controls-group rounded-lg flex-nowrap">
      <v-tooltip text="Aumentar Zoom (+)" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon="mdi-plus"
            variant="text"
            density="comfortable"
            size="small"
            class="zoom-btn"
            aria-label="Aumentar zoom"
            @click="$emit('zoom-in')"
          ></v-btn>
        </template>
      </v-tooltip>

      <span class="text-caption font-weight-bold px-1 user-select-none font-mono zoom-text">
        {{ Math.round(zoomLevel * 100) }}%
      </span>

      <v-tooltip text="Diminuir Zoom (-)" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon="mdi-minus"
            variant="text"
            density="comfortable"
            size="small"
            class="zoom-btn"
            aria-label="Diminuir zoom"
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
            density="comfortable"
            size="small"
            class="zoom-btn"
            aria-label="Resetar zoom"
            @click="$emit('zoom-reset')"
          ></v-btn>
        </template>
      </v-tooltip>

      <v-tooltip text="Centralizar e Mostrar Tudo no Mapa (Fit)" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon="mdi-fit-to-screen-outline"
            variant="text"
            density="comfortable"
            size="small"
            class="zoom-btn btn-fit-screen"
            aria-label="Enquadrar na tela"
            @click="$emit('fit-screen')"
          ></v-btn>
        </template>
      </v-tooltip>
    </div>

    <v-divider vertical class="custom-divider mx-2 mx-sm-3"></v-divider>

    <!-- Grupo 3: Layouts, Filtros, Busca & Ajuda -->
    <div class="controls-group">
      <!-- Menu de Layouts Automáticos -->
      <v-menu location="bottom end">
        <template #activator="{ props: menuProps }">
          <v-tooltip text="Organizar nós automaticamente" location="bottom">
            <template #activator="{ props: tooltipProps }">
              <v-btn
                v-bind="{ ...menuProps, ...tooltipProps }"
                class="control-btn btn-layout text-capitalize"
                aria-label="Auto layout"
              >
                <v-icon size="20" class="mr-xl-1">mdi-graph-outline</v-icon>
                <span class="d-none d-xl-inline text-caption">Layout</span>
                <v-icon end size="16" class="d-none d-xl-inline">mdi-chevron-down</v-icon>
              </v-btn>
            </template>
          </v-tooltip>
        </template>
        <v-list density="compact" class="rounded-lg elevation-6 dropdown-menu">
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
          <v-tooltip text="Filtrar tipos de equipamentos exibidos" location="bottom">
            <template #activator="{ props: tooltipProps }">
              <v-btn
                v-bind="{ ...menuProps, ...tooltipProps }"
                class="control-btn btn-filter text-capitalize"
                aria-label="Filtrar dispositivos"
              >
                <v-icon size="20" class="mr-xl-1">mdi-filter-variant</v-icon>
                <span class="d-none d-xl-inline text-caption">Filtros</span>
                <v-badge v-if="activeTypeFilter" color="primary" dot class="ml-1"></v-badge>
              </v-btn>
            </template>
          </v-tooltip>
        </template>
        <v-list density="compact" class="rounded-lg elevation-6 dropdown-menu">
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

      <!-- Botão de Busca Rápida Integrado (Visível em telas onde a barra de busca expandida é oculta) -->
      <v-tooltip text="Localizar equipamento no mapa" location="bottom">
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            class="control-btn btn-search d-inline-flex d-lg-none"
            aria-label="Buscar dispositivo no mapa"
            @click="$emit('open-search')"
          >
            <v-icon size="20">mdi-magnify</v-icon>
          </v-btn>
        </template>
      </v-tooltip>

      <!-- Legenda Rápida e Guia Touch -->
      <v-menu location="bottom end">
        <template #activator="{ props: menuProps }">
          <v-tooltip text="Guia de gestos e legenda de links" location="bottom">
            <template #activator="{ props: tooltipProps }">
              <v-btn
                v-bind="{ ...menuProps, ...tooltipProps }"
                class="control-btn btn-help"
                aria-label="Guia de gestos e legenda"
              >
                <v-icon size="20">mdi-help-circle-outline</v-icon>
              </v-btn>
            </template>
          </v-tooltip>
        </template>
        <v-card class="pa-4 rounded-xl elevation-8 guide-card" max-width="340">
          <div class="font-weight-bold text-subtitle-2 mb-2 d-flex align-center text-primary-light">
            <v-icon color="#38bdf8" class="mr-2" size="20">mdi-gesture-tap</v-icon>
            <span>Gestos & Interação</span>
          </div>
          <div class="text-caption text-medium-emphasis mb-3">
            <div class="mb-1 d-flex align-center">
              <v-icon size="16" class="mr-2 text-cyan-accent-2">mdi-hand-back-right</v-icon>
              <span><strong>1 dedo / Arraste:</strong> Mover o mapa ou arrastar nós</span>
            </div>
            <div class="mb-1 d-flex align-center">
              <v-icon size="16" class="mr-2 text-cyan-accent-2">mdi-gesture-pinch</v-icon>
              <span><strong>2 dedos (Pinch):</strong> Zoom in / Zoom out suave</span>
            </div>
            <div class="d-flex align-center">
              <v-icon size="16" class="mr-2 text-cyan-accent-2">mdi-cursor-default-click</v-icon>
              <span><strong>Toque no nó / link:</strong> Abre detalhes ou conecta</span>
            </div>
          </div>

          <v-divider class="my-2"></v-divider>

          <div class="font-weight-bold text-subtitle-2 mb-2 text-white">Legenda de Conexões</div>
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
        </v-card>
      </v-menu>
    </div>

    <!-- Indicador de Modo Somente Leitura se aplicável -->
    <v-chip
      v-if="canWrite === false"
      size="small"
      color="info"
      variant="tonal"
      class="font-weight-medium ml-1"
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
  (e: 'open-search'): void
}>()
</script>

<style scoped>
.topology-controls-bar {
  background: rgba(15, 23, 42, 0.9);
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
  max-width: calc(100vw - 32px);
  overflow-x: auto;
  scrollbar-width: none;
  display: flex;
  align-items: center;
  gap: 8px;
}
.topology-controls-bar::-webkit-scrollbar {
  display: none;
}

.controls-group {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: nowrap;
}

.custom-divider {
  border-color: rgba(255, 255, 255, 0.14) !important;
  height: 22px;
}

/* Botões base */
.control-btn {
  min-height: 38px;
  min-width: 38px;
  height: 38px;
  padding: 0 12px;
  border-radius: 9px;
  margin: 0 1px;
  transition: all 0.2s ease-in-out;
  flex-shrink: 0;
}

/* Cores específicas de alto contraste e vibração */
.btn-primary {
  background: #2563eb !important;
  color: #ffffff !important;
  box-shadow: 0 2px 8px rgba(37, 99, 235, 0.4);
}
.btn-primary:hover {
  background: #1d4ed8 !important;
}

.btn-switch {
  background: rgba(99, 102, 241, 0.18) !important;
  border: 1px solid rgba(129, 140, 248, 0.45) !important;
  color: #c7d2fe !important;
}
.btn-switch:hover {
  background: rgba(99, 102, 241, 0.3) !important;
  color: #ffffff !important;
}

.btn-recalculate {
  background: rgba(20, 184, 166, 0.18) !important;
  border: 1px solid rgba(45, 212, 191, 0.45) !important;
  color: #99f6e4 !important;
}
.btn-recalculate:hover {
  background: rgba(20, 184, 166, 0.3) !important;
  color: #ffffff !important;
}

.btn-cable {
  background: rgba(245, 158, 11, 0.16) !important;
  border: 1px solid rgba(251, 191, 36, 0.4) !important;
  color: #fef08a !important;
}
.btn-cable:hover {
  background: rgba(245, 158, 11, 0.28) !important;
  color: #ffffff !important;
}
.btn-cable-active {
  background: #f59e0b !important;
  color: #0f172a !important;
  box-shadow: 0 0 16px rgba(245, 158, 11, 0.7) !important;
  border-color: #fde047 !important;
}

/* Grupo de Zoom */
.zoom-controls-group {
  display: flex;
  align-items: center;
  background: rgba(2, 6, 23, 0.7);
  border: 1px solid rgba(255, 255, 255, 0.14);
  padding: 2px 6px;
  flex-shrink: 0;
  gap: 4px;
}
.zoom-btn {
  min-height: 32px;
  min-width: 32px;
  height: 32px;
  width: 32px;
  margin: 0 1px;
  color: #f1f5f9 !important;
  border-radius: 6px;
}
.zoom-btn:hover {
  background: rgba(255, 255, 255, 0.14) !important;
}
.btn-fit-screen {
  color: #38bdf8 !important;
  background: rgba(56, 189, 248, 0.15) !important;
  border: 1px solid rgba(56, 189, 248, 0.3) !important;
  border-radius: 6px;
}
.btn-fit-screen:hover {
  background: rgba(56, 189, 248, 0.3) !important;
  color: #ffffff !important;
}

/* Auto-Layout, Filtros, Busca & Ajuda */
.btn-layout {
  background: rgba(168, 85, 247, 0.18) !important;
  border: 1px solid rgba(192, 132, 252, 0.4) !important;
  color: #e9d5ff !important;
}
.btn-layout:hover {
  background: rgba(168, 85, 247, 0.3) !important;
  color: #ffffff !important;
}

.btn-filter {
  background: rgba(236, 72, 153, 0.18) !important;
  border: 1px solid rgba(244, 114, 182, 0.4) !important;
  color: #fce7f3 !important;
}
.btn-filter:hover {
  background: rgba(236, 72, 153, 0.3) !important;
  color: #ffffff !important;
}

.btn-search {
  background: rgba(56, 189, 248, 0.18) !important;
  border: 1px solid rgba(56, 189, 248, 0.45) !important;
  color: #bae6fd !important;
}
.btn-search:hover {
  background: rgba(56, 189, 248, 0.3) !important;
  color: #ffffff !important;
}

.btn-help {
  background: rgba(255, 255, 255, 0.1) !important;
  border: 1px solid rgba(255, 255, 255, 0.25) !important;
  color: #f8fafc !important;
}
.btn-help:hover {
  background: rgba(255, 255, 255, 0.2) !important;
  color: #38bdf8 !important;
}

.zoom-text {
  min-width: 42px;
  text-align: center;
  color: #f8fafc;
}

.dropdown-menu {
  background: rgba(15, 23, 42, 0.96) !important;
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.15);
}

.guide-card {
  background: rgba(15, 23, 42, 0.96) !important;
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.15);
}

.legend-line {
  width: 24px;
  height: 4px;
  border-radius: 2px;
}

@media (max-width: 600px) {
  .topology-controls-bar {
    padding: 8px 12px !important;
    gap: 8px !important;
  }
  .controls-group {
    gap: 5px;
  }
  .control-btn {
    min-height: 40px;
    min-width: 40px;
    height: 40px;
    padding: 0 8px;
    margin: 0 1px;
  }
  .zoom-btn {
    min-height: 34px;
    min-width: 34px;
    height: 34px;
    width: 34px;
  }
}
</style>

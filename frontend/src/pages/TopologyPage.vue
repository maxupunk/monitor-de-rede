<template>
  <div class="topology-view-wrapper d-flex flex-column fill-height w-100 position-relative">
    <!-- Container Principal do Mapa Gráfico -->
    <div class="overflow-hidden position-relative flex-grow-1 topology-map-container w-100 h-100">
      <!-- Barra Flutuante de Controles Superior -->
      <div class="floating-controls-wrapper">
        <TopologyControls
          :zoom-level="zoom"
          :is-connect-mode="isConnectMode"
          :active-type-filter="selectedTypeFilter"
          :recalculating="topologyStore.recalculating"
          :can-write="authStore.canWrite"
          @zoom-in="zoomIn"
          @zoom-out="zoomOut"
          @zoom-reset="zoomReset"
          @fit-screen="fitToScreen"
          @toggle-connect-mode="toggleConnectMode"
          @apply-layout="applyLayout"
          @filter-type="applyTypeFilter"
          @add-link="openLinkDialog()"
          @add-switch="unmanagedSwitchDialog = true"
          @recalculate="recalculateTopology"
        />
      </div>

      <!-- Barra Flutuante de Busca Rápida de Dispositivo -->
      <div class="floating-search-wrapper hidden-xs">
        <v-autocomplete
          v-model="highlightedDeviceId"
          :items="topologyStore.nodes"
          item-title="name"
          item-value="id"
          label="Localizar equipamento no mapa..."
          prepend-inner-icon="mdi-magnify"
          variant="solo-filled"
          density="compact"
          hide-details
          clearable
          class="search-autocomplete elevation-3"
          @update:model-value="onDeviceSearchSelected"
        >
          <template #item="{ props: itemProps, item }">
            <v-list-item v-bind="itemProps" :subtitle="item.ipAddress || item.type">
              <template #prepend>
                <v-icon :color="getNodeColor(item.status)" size="20">
                  {{ getNodeIcon(item.type) }}
                </v-icon>
              </template>
            </v-list-item>
          </template>
        </v-autocomplete>
      </div>

      <!-- Alerta Informativo do Modo de Conexão com Mouse Ativo -->
      <transition name="slide-y-transition">
        <div v-if="isConnectMode" class="connect-mode-banner elevation-6 px-4 py-2 rounded-pill">
          <v-icon color="white" class="mr-2 cable-pulse">mdi-vector-polyline</v-icon>
          <span class="text-caption font-weight-bold text-white">
            <span v-if="!connectSourceId">
              Clique no <strong>equipamento de ORIGEM</strong> para iniciar o enlace
            </span>
            <span v-else>
              Clique no <strong>equipamento de DESTINO</strong> para finalizar (ESC para cancelar)
            </span>
          </span>
          <v-btn
            size="x-small"
            variant="text"
            color="white"
            icon="mdi-close"
            class="ml-2"
            @click="cancelConnectMode"
          ></v-btn>
        </div>
      </transition>

      <!-- Viewport / Canvas Interativo (Pan & Zoom) -->
      <div
        ref="canvasViewport"
        class="topology-viewport"
        :class="{
          'cursor-grab': !isPanning && !isConnectMode,
          'cursor-grabbing': isPanning,
          'cursor-crosshair': isConnectMode,
        }"
        @mousedown="onViewportMouseDown"
        @mousemove="onViewportMouseMove"
        @mouseup="onViewportMouseUp"
        @wheel.prevent="onViewportWheel"
      >
        <!-- Camada Transformada por Pan e Zoom -->
        <div
          class="topology-world"
          :style="{
            transform: `translate(${panX}px, ${panY}px) scale(${zoom})`,
            transformOrigin: '0 0',
          }"
        >
          <!-- Grid de Pontos de Fundo -->
          <svg width="4000" height="4000" class="topology-grid-layer">
            <defs>
              <pattern id="dotGrid" width="40" height="40" patternUnits="userSpaceOnUse">
                <circle cx="20" cy="20" r="1.5" fill="rgba(var(--v-theme-on-surface), 0.12)" />
              </pattern>
            </defs>
            <rect width="100%" height="100%" fill="url(#dotGrid)" />
          </svg>

          <!-- SVG Canvas para Arestas / Cabos -->
          <svg width="4000" height="4000" class="topology-edges-layer">
            <!-- Linha Elástica Provisória de Conexão do Mouse -->
            <g v-if="isConnectMode && connectSourceId && mousePosWorld">
              <line
                :x1="getNodeCenter(connectSourceId).x"
                :y1="getNodeCenter(connectSourceId).y"
                :x2="mousePosWorld.x"
                :y2="mousePosWorld.y"
                stroke="#2196F3"
                stroke-width="3"
                stroke-dasharray="6,6"
                class="elastic-link-line"
              />
            </g>

            <!-- Arestas Existentes -->
            <g
              v-for="edge in visibleEdges"
              :key="edge.id"
              class="edge-group cursor-pointer"
              @click.stop="onEdgeClick(edge)"
            >
              <!-- Linha Invisível mais Grossa para Facilitar Clique / Hover -->
              <line
                :x1="edge.x1"
                :y1="edge.y1"
                :x2="edge.x2"
                :y2="edge.y2"
                stroke="transparent"
                stroke-width="18"
              />

              <!-- Linha Principal da Conexão -->
              <line
                :x1="edge.x1"
                :y1="edge.y1"
                :x2="edge.x2"
                :y2="edge.y2"
                :stroke="getLinkColor(edge.linkType)"
                :stroke-width="edge.isHighlighted ? 4 : 2.5"
                :stroke-dasharray="getLinkDashArray(edge.linkType)"
                class="edge-line"
                :class="{ 'edge-line-active': edge.status === 'up' && edge.linkType !== 'parent' }"
              />

              <!-- Badges de Portas / Interfaces nas Extremidades -->
              <!-- Porta de Saída (Origem) -->
              <g
                v-if="edge.sourceInterfaceName"
                :transform="`translate(${calculatePortBadgePos(edge.x1, edge.y1, edge.x2, edge.y2, 0.2).x}, ${calculatePortBadgePos(edge.x1, edge.y1, edge.x2, edge.y2, 0.2).y})`"
              >
                <rect x="-28" y="-9" width="56" height="18" rx="9" class="port-pill-bg" />
                <text x="0" y="3.5" text-anchor="middle" class="port-pill-text">
                  {{ truncate(edge.sourceInterfaceName, 7) }}
                </text>
              </g>

              <!-- Badge de Consumo / Tráfego no Meio do Link (Atualizado em Tempo Real) -->
              <g
                v-if="edge.trafficLabel || edge.sourceInterfaceName || edge.targetInterfaceName"
                :transform="`translate(${calculatePortBadgePos(edge.x1, edge.y1, edge.x2, edge.y2, 0.5).x}, ${calculatePortBadgePos(edge.x1, edge.y1, edge.x2, edge.y2, 0.5).y})`"
                class="traffic-pill-group"
              >
                <rect x="-36" y="-10" width="72" height="20" rx="10" class="traffic-pill-bg" />
                <text x="0" y="4" text-anchor="middle" class="traffic-pill-text">
                  {{ edge.trafficLabel || '0 bps' }}
                </text>
              </g>

              <!-- Porta de Chegada (Destino) -->
              <g
                v-if="edge.targetInterfaceName"
                :transform="`translate(${calculatePortBadgePos(edge.x1, edge.y1, edge.x2, edge.y2, 0.8).x}, ${calculatePortBadgePos(edge.x1, edge.y1, edge.x2, edge.y2, 0.8).y})`"
              >
                <rect x="-28" y="-9" width="56" height="18" rx="9" class="port-pill-bg" />
                <text x="0" y="3.5" text-anchor="middle" class="port-pill-text">
                  {{ truncate(edge.targetInterfaceName, 7) }}
                </text>
              </g>
            </g>
          </svg>

          <!-- Nós / Equipamentos Renderizados em HTML com Rich Styling -->
          <div
            v-for="node in visibleNodes"
            :key="node.id"
            class="topology-node-container"
            :class="{
              'node-dragging': draggingNodeId === node.id,
              'node-connect-source': connectSourceId === node.id,
              'node-highlighted': highlightedDeviceId === node.id,
            }"
            :style="{ left: `${node.x}px`, top: `${node.y}px` }"
            @mousedown.stop="onNodeMouseDown($event, node)"
            @click.stop="onNodeClick(node)"
          >
            <!-- Halo de Conexão no Modo Cable Tool -->
            <div v-if="isConnectMode" class="connect-port-handles">
              <span class="port-handle handle-top"></span>
              <span class="port-handle handle-right"></span>
              <span class="port-handle handle-bottom"></span>
              <span class="port-handle handle-left"></span>
            </div>

            <!-- Avatar do Dispositivo com Anel Luminoso de Status -->
            <div class="node-avatar-wrapper position-relative">
              <div class="status-pulse-ring" :class="`status-ring-${node.status}`"></div>
              <v-avatar
                :color="getNodeColor(node.status)"
                size="48"
                class="elevation-4 node-avatar"
              >
                <v-icon color="white" size="26">
                  {{ getNodeIcon(node.type) }}
                </v-icon>
              </v-avatar>

              <!-- Badge de Interfaces Ativas / Hub -->
              <div
                v-if="node.interfaceCount > 0"
                class="node-port-count-badge text-caption font-weight-bold elevation-2"
                :title="`${node.interfaceCount} interfaces registradas`"
              >
                {{ node.interfaceCount }}
              </div>
            </div>

            <!-- Cartão / Label de Informação do Nó -->
            <div class="node-label-card pa-1 mt-1 rounded-lg elevation-2 text-center bg-surface">
              <div class="node-title font-weight-bold text-caption text-truncate px-1">
                {{ node.name }}
              </div>
              <div
                v-if="node.ipAddress"
                class="node-subtitle text-caption text-medium-emphasis px-1 font-mono text-truncate"
              >
                {{ node.ipAddress }}
              </div>
              <div v-else class="node-subtitle text-caption text-primary px-1 text-truncate">
                {{ getNodeTypeLabel(node.type) }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Estado Vazio / Sem Nós -->
      <div
        v-if="!topologyStore.loading && topologyStore.nodes.length === 0"
        class="d-flex align-center justify-center fill-height position-absolute top-0 left-0 w-100 h-100 empty-state-layer text-grey"
      >
        <div class="text-center pa-6">
          <v-avatar color="primary" variant="tonal" size="80" class="mb-4">
            <v-icon size="48">mdi-sitemap</v-icon>
          </v-avatar>
          <div class="text-h6 font-weight-bold mb-1">Nenhum equipamento na topologia</div>
          <p class="text-body-2 text-medium-emphasis mb-4">
            Cadastre dispositivos ou clique em "Recalcular Topologia" para construir o mapa.
          </p>
          <div class="d-flex justify-center gap-2">
            <v-btn color="primary" prepend-icon="mdi-calculator" @click="recalculateTopology">
              Recalcular Topologia
            </v-btn>
            <v-btn
              color="indigo"
              variant="outlined"
              prepend-icon="mdi-hub"
              @click="unmanagedSwitchDialog = true"
            >
              Criar Switch
            </v-btn>
          </div>
        </div>
      </div>

      <!-- Loading Overlay -->
      <v-overlay :model-value="topologyStore.loading" contained class="align-center justify-center">
        <div class="text-center bg-surface pa-6 rounded-xl elevation-12">
          <v-progress-circular
            indeterminate
            color="primary"
            size="48"
            class="mb-3"
          ></v-progress-circular>
          <div class="text-subtitle-1 font-weight-bold">Carregando Mapa de Topologia...</div>
        </div>
      </v-overlay>
    </div>

    <!-- Drawer de Detalhes do Nó Selecionado -->
    <v-dialog
      v-model="nodeDrawer"
      :max-width="$vuetify.display.xs ? undefined : 460"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card v-if="selectedNode" class="rounded-xl pa-2 overflow-hidden elevation-12">
        <v-card-item class="pa-4 bg-surface-variant-subtle rounded-lg">
          <div class="d-flex align-center">
            <v-avatar :color="getNodeColor(selectedNode.status)" size="48" class="mr-3 elevation-2">
              <v-icon color="white" size="26">{{ getNodeIcon(selectedNode.type) }}</v-icon>
            </v-avatar>
            <div class="flex-grow-1 overflow-hidden">
              <div class="text-h6 font-weight-bold text-truncate">{{ selectedNode.name }}</div>
              <div class="text-caption text-medium-emphasis d-flex align-center">
                <span class="mr-2">IP: {{ selectedNode.ipAddress || 'Não configurado' }}</span>
                <v-chip
                  size="x-small"
                  :color="getNodeColor(selectedNode.status)"
                  variant="tonal"
                  class="text-uppercase font-weight-bold"
                >
                  {{ selectedNode.status }}
                </v-chip>
              </div>
            </div>
            <v-btn
              icon="mdi-close"
              variant="text"
              density="comfortable"
              size="small"
              @click="nodeDrawer = false"
            ></v-btn>
          </div>
        </v-card-item>

        <v-card-text class="pa-4">
          <v-list density="compact" class="pa-0">
            <v-list-item
              title="Tipo de Dispositivo"
              :subtitle="getNodeTypeLabel(selectedNode.type)"
            >
              <template #prepend>
                <v-icon color="primary">mdi-shape-outline</v-icon>
              </template>
            </v-list-item>
            <v-list-item
              v-if="selectedNode.vendor"
              title="Fabricante"
              :subtitle="selectedNode.vendor"
            >
              <template #prepend>
                <v-icon color="primary">mdi-domain</v-icon>
              </template>
            </v-list-item>
            <v-list-item v-if="selectedNode.model" title="Modelo" :subtitle="selectedNode.model">
              <template #prepend>
                <v-icon color="primary">mdi-tag-outline</v-icon>
              </template>
            </v-list-item>
            <v-list-item
              title="Interfaces Conhecidas"
              :subtitle="`${selectedNode.interfaceCount} interfaces mapeadas`"
            >
              <template #prepend>
                <v-icon color="primary">mdi-ethernet</v-icon>
              </template>
            </v-list-item>
          </v-list>

          <v-divider class="my-3"></v-divider>

          <!-- Ações Rápidas -->
          <div class="d-flex flex-column gap-2">
            <v-btn
              color="primary"
              variant="tonal"
              prepend-icon="mdi-vector-polyline-plus"
              block
              @click="openLinkDialogFromNode(selectedNode.id)"
            >
              Conectar a Outro Dispositivo
            </v-btn>
            <v-btn
              v-if="selectedNode.type !== 'unmanaged_switch' && selectedNode.type !== 'hub'"
              color="secondary"
              variant="outlined"
              prepend-icon="mdi-open-in-new"
              block
              :to="`/devices/${selectedNode.id}`"
            >
              Abrir Ficha do Dispositivo
            </v-btn>
            <v-btn
              color="error"
              variant="tonal"
              prepend-icon="mdi-trash-can-outline"
              block
              @click="confirmDeleteDevice(selectedNode)"
            >
              {{
                selectedNode.type === 'unmanaged_switch' || selectedNode.type === 'hub'
                  ? 'Excluir Switch'
                  : 'Excluir Dispositivo'
              }}
            </v-btn>
          </div>
        </v-card-text>
      </v-card>
    </v-dialog>

    <!-- Diálogo de Confirmação de Exclusão de Dispositivo/Switch -->
    <v-dialog v-model="deleteDeviceDialog" max-width="420">
      <v-card v-if="deviceToDelete" class="rounded-xl pa-4 elevation-12">
        <v-card-title class="font-weight-bold d-flex align-center pa-0 mb-2">
          <v-icon color="error" class="mr-2">mdi-alert-circle</v-icon>
          <span>
            Excluir
            {{
              deviceToDelete.type === 'unmanaged_switch' || deviceToDelete.type === 'hub'
                ? 'Switch'
                : 'Dispositivo'
            }}
          </span>
        </v-card-title>
        <v-card-text class="pa-0 py-2">
          Tem certeza de que deseja remover <strong>{{ deviceToDelete.name }}</strong> da topologia
          e do sistema? Todas as conexões e portas associadas também serão excluídas.
        </v-card-text>
        <v-card-actions class="pa-0 pt-3 justify-end">
          <v-btn variant="text" :disabled="deletingDevice" @click="deleteDeviceDialog = false">
            Cancelar
          </v-btn>
          <v-btn
            color="error"
            variant="elevated"
            :loading="deletingDevice"
            @click="executeDeleteDevice"
          >
            Excluir
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Diálogo de Detalhes / Edição / Exclusão de Conexão -->
    <v-dialog
      v-model="edgeDialog"
      :max-width="$vuetify.display.xs ? undefined : 460"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card v-if="selectedEdge" class="rounded-xl pa-4 elevation-12">
        <v-card-title class="font-weight-bold d-flex align-center justify-space-between pa-0 mb-3">
          <div class="d-flex align-center">
            <v-icon color="primary" class="mr-2">mdi-transit-connection-variant</v-icon>
            <span>Detalhes do Enlace</span>
          </div>
          <v-btn
            icon="mdi-close"
            variant="text"
            density="compact"
            size="small"
            @click="edgeDialog = false"
          ></v-btn>
        </v-card-title>
        <v-divider class="mb-3"></v-divider>
        <v-card-text class="pa-0">
          <div class="pa-3 rounded-lg bg-surface-variant-subtle mb-3">
            <div class="d-flex align-center justify-space-between mb-1">
              <span class="text-caption text-medium-emphasis">Dispositivo A (Origem):</span>
              <v-chip size="x-small" color="primary" variant="tonal">Origem</v-chip>
            </div>
            <div class="font-weight-bold">{{ selectedEdge.sourceDeviceName || 'Origem' }}</div>
            <div class="text-caption text-primary font-weight-medium mt-1">
              <v-icon size="14" class="mr-1">mdi-ethernet</v-icon>
              Porta de Saída: {{ selectedEdge.sourceInterfaceName || 'Automática / Não definida' }}
            </div>

            <v-divider class="my-2"></v-divider>

            <div class="d-flex align-center justify-space-between mb-1">
              <span class="text-caption text-medium-emphasis">Dispositivo B (Destino):</span>
              <v-chip size="x-small" color="success" variant="tonal">Destino</v-chip>
            </div>
            <div class="font-weight-bold">{{ selectedEdge.targetDeviceName || 'Destino' }}</div>
            <div class="text-caption text-success font-weight-medium mt-1">
              <v-icon size="14" class="mr-1">mdi-ethernet</v-icon>
              Porta de Chegada:
              {{ selectedEdge.targetInterfaceName || 'Automática / Não definida' }}
            </div>
          </div>

          <v-list density="compact" class="pa-0">
            <v-list-item
              title="Tecnologia do Link"
              :subtitle="getLinkTypeLabel(selectedEdge.linkType)"
            >
              <template #prepend>
                <v-icon :color="getLinkColor(selectedEdge.linkType)">
                  {{ getLinkTypeIcon(selectedEdge.linkType) }}
                </v-icon>
              </template>
            </v-list-item>
            <v-list-item
              v-if="selectedEdge.trafficLabel"
              title="Consumo de Tráfego em Tempo Real"
              :subtitle="selectedEdge.trafficLabel"
            >
              <template #prepend>
                <v-icon color="cyan">mdi-speedometer</v-icon>
              </template>
            </v-list-item>
            <v-list-item title="Método de Descoberta" :subtitle="selectedEdge.discoveryMethod">
              <template #prepend>
                <v-icon color="grey">mdi-information-outline</v-icon>
              </template>
            </v-list-item>
          </v-list>
        </v-card-text>
        <v-card-actions class="pa-0 mt-4 d-flex align-center justify-space-between flex-wrap gap-2">
          <v-btn
            v-if="selectedEdge.id > 0"
            color="error"
            variant="tonal"
            prepend-icon="mdi-trash-can-outline"
            :loading="deletingEdge"
            @click="confirmDeleteEdge(selectedEdge.id)"
          >
            Remover Link
          </v-btn>
          <span v-else class="text-caption text-grey">Enlace automático</span>

          <div class="d-flex align-center gap-2">
            <v-btn
              color="primary"
              variant="tonal"
              prepend-icon="mdi-pencil-outline"
              @click="editLinkFromEdge(selectedEdge)"
            >
              {{ selectedEdge.id > 0 ? 'Editar Conexão' : 'Personalizar Enlace' }}
            </v-btn>
            <v-btn variant="text" @click="edgeDialog = false">Fechar</v-btn>
          </div>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Diálogo Inteligente de Criação e Edição de Link -->
    <TopologyLinkDialog
      v-model="linkDialog"
      :editing-link-id="linkDialogEditingId"
      :initial-source-device-id="linkDialogSourceId"
      :initial-target-device-id="linkDialogTargetId"
      :initial-source-interface-id="linkDialogSourceInterfaceId"
      :initial-target-interface-id="linkDialogTargetInterfaceId"
      :initial-link-type="linkDialogLinkType"
      @saved="onLinkSaved"
    />

    <!-- Diálogo de Criação de Switch -->
    <UnmanagedSwitchDialog v-model="unmanagedSwitchDialog" @created="onSwitchCreated" />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import { useTopologyStore, type TopologyNode, type TopologyEdge } from '@/stores/topology'
import { useAuthStore } from '@/stores/auth'
import TopologyControls from '@/components/topology/TopologyControls.vue'
import TopologyLinkDialog from '@/components/topology/TopologyLinkDialog.vue'
import UnmanagedSwitchDialog from '@/components/topology/UnmanagedSwitchDialog.vue'

interface RenderedNode extends TopologyNode {
  x: number
  y: number
}

interface RenderedEdge extends TopologyEdge {
  x1: number
  y1: number
  x2: number
  y2: number
  isHighlighted: boolean
}

const STORAGE_POS_KEY = 'netmonitor_topology_positions_v1'

const topologyStore = useTopologyStore()
const authStore = useAuthStore()
const canvasViewport = ref<HTMLElement | null>(null)

// Estados de Visualização (Pan & Zoom)
const zoom = ref(1)
const panX = ref(60)
const panY = ref(60)
const isPanning = ref(false)
const panStart = reactive({ x: 0, y: 0, panX: 0, panY: 0, hasMoved: false })

// Estados de Arraste de Nó
const nodePositions = reactive<Map<number, { x: number; y: number }>>(new Map())
const draggingNodeId = ref<number | null>(null)
const dragStart = reactive({ mouseX: 0, mouseY: 0, nodeX: 0, nodeY: 0, hasMoved: false })

// Estados de Conexão Gráfica com o Mouse (Cable Tool)
const isConnectMode = ref(false)
const connectSourceId = ref<number | null>(null)
const mousePosWorld = ref<{ x: number; y: number } | null>(null)

// Estados de Filtro e Destaque
const selectedTypeFilter = ref<string | null>(null)
const highlightedDeviceId = ref<number | null>(null)

// Diálogos e Seleções
const selectedNode = ref<TopologyNode | null>(null)
const nodeDrawer = ref(false)
const selectedEdge = ref<TopologyEdge | null>(null)
const edgeDialog = ref(false)
const deletingEdge = ref(false)

const deleteDeviceDialog = ref(false)
const deviceToDelete = ref<TopologyNode | null>(null)
const deletingDevice = ref(false)

const linkDialog = ref(false)
const linkDialogEditingId = ref<number | null>(null)
const linkDialogSourceId = ref<number | null>(null)
const linkDialogTargetId = ref<number | null>(null)
const linkDialogSourceInterfaceId = ref<number | null>(null)
const linkDialogTargetInterfaceId = ref<number | null>(null)
const linkDialogLinkType = ref<string | null>(null)

const unmanagedSwitchDialog = ref(false)

let pollTimer: ReturnType<typeof setInterval> | null = null

onMounted(async () => {
  loadPositionsFromStorage()
  await topologyStore.fetchTopology()
  ensureInitialNodeLayout()
  window.addEventListener('keydown', onKeyDown)

  // Atualização contínua de tráfego e métricas em tempo real (a cada 5 segundos)
  pollTimer = setInterval(() => {
    topologyStore.fetchTopology(null, false)
  }, 5000)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown)
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
})

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isConnectMode.value) {
    cancelConnectMode()
  }
}

// ----------------------------------------------------
// LAYOUT E POSICIONAMENTO DE NÓS
// ----------------------------------------------------

function loadPositionsFromStorage() {
  try {
    const raw = localStorage.getItem(STORAGE_POS_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Record<string, { x: number; y: number }>
      for (const [idStr, pos] of Object.entries(parsed)) {
        nodePositions.set(Number(idStr), pos)
      }
    }
  } catch {
    // Ignora erros de parsing
  }
}

function savePositionsToStorage() {
  try {
    const obj: Record<string, { x: number; y: number }> = {}
    for (const [id, pos] of nodePositions.entries()) {
      obj[id] = pos
    }
    localStorage.setItem(STORAGE_POS_KEY, JSON.stringify(obj))
  } catch {
    // Ignora
  }
}

function ensureInitialNodeLayout() {
  const nodes = topologyStore.nodes
  let hasMissing = false

  for (const node of nodes) {
    if (!nodePositions.has(node.id)) {
      hasMissing = true
      break
    }
  }

  if (hasMissing || nodePositions.size === 0) {
    applyLayout('hierarchical')
  }
}

const visibleNodes = computed<RenderedNode[]>(() => {
  const filter = selectedTypeFilter.value
  const list = filter
    ? topologyStore.nodes.filter((n) => n.type?.toLowerCase() === filter.toLowerCase())
    : topologyStore.nodes

  return list.map((node) => {
    const pos = nodePositions.get(node.id) || { x: 300, y: 300 }
    return {
      ...node,
      x: pos.x,
      y: pos.y,
    }
  })
})

const visibleEdges = computed<RenderedEdge[]>(() => {
  const nodesMap = new Map<number, RenderedNode>(visibleNodes.value.map((n) => [n.id, n]))
  const result: RenderedEdge[] = []

  for (const edge of topologyStore.edges) {
    const srcId = edge.sourceDeviceId ?? edge.source
    const tgtId = edge.targetDeviceId ?? edge.target
    const src = nodesMap.get(srcId)
    const tgt = nodesMap.get(tgtId)

    if (src && tgt) {
      const isHigh = highlightedDeviceId.value === src.id || highlightedDeviceId.value === tgt.id
      result.push({
        ...edge,
        x1: src.x + 30,
        y1: src.y + 30,
        x2: tgt.x + 30,
        y2: tgt.y + 30,
        isHighlighted: isHigh,
      })
    }
  }
  return result
})

function getNodeCenter(nodeId: number): { x: number; y: number } {
  const pos = nodePositions.get(nodeId) || { x: 0, y: 0 }
  return { x: pos.x + 30, y: pos.y + 30 }
}

function calculatePortBadgePos(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  fraction: number
): { x: number; y: number } {
  return {
    x: x1 + (x2 - x1) * fraction,
    y: y1 + (y2 - y1) * fraction,
  }
}

// ----------------------------------------------------
// ALGORITMOS DE AUTO-LAYOUT
// ----------------------------------------------------

function applyLayout(type: 'hierarchical' | 'force' | 'radial' | 'grid') {
  const nodes = topologyStore.nodes
  if (nodes.length === 0) return

  if (type === 'hierarchical') {
    layoutHierarchical(nodes)
  } else if (type === 'radial') {
    layoutRadial(nodes)
  } else if (type === 'grid') {
    layoutGrid(nodes)
  } else if (type === 'force') {
    layoutForce(nodes)
  }

  savePositionsToStorage()
}

function layoutHierarchical(nodes: TopologyNode[]) {
  // Separa por camadas: Roteadores / Gateways (0), Switches (1), Hosts / Demais (2)
  const tier0: TopologyNode[] = []
  const tier1: TopologyNode[] = []
  const tier2: TopologyNode[] = []

  for (const node of nodes) {
    const t = node.type?.toLowerCase()
    if (t === 'router' || t === 'firewall') {
      tier0.push(node)
    } else if (t === 'switch' || t === 'unmanaged_switch' || t === 'hub') {
      tier1.push(node)
    } else {
      tier2.push(node)
    }
  }

  const tiers = [tier0, tier1, tier2].filter((t) => t.length > 0)
  const startY = 120
  const layerSpacingY = 180

  tiers.forEach((tierNodes, layerIdx) => {
    const y = startY + layerIdx * layerSpacingY
    const count = tierNodes.length
    const spacingX = Math.max(140, Math.min(220, 1000 / (count + 1)))
    const totalWidth = count * spacingX
    const startX = Math.max(100, (1100 - totalWidth) / 2)

    tierNodes.forEach((node, idx) => {
      nodePositions.set(node.id, {
        x: startX + idx * spacingX,
        y: y + (idx % 2 === 1 ? 15 : 0),
      })
    })
  })
}

function layoutRadial(nodes: TopologyNode[]) {
  const total = nodes.length
  const centerX = 500
  const centerY = 360
  const radius = Math.min(360, Math.max(180, total * 30))

  nodes.forEach((node, idx) => {
    const angle = (idx / total) * 2 * Math.PI - Math.PI / 2
    nodePositions.set(node.id, {
      x: centerX + radius * Math.cos(angle) - 30,
      y: centerY + radius * Math.sin(angle) - 30,
    })
  })
}

function layoutGrid(nodes: TopologyNode[]) {
  const cols = Math.max(2, Math.ceil(Math.sqrt(nodes.length * 1.5)))
  const startX = 140
  const startY = 120
  const spacingX = 180
  const spacingY = 160

  nodes.forEach((node, idx) => {
    const r = Math.floor(idx / cols)
    const c = idx % cols
    nodePositions.set(node.id, {
      x: startX + c * spacingX,
      y: startY + r * spacingY,
    })
  })
}

function layoutForce(nodes: TopologyNode[]) {
  const width = 1000
  const height = 700
  const positions = new Map<number, { x: number; y: number }>()

  nodes.forEach((node, idx) => {
    const angle = (idx / nodes.length) * 2 * Math.PI
    positions.set(node.id, {
      x: width / 2 + Math.cos(angle) * 200,
      y: height / 2 + Math.sin(angle) * 200,
    })
  })

  // Simulação física simplificada de 35 passos
  for (let iter = 0; iter < 35; iter++) {
    // Repulsão entre todos os pares
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const p1 = positions.get(nodes[i].id)!
        const p2 = positions.get(nodes[j].id)!
        const dx = p2.x - p1.x
        const dy = p2.y - p1.y
        const distSq = dx * dx + dy * dy || 1
        const dist = Math.sqrt(distSq)
        if (dist < 220) {
          const force = (220 - dist) / dist
          p1.x -= dx * force * 0.05
          p1.y -= dy * force * 0.05
          p2.x += dx * force * 0.05
          p2.y += dy * force * 0.05
        }
      }
    }

    // Atração pelas conexões
    for (const edge of topologyStore.edges) {
      const srcId = edge.sourceDeviceId ?? edge.source
      const tgtId = edge.targetDeviceId ?? edge.target
      const p1 = positions.get(srcId)
      const p2 = positions.get(tgtId)
      if (p1 && p2) {
        const dx = p2.x - p1.x
        const dy = p2.y - p1.y
        const dist = Math.sqrt(dx * dx + dy * dy) || 1
        const force = (dist - 140) * 0.04
        p1.x += (dx / dist) * force
        p1.y += (dy / dist) * force
        p2.x -= (dx / dist) * force
        p2.y -= (dy / dist) * force
      }
    }
  }

  for (const [id, pos] of positions.entries()) {
    nodePositions.set(id, {
      x: Math.max(60, Math.min(width - 60, pos.x)),
      y: Math.max(60, Math.min(height - 60, pos.y)),
    })
  }
}

// ----------------------------------------------------
// INTERAÇÃO DE PAN & ZOOM
// ----------------------------------------------------

function onViewportMouseDown(e: MouseEvent) {
  if (isConnectMode.value) return
  if (e.button === 0) {
    isPanning.value = true
    panStart.x = e.clientX
    panStart.y = e.clientY
    panStart.panX = panX.value
    panStart.panY = panY.value
    panStart.hasMoved = false
  }
}

function onViewportMouseMove(e: MouseEvent) {
  // Movimento de Pan
  if (isPanning.value) {
    const dist = Math.hypot(e.clientX - panStart.x, e.clientY - panStart.y)
    if (dist > 4) {
      panStart.hasMoved = true
    }
    panX.value = panStart.panX + (e.clientX - panStart.x)
    panY.value = panStart.panY + (e.clientY - panStart.y)
    return
  }

  // Movimento de Arraste de Nó
  if (draggingNodeId.value !== null) {
    const totalDist = Math.hypot(e.clientX - dragStart.mouseX, e.clientY - dragStart.mouseY)
    if (totalDist > 4) {
      dragStart.hasMoved = true
    }
    const dx = (e.clientX - dragStart.mouseX) / zoom.value
    const dy = (e.clientY - dragStart.mouseY) / zoom.value
    const newX = Math.max(20, Math.min(3800, dragStart.nodeX + dx))
    const newY = Math.max(20, Math.min(3800, dragStart.nodeY + dy))
    nodePositions.set(draggingNodeId.value, { x: newX, y: newY })
    return
  }

  // Atualização de linha elástica no modo de conexão
  if (isConnectMode.value && connectSourceId.value && canvasViewport.value) {
    const rect = canvasViewport.value.getBoundingClientRect()
    mousePosWorld.value = {
      x: (e.clientX - rect.left - panX.value) / zoom.value,
      y: (e.clientY - rect.top - panY.value) / zoom.value,
    }
  }
}

function onViewportMouseUp() {
  if (isPanning.value) {
    isPanning.value = false
  }
  if (draggingNodeId.value !== null) {
    draggingNodeId.value = null
    savePositionsToStorage()
  }
}

function onViewportWheel(e: WheelEvent) {
  const delta = e.deltaY < 0 ? 0.08 : -0.08
  const newZoom = Math.min(2.5, Math.max(0.3, zoom.value + delta))

  if (canvasViewport.value) {
    const rect = canvasViewport.value.getBoundingClientRect()
    const mouseX = e.clientX - rect.left
    const mouseY = e.clientY - rect.top

    panX.value = mouseX - ((mouseX - panX.value) / zoom.value) * newZoom
    panY.value = mouseY - ((mouseY - panY.value) / zoom.value) * newZoom
  }

  zoom.value = newZoom
}

function zoomIn() {
  zoom.value = Math.min(2.5, zoom.value + 0.15)
}

function zoomOut() {
  zoom.value = Math.max(0.3, zoom.value - 0.15)
}

function zoomReset() {
  zoom.value = 1
  panX.value = 60
  panY.value = 60
}

function fitToScreen() {
  const nodes = visibleNodes.value
  if (nodes.length === 0 || !canvasViewport.value) return

  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity

  for (const n of nodes) {
    minX = Math.min(minX, n.x)
    minY = Math.min(minY, n.y)
    maxX = Math.max(maxX, n.x + 80)
    maxY = Math.max(maxY, n.y + 80)
  }

  const rect = canvasViewport.value.getBoundingClientRect()
  const contentWidth = maxX - minX + 120
  const contentHeight = maxY - minY + 120

  const scaleX = rect.width / contentWidth
  const scaleY = rect.height / contentHeight
  const targetZoom = Math.min(1.4, Math.max(0.4, Math.min(scaleX, scaleY)))

  zoom.value = targetZoom
  panX.value = (rect.width - (maxX + minX) * targetZoom) / 2
  panY.value = (rect.height - (maxY + minY) * targetZoom) / 2
}

// ----------------------------------------------------
// INTERAÇÃO COM NÓS (DRAG & DROP, CLIQUE, CONEXÃO)
// ----------------------------------------------------

function onNodeMouseDown(e: MouseEvent, node: RenderedNode) {
  if (isConnectMode.value) return
  if (e.button === 0) {
    draggingNodeId.value = node.id
    dragStart.mouseX = e.clientX
    dragStart.mouseY = e.clientY
    dragStart.nodeX = node.x
    dragStart.nodeY = node.y
    dragStart.hasMoved = false
  }
}

function onNodeClick(node: RenderedNode) {
  // Se o usuário realizou arraste do nó, não abre o diálogo/drawer
  if (dragStart.hasMoved) {
    dragStart.hasMoved = false
    return
  }

  if (isConnectMode.value) {
    if (!connectSourceId.value) {
      connectSourceId.value = node.id
    } else if (connectSourceId.value !== node.id) {
      // Conecta Origem ao Destino!
      const srcId = connectSourceId.value
      const tgtId = node.id
      cancelConnectMode()
      openLinkDialog(srcId, tgtId)
    }
    return
  }

  selectedNode.value = node
  nodeDrawer.value = true
}

function confirmDeleteDevice(node: TopologyNode) {
  deviceToDelete.value = node
  deleteDeviceDialog.value = true
}

async function executeDeleteDevice() {
  if (!deviceToDelete.value) return
  deletingDevice.value = true
  try {
    const id = deviceToDelete.value.id
    const success = await topologyStore.deleteDevice(id)
    if (success) {
      nodePositions.delete(id)
      savePositionsToStorage()
      deleteDeviceDialog.value = false
      nodeDrawer.value = false
    }
  } finally {
    deletingDevice.value = false
  }
}

function toggleConnectMode() {
  isConnectMode.value = !isConnectMode.value
  connectSourceId.value = null
  mousePosWorld.value = null
}

function cancelConnectMode() {
  isConnectMode.value = false
  connectSourceId.value = null
  mousePosWorld.value = null
}

function onDeviceSearchSelected(deviceId: number | null) {
  if (!deviceId) return
  const pos = nodePositions.get(deviceId)
  if (pos && canvasViewport.value) {
    const rect = canvasViewport.value.getBoundingClientRect()
    panX.value = rect.width / 2 - pos.x * zoom.value - 30 * zoom.value
    panY.value = rect.height / 2 - pos.y * zoom.value - 30 * zoom.value
  }
}

function applyTypeFilter(type: string | null) {
  selectedTypeFilter.value = type
}

// ----------------------------------------------------
// INTERAÇÃO COM ARESTAS (EDGES)
// ----------------------------------------------------

function onEdgeClick(edge: TopologyEdge) {
  // Se o usuário realizou arraste/pan do mapa, ignora o clique
  if (panStart.hasMoved || dragStart.hasMoved) {
    panStart.hasMoved = false
    dragStart.hasMoved = false
    return
  }
  selectedEdge.value = edge
  edgeDialog.value = true
}

async function confirmDeleteEdge(edgeId: number) {
  deletingEdge.value = true
  try {
    const success = await topologyStore.deleteLink(edgeId)
    if (success) {
      edgeDialog.value = false
    }
  } finally {
    deletingEdge.value = false
  }
}

// ----------------------------------------------------
// AÇÕES DE DIÁLOGOS
// ----------------------------------------------------

function openLinkDialog(sourceId?: number, targetId?: number) {
  linkDialogEditingId.value = null
  linkDialogSourceId.value = sourceId ?? null
  linkDialogTargetId.value = targetId ?? null
  linkDialogSourceInterfaceId.value = null
  linkDialogTargetInterfaceId.value = null
  linkDialogLinkType.value = 'manual'
  linkDialog.value = true
}

function editLinkFromEdge(edge: TopologyEdge) {
  edgeDialog.value = false
  if (edge.id > 0) {
    linkDialogEditingId.value = edge.id
  } else {
    linkDialogEditingId.value = null
  }
  linkDialogSourceId.value = edge.sourceDeviceId ?? edge.source
  linkDialogTargetId.value = edge.targetDeviceId ?? edge.target
  linkDialogSourceInterfaceId.value = edge.sourceInterfaceId ?? null
  linkDialogTargetInterfaceId.value = edge.targetInterfaceId ?? null
  linkDialogLinkType.value = edge.linkType || 'manual'
  linkDialog.value = true
}

function openLinkDialogFromNode(nodeId: number) {
  nodeDrawer.value = false
  openLinkDialog(nodeId)
}

function onLinkSaved() {
  // Atualiza posições ou mantém
}

function onSwitchCreated() {
  ensureInitialNodeLayout()
}

async function recalculateTopology() {
  await topologyStore.recalculateTopology()
  ensureInitialNodeLayout()
}

// ----------------------------------------------------
// HELPERS VISUAIS E FORMATAÇÃO
// ----------------------------------------------------

function getNodeColor(status: string) {
  switch (status?.toLowerCase()) {
    case 'online':
      return '#4CAF50'
    case 'offline':
      return '#F44336'
    case 'warning':
      return '#FF9800'
    default:
      return '#9E9E9E'
  }
}

function getNodeIcon(type: string) {
  switch (type?.toLowerCase()) {
    case 'router':
      return 'mdi-router'
    case 'switch':
      return 'mdi-expansion-card'
    case 'unmanaged_switch':
    case 'hub':
      return 'mdi-hub'
    case 'server':
      return 'mdi-server'
    case 'firewall':
      return 'mdi-shield-check'
    case 'ap':
    case 'wireless':
      return 'mdi-access-point'
    default:
      return 'mdi-desktop-tower'
  }
}

function getNodeTypeLabel(type: string) {
  switch (type?.toLowerCase()) {
    case 'router':
      return 'Roteador'
    case 'switch':
      return 'Switch Gerenciável'
    case 'unmanaged_switch':
    case 'hub':
      return 'Switch'
    case 'server':
      return 'Servidor'
    case 'firewall':
      return 'Firewall'
    case 'ap':
      return 'Ponto de Acesso Wi-Fi'
    default:
      return 'Dispositivo'
  }
}

function getLinkColor(type: string) {
  switch (type?.toLowerCase()) {
    case 'fiber':
      return '#9C27B0'
    case 'wireless':
      return '#4CAF50'
    case 'vpn':
      return '#FF9800'
    case 'lldp':
    case 'cdp':
      return '#00BCD4'
    case 'parent':
    case 'subnet':
      return '#FF9800'
    default:
      return '#2196F3'
  }
}

function getLinkTypeIcon(type?: string): string {
  switch (type?.toLowerCase()) {
    case 'fiber':
      return 'mdi-laser-pointer'
    case 'wireless':
      return 'mdi-wifi'
    case 'vpn':
      return 'mdi-shield-lock'
    case 'lldp':
    case 'cdp':
      return 'mdi-auto-fix'
    default:
      return 'mdi-ethernet'
  }
}

function getLinkTypeLabel(type?: string): string {
  switch (type?.toLowerCase()) {
    case 'fiber':
      return 'Fibra Óptica (GBIC / SFP)'
    case 'wireless':
      return 'Wireless / Enlace Sem Fio'
    case 'vpn':
      return 'Túnel VPN / Lógico'
    case 'lldp':
      return 'Descoberto via LLDP'
    case 'cdp':
      return 'Descoberto via CDP'
    case 'parent':
    case 'subnet':
      return 'Hierarquia / Sub-rede'
    default:
      return 'Cabo Ethernet (UTP)'
  }
}

function getLinkDashArray(type: string): string {
  if (type === 'parent' || type === 'subnet') {
    return '5,5'
  }
  return 'none'
}

function truncate(str: string, maxLen: number): string {
  if (!str) return ''
  return str.length > maxLen ? `${str.substring(0, maxLen)}…` : str
}
</script>

<style scoped>
.topology-view-wrapper {
  height: 100%;
  width: 100%;
  overflow: hidden;
}
.topology-map-container {
  height: 100%;
  width: 100%;
  background: #0f172a;
}
.topology-viewport {
  width: 100%;
  height: 100%;
  overflow: hidden;
  position: relative;
  user-select: none;
}
.topology-world {
  position: absolute;
  top: 0;
  left: 0;
  width: 4000px;
  height: 4000px;
  will-change: transform;
}
.topology-grid-layer {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
}
.topology-edges-layer {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 5;
}

/* Nós */
.topology-node-container {
  position: absolute;
  width: 60px;
  height: 60px;
  display: flex;
  flex-direction: column;
  align-items: center;
  z-index: 10;
  cursor: pointer;
  transition: transform 0.15s ease-out;
}
.node-dragging {
  z-index: 20;
  transition: none !important;
}
.node-highlighted .node-avatar {
  box-shadow: 0 0 0 6px rgba(var(--v-theme-primary), 0.6) !important;
  transform: scale(1.12);
}
.node-connect-source .node-avatar {
  box-shadow: 0 0 0 6px rgba(33, 150, 243, 0.8) !important;
  animation: pulse-ring 1.5s infinite;
}

/* Status Ring Luminoso */
.node-avatar-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
}
.status-pulse-ring {
  position: absolute;
  width: 58px;
  height: 58px;
  border-radius: 50%;
  opacity: 0.7;
  pointer-events: none;
}
.status-ring-online {
  border: 2px solid #4caf50;
  box-shadow: 0 0 10px rgba(76, 175, 80, 0.5);
}
.status-ring-offline {
  border: 2px solid #f44336;
  box-shadow: 0 0 10px rgba(244, 67, 54, 0.5);
  animation: blink-status 1s infinite alternate;
}
.status-ring-warning {
  border: 2px solid #ff9800;
  box-shadow: 0 0 10px rgba(255, 152, 0, 0.5);
}
.status-ring-unknown {
  border: 2px dashed #9e9e9e;
}

/* Badge de Portas no Nó */
.node-port-count-badge {
  position: absolute;
  top: -2px;
  right: -2px;
  background: #1e293b;
  color: #38bdf8;
  border: 1.5px solid #0284c7;
  border-radius: 10px;
  padding: 0 5px;
  font-size: 10px;
  line-height: 16px;
}

/* Label do Nó */
.node-label-card {
  min-width: 90px;
  max-width: 130px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
  border: 1px solid rgba(var(--v-theme-outline), 0.15);
}
.node-title {
  line-height: 1.2;
}
.node-subtitle {
  font-size: 10px;
  line-height: 1.1;
}

/* Arestas e Badges */
.edge-line {
  transition:
    stroke 0.2s,
    stroke-width 0.2s;
}
.edge-group:hover .edge-line {
  stroke-width: 4.5px !important;
  filter: drop-shadow(0 0 6px currentColor);
}
.edge-line-active {
  stroke-linecap: round;
}
.port-pill-bg {
  fill: #1e293b;
  stroke: #475569;
  stroke-width: 1;
  filter: drop-shadow(0 1px 3px rgba(0, 0, 0, 0.5));
}
.port-pill-text {
  fill: #f8fafc;
  font-size: 9px;
  font-family: monospace;
  font-weight: bold;
}

/* Badge de Tráfego / Consumo no Centro do Link */
.traffic-pill-bg {
  fill: #0f172a;
  stroke: #38bdf8;
  stroke-width: 1.5;
  filter: drop-shadow(0 2px 5px rgba(0, 0, 0, 0.6));
}
.traffic-pill-text {
  fill: #38bdf8;
  font-size: 10px;
  font-family: monospace;
  font-weight: bold;
}

/* Controles Flutuantes */
.floating-controls-wrapper {
  position: absolute;
  top: 16px;
  left: 16px;
  z-index: 30;
}
.floating-search-wrapper {
  position: absolute;
  top: 16px;
  right: 16px;
  z-index: 30;
  width: 280px;
}
.search-autocomplete {
  background: rgba(var(--v-theme-surface), 0.9) !important;
  backdrop-filter: blur(10px);
  border-radius: 12px;
}

/* Banner do Modo Conexão */
.connect-mode-banner {
  position: absolute;
  top: 20px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 40;
  background: rgba(33, 150, 243, 0.95);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
}

/* Alças / Conectores Magnéticos nos Nós no Modo de Conexão */
.connect-port-handles {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
.port-handle {
  position: absolute;
  width: 8px;
  height: 8px;
  background: #2196f3;
  border: 1.5px solid white;
  border-radius: 50%;
  box-shadow: 0 0 6px #2196f3;
}
.handle-top {
  top: -4px;
  left: 26px;
}
.handle-right {
  right: -4px;
  top: 24px;
}
.handle-bottom {
  bottom: 12px;
  left: 50%;
  transform: translateX(-50%);
}
.handle-left {
  left: -4px;
  top: 24px;
}

.cursor-grab {
  cursor: grab;
}
.cursor-grabbing {
  cursor: grabbing;
}
.cursor-crosshair {
  cursor: crosshair;
}
.gap-2 {
  gap: 8px;
}
.font-mono {
  font-family: monospace;
}
.bg-surface-variant-subtle {
  background: rgba(var(--v-theme-surface-variant), 0.35);
}
</style>

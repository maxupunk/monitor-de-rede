<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Mapa de Topologia de Rede</h1>
        <p class="text-subtitle-1 text-grey-darken-1">Visualização de vizinhos LLDP/CDP, sub-redes e links físicos</p>
      </div>
      <div class="d-flex gap-2">
        <v-btn
          color="secondary"
          prepend-icon="mdi-calculator"
          :loading="topologyStore.recalculating"
          @click="topologyStore.recalculateTopology()"
        >
          Recalcular Topologia
        </v-btn>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="linkDialog = true">
          Adicionar Conexão
        </v-btn>
      </div>
    </div>

    <!-- Container do Mapa Gráfico -->
    <v-card elevation="2" class="rounded-lg overflow-hidden position-relative" style="height: 600px;">
      <!-- Canvas / Overlay SVG de Topologia -->
      <svg width="100%" height="100%" class="topology-canvas">
        <!-- Areias/Conexões (Edges) -->
        <g v-for="edge in edgePositions" :key="edge.id">
          <line
            :x1="edge.x1"
            :y1="edge.y1"
            :x2="edge.x2"
            :y2="edge.y2"
            :stroke="getLinkColor(edge.linkType)"
            stroke-width="3"
            :stroke-dasharray="edge.linkType === 'subnet' ? '5,5' : 'none'"
          />
        </g>
      </svg>

      <!-- Nós / Equipamentos (Nodes) -->
      <div
        v-for="node in nodePositions"
        :key="node.id"
        class="topology-node cursor-pointer pa-3 rounded-circle text-center elevation-4"
        :style="{ left: `${node.x}px`, top: `${node.y}px` }"
        @click="selectNode(node)"
      >
        <v-avatar :color="getNodeColor(node.status)" size="44">
          <v-icon color="white" size="24">
            {{ getNodeIcon(node.type) }}
          </v-icon>
        </v-avatar>
        <div class="node-label font-weight-bold text-caption mt-1 px-2 rounded bg-surface">
          {{ node.name }}
        </div>
      </div>

      <!-- Sem dados ou carregando -->
      <div v-if="topologyStore.nodes.length === 0" class="d-flex align-center justify-center fill-height text-grey">
        <div class="text-center">
          <v-icon size="64" color="grey-lighten-1" class="mb-2">mdi-sitemap</v-icon>
          <div class="text-h6">Nenhum equipamento mapeado na topologia</div>
          <p class="text-caption">Clique em "Recalcular Topologia" ou cadastre dispositivos e sub-redes.</p>
        </div>
      </div>
    </v-card>

    <!-- Drawer de Detalhes do Nó Selecionado -->
    <v-dialog v-model="nodeDrawer" max-width="400">
      <v-card v-if="selectedNode" class="rounded-lg pa-4">
        <v-card-title class="d-flex align-center">
          <v-avatar :color="getNodeColor(selectedNode.status)" size="36" class="mr-3">
            <v-icon color="white">{{ getNodeIcon(selectedNode.type) }}</v-icon>
          </v-avatar>
          <div>
            <div class="text-h6 font-weight-bold">{{ selectedNode.name }}</div>
            <div class="text-caption text-grey">IP: {{ selectedNode.ipAddress || 'N/A' }}</div>
          </div>
        </v-card-title>
        <v-divider class="my-3"></v-divider>
        <v-card-text>
          <p><strong>Tipo:</strong> {{ selectedNode.type }}</p>
          <p><strong>Fabricante:</strong> {{ selectedNode.vendor || 'Desconhecido' }}</p>
          <p><strong>Status:</strong> {{ (selectedNode.status || 'UNKNOWN').toUpperCase() }}</p>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn color="primary" variant="outlined" :to="`/devices/${selectedNode.id}`">
            Ver Detalhes do Dispositivo
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Modal para Adicionar Conexão Manual -->
    <v-dialog v-model="linkDialog" max-width="500">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">Adicionar Link de Topologia</v-card-title>
        <v-card-text>
          <v-form @submit.prevent="saveLink">
            <v-select
              v-model="newLink.sourceDeviceId"
              :items="topologyStore.nodes"
              item-title="name"
              item-value="id"
              label="Dispositivo de Origem"
              variant="outlined"
              required
            ></v-select>
            <v-select
              v-model="newLink.targetDeviceId"
              :items="topologyStore.nodes"
              item-title="name"
              item-value="id"
              label="Dispositivo de Destino"
              variant="outlined"
              required
            ></v-select>
          </v-form>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="linkDialog = false">Cancelar</v-btn>
          <v-btn color="primary" @click="saveLink">Salvar Conexão</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, reactive } from 'vue'
import { useTopologyStore, type TopologyNode, type TopologyEdge } from '@/stores/topology'

interface RenderedNode extends TopologyNode {
  x: number
  y: number
}

interface RenderedEdge {
  id: number
  linkType: string
  x1: number
  y1: number
  x2: number
  y2: number
}

const topologyStore = useTopologyStore()
const selectedNode = ref<TopologyNode | null>(null)
const nodeDrawer = ref(false)
const linkDialog = ref(false)

const newLink = reactive<{ sourceDeviceId: number; targetDeviceId: number }>({
  sourceDeviceId: 1,
  targetDeviceId: 2,
})

onMounted(() => {
  topologyStore.fetchTopology()
})

const nodePositions = computed<RenderedNode[]>(() => {
  const list = topologyStore.nodes
  const total = list.length
  const centerX = 400
  const centerY = 280
  const radius = Math.min(220, total * 35)

  return list.map((node: TopologyNode, index: number) => {
    const angle = (index / (total || 1)) * 2 * Math.PI
    return {
      ...node,
      x: node.x ?? centerX + radius * Math.cos(angle) - 25,
      y: node.y ?? centerY + radius * Math.sin(angle) - 25,
    }
  })
})

const edgePositions = computed<RenderedEdge[]>(() => {
  const nodesMap = new Map<number, RenderedNode>(nodePositions.value.map((n) => [n.id, n]))
  const result: RenderedEdge[] = []
  for (const edge of topologyStore.edges as TopologyEdge[]) {
    const src = nodesMap.get(edge.sourceDeviceId)
    const tgt = nodesMap.get(edge.targetDeviceId)
    if (src && tgt) {
      result.push({
        id: edge.id,
        linkType: edge.linkType,
        x1: src.x + 25,
        y1: src.y + 25,
        x2: tgt.x + 25,
        y2: tgt.y + 25,
      })
    }
  }
  return result
})

function selectNode(node: TopologyNode) {
  selectedNode.value = node
  nodeDrawer.value = true
}

function getNodeColor(status: string) {
  switch (status) {
    case 'online': return 'success'
    case 'offline': return 'error'
    case 'warning': return 'warning'
    default: return 'grey'
  }
}

function getNodeIcon(type: string) {
  switch (type?.toLowerCase()) {
    case 'router': return 'mdi-router'
    case 'switch': return 'mdi-expansion-card'
    case 'server': return 'mdi-server'
    case 'firewall': return 'mdi-shield-check'
    default: return 'mdi-desktop-tower'
  }
}

function getLinkColor(type: string) {
  switch (type) {
    case 'lldp':
    case 'cdp': return '#4CAF50'
    case 'manual': return '#2196F3'
    default: return '#FF9800'
  }
}

async function saveLink() {
  if (!newLink.sourceDeviceId || !newLink.targetDeviceId) return
  await topologyStore.addLink(newLink)
  linkDialog.value = false
}
</script>

<style scoped>
.topology-canvas {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
}
.topology-node {
  position: absolute;
  transform: translate(0, 0);
  transition: all 0.3s ease;
  z-index: 10;
}
.node-label {
  white-space: nowrap;
  box-shadow: 0 1px 3px rgba(0,0,0,0.2);
}
</style>

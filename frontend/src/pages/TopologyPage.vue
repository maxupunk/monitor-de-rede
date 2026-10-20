<template>
  <div ref="root" class="topology-page">
    <TopologyToolbar
      v-model:site-id="prefs.siteId"
      v-model:filters="prefs.filters"
      v-model:legend-open="prefs.legendOpen"
      v-model:group-by-site="prefs.groupBySite"
      v-model:always-detail="prefs.alwaysDetail"
      v-model:orthogonal-edges="prefs.orthogonalEdges"
      :edit-mode="editMode"
      :sites="sites.sites"
      :nodes="store.nodes"
      :filter-count="activeFilterCount(prefs.filters)"
      :can-write="auth.canWrite"
      :computing="layout.computing.value"
      :new-count="layout.autoPlaced.value.size"
      :fullscreen="cmd.fullscreen.value"
      :compact="mdAndDown"
      @update:edit-mode="cmd.setEditMode"
      @search="cmd.searchOpen.value = true"
      @layout="cmd.runLayout"
      @arrange-new="layout.arrangeNew"
      @add="cmd.onAdd"
      @export="cmd.exportPng"
      @fullscreen="cmd.toggleFullscreen"
      @help="cmd.shortcutsOpen.value = true"
      @more="moreOpen = true"
    />

    <div class="topology-body">
      <div class="map-area">
        <TopologyCanvas
          v-if="layout.ready.value"
          ref="canvas"
          :elements="view.elements.value"
          :palette="palette"
          :editable="editMode"
          :connect-source-id="cmd.cable.source"
          :always-detail="prefs.alwaysDetail"
          :orthogonal="prefs.orthogonalEdges"
          :selection="view.selection.value"
          :insets="insets"
          @ready="onCanvasReady"
          @node-tap="onNodeTap"
          @edge-tap="(id) => (view.selection.value = { kind: 'edge', id })"
          @background-tap="view.selection.value = null"
          @context="onContext"
          @hover="onHover"
          @positions-changed="layout.onUserMoved"
          @drag-state="layout.setDragging"
          @viewport="onViewport"
        />
        <TopologyTooltip
          v-if="view.hover.value && !cmd.context.open"
          :node="view.hoveredNode.value"
          :edge="view.hoveredEdge.value"
          :x="view.hover.value.x"
          :y="view.hover.value.y"
        />
        <TopologyEdgeQuickTest
          v-if="quickTest.state.value && !cmd.context.open && !cmd.cable.active"
          :x="quickTest.state.value.x"
          :y="quickTest.state.value.y"
          @test="testQuickEdge"
          @hold="quickTest.hold"
          @release="quickTest.release"
        />

        <TopologyMapHud
          v-model:legend-open="prefs.legendOpen"
          :has-nodes="hasNodes"
          :narrow="smAndDown"
          :edit-mode="editMode"
          :cable-active="cmd.cable.active"
          :cable-source-name="cableSourceName"
          :highlighted="overlay.highlight !== null && !linkTestLayer?.overlayActive"
          :computing="layout.computing.value !== null"
          :minimap-source="canvas?.minimapSource ?? null"
          :minimap-open="minimapOpen"
          :palette="palette"
          :summary="view.summary.value"
          :problems-only="prefs.filters.problemsOnly"
          :zoom="zoom"
          @cable="cmd.startCableMode"
          @cancel-cable="cmd.cancelCable"
          @edit-done="cmd.setEditMode(false)"
          @clear-highlight="overlay.setPathHighlight(null)"
          @toggle-problems="toggleProblems"
          @zoom="(factor) => canvas?.zoomBy(factor)"
          @fit="fitMap()"
          @reset-zoom="canvas?.resetZoom()"
          @toggle-minimap="toggleMinimap"
        />
        <div v-if="showTips" class="tips-wrap" :class="{ narrow: smAndDown }">
          <TopologyTips
            :narrow="smAndDown"
            :can-write="auth.canWrite"
            @dismiss="prefs.tipsDismissed = true"
            @shortcuts="cmd.shortcutsOpen.value = true"
          />
        </div>
        <TopologyEmptyState
          v-if="(!layout.ready.value && !store.error) || store.error || (store.loaded && !hasNodes)"
          :loading="!layout.ready.value && !store.error"
          :error="store.error"
          :filtered-by-site="prefs.siteId !== null"
          :can-write="auth.canWrite"
          :discovering="store.recalculating"
          @retry="loadSite"
          @discover="host.discoverLinks()"
          @add-switch="host.addUnmanagedSwitch()"
          @all-sites="prefs.siteId = null"
        />
        <TopologyLinkTestLayer ref="linkTestLayer" :fit-nodes="fitMap" :narrow="smAndDown" />
        <TopologyQuickPingLayer />
      </div>

      <TopologyInspector
        v-if="view.selectedTarget.value && !smAndDown"
        v-model:tab-choice="view.inspectorTab.value"
        :target="view.selectedTarget.value"
        :host="host"
        @close="view.selection.value = null"
      />
    </div>

    <!-- Celular: folha sem véu, o item selecionado continua visível acima dela. -->
    <v-bottom-sheet v-if="smAndDown" v-model="sheetOpen" :scrim="false">
      <TopologyInspector
        v-if="view.selectedTarget.value"
        v-model:tab-choice="view.inspectorTab.value"
        :target="view.selectedTarget.value"
        :host="host"
        sheet
        @close="view.selection.value = null"
      />
    </v-bottom-sheet>

    <TopologyContextMenu
      v-model="cmd.context.open"
      :target="cmd.context.target"
      :host="host"
      :x="cmd.context.x"
      :y="cmd.context.y"
    />
    <TopologySearch v-model="cmd.searchOpen.value" :nodes="store.nodes" @select="host.focusNode" />
    <TopologyShortcutsDialog v-model="cmd.shortcutsOpen.value" />
    <TopologyMobileSheet
      v-if="mdAndDown"
      v-model="moreOpen"
      v-model:filters="prefs.filters"
      v-model:legend-open="prefs.legendOpen"
      v-model:group-by-site="prefs.groupBySite"
      v-model:always-detail="prefs.alwaysDetail"
      v-model:orthogonal-edges="prefs.orthogonalEdges"
      :nodes="store.nodes"
      :can-write="auth.canWrite"
      :new-count="layout.autoPlaced.value.size"
      @layout="cmd.runLayout"
      @arrange-new="layout.arrangeNew"
      @add="cmd.onAdd"
      @export="cmd.exportPng"
    />
    <TopologyDialogs :site-id="prefs.siteId" @saved="notify" />
    <TopologyPluginRunner ref="pluginRunner" @error="(message) => notify(message, 'error')" />
    <TopologyFeedback :save-error="layout.saveError.value" @retry-save="layout.retrySave" />
  </div>
</template>

<script setup lang="ts">
/**
 * Mapa de topologia — composição fina: o canvas desenha, o inspetor detalha,
 * a barra comanda. Estado e regras vivem nos composables e utilitários de
 * `topology/`; extensões entram pelo `inspectorRegistry`.
 */
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useAuthStore } from '@/stores/auth'
import { useDevicesStore } from '@/stores/devices'
import { useSitesStore } from '@/stores/sites'
import { useTopologyStore } from '@/stores/topology'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import { activeFilterCount } from '@/utils/topology/filters'
import {
  edgeLinkTestRequest,
  registerBuiltinTopologyActions,
} from '@/utils/topology/builtinActions'
import {
  mapInsets,
  type CanvasPointerEvent,
  type CanvasViewport,
} from '@/utils/topology/canvasTypes'
import { minimapVisible } from '@/utils/topology/prefs'
import { useTopologyPrefs } from '@/composables/topology/useTopologyPrefs'
import { useTopologyPalette } from '@/composables/topology/useTopologyPalette'
import { useTopologyView } from '@/composables/topology/useTopologyView'
import { useTopologyLayout } from '@/composables/topology/useTopologyLayout'
import { useTopologyDialogs } from '@/composables/topology/useTopologyDialogs'
import { useTopologyHost } from '@/composables/topology/useTopologyHost'
import { useTopologyCommands } from '@/composables/topology/useTopologyCommands'
import { useTopologyKeyboard } from '@/composables/topology/useTopologyKeyboard'
import { useTopologyPluginActions } from '@/composables/topology/useTopologyPluginActions'
import { provideSheetStack } from '@/composables/topology/useSheetStack'
import { useEdgeQuickTest } from '@/composables/topology/useEdgeQuickTest'
import TopologyCanvas from '@/components/topology/canvas/TopologyCanvas.vue'
import TopologyToolbar from '@/components/topology/TopologyToolbar.vue'
import TopologyTooltip from '@/components/topology/TopologyTooltip.vue'
import TopologyMapHud from '@/components/topology/TopologyMapHud.vue'
import TopologyEmptyState from '@/components/topology/TopologyEmptyState.vue'
import TopologyInspector from '@/components/topology/TopologyInspector.vue'
import TopologyContextMenu from '@/components/topology/TopologyContextMenu.vue'
import TopologySearch from '@/components/topology/TopologySearch.vue'
import TopologyShortcutsDialog from '@/components/topology/TopologyShortcutsDialog.vue'
import TopologyMobileSheet from '@/components/topology/TopologyMobileSheet.vue'
import TopologyDialogs from '@/components/topology/TopologyDialogs.vue'
import TopologyPluginRunner from '@/components/topology/TopologyPluginRunner.vue'
import TopologyFeedback from '@/components/topology/TopologyFeedback.vue'
import TopologyLinkTestLayer from '@/components/topology/linkTests/TopologyLinkTestLayer.vue'
import TopologyQuickPingLayer from '@/components/topology/quickPing/TopologyQuickPingLayer.vue'
import TopologyTips from '@/components/topology/TopologyTips.vue'
import TopologyEdgeQuickTest from '@/components/topology/TopologyEdgeQuickTest.vue'

const store = useTopologyStore()
const overlay = useTopologyOverlayStore()
const auth = useAuthStore()
const sites = useSitesStore()
const devices = useDevicesStore()
const prefs = useTopologyPrefs()
const palette = useTopologyPalette()
const view = useTopologyView(prefs)
const dialogs = useTopologyDialogs()
const pluginActions = useTopologyPluginActions()
const { smAndDown, mdAndDown, height } = useDisplay()

const root = ref<HTMLElement | null>(null)
const canvas = ref<InstanceType<typeof TopologyCanvas> | null>(null)
const pluginRunner = ref<InstanceType<typeof TopologyPluginRunner> | null>(null)
const linkTestLayer = ref<InstanceType<typeof TopologyLinkTestLayer> | null>(null)
const zoom = ref(1)
/** O Cytoscape já desenhou: antes disso, enquadrar vira pedido na fila. */
const canvasReady = ref(false)
/** Celular: folhas inferiores, uma por vez (a última vence; voltar devolve a anterior). */
const sheets = provideSheetStack()

const moreOpen = computed({
  get: () => sheets.isTop('options'),
  set: (open) => (open ? sheets.open('options') : sheets.close('options')),
})
const editMode = computed(() => prefs.editMode && auth.canWrite)
const hasNodes = computed(() => store.nodes.length > 0)
const minimapOpen = computed(
  () => !smAndDown.value && minimapVisible(prefs.minimap, store.nodes.length)
)
const showTips = computed(
  () => !prefs.tipsDismissed && hasNodes.value && layout.ready.value && !editMode.value
)
const insets = computed(() =>
  mapInsets({
    narrow: smAndDown.value,
    editMode: editMode.value,
    legendOpen: prefs.legendOpen && hasNodes.value,
    minimap: minimapOpen.value,
    // No celular a folha de cima cobre a parte de baixo do mapa.
    sheetHeight: sheets.top && sheets.top !== 'options' ? Math.round(height.value * 0.6) : 0,
  })
)

const layout = useTopologyLayout({ canvas: () => canvas.value, canPersist: () => auth.canWrite })
const cmd = useTopologyCommands({
  prefs,
  selection: view.selection,
  selectedTarget: view.selectedTarget,
  targetFor: view.targetFor,
  editMode: () => editMode.value,
  canvas: () => canvas.value,
  root,
  runLayout: layout.runLayout,
  getHost: () => host,
  notify: (text, color) => notify(text, color),
})
const { host, notify } = useTopologyHost({
  selection: view.selection,
  editMode: () => editMode.value,
  cableSource: computed({
    get: () => cmd.cable.source,
    set: (id) => Object.assign(cmd.cable, { active: id !== null, source: id }),
  }),
  dialogs,
  canvas: () => canvas.value,
  fit: (ids) => fitMap(ids),
  openTab: view.openTab,
  runPlugin: (node, item) => void pluginRunner.value?.open(node, item),
})
const nodeOf = (id: number) => store.nodesById.get(id)
const quickTest = useEdgeQuickTest((edgeId) => {
  if (!auth.canWrite || smAndDown.value || cmd.cable.active) return false
  const edge = store.edgesById.get(edgeId)
  return edge !== undefined && edgeLinkTestRequest(edge, nodeOf) !== null
})
registerBuiltinTopologyActions(() => store.pluginActions)
useTopologyKeyboard(cmd.onCommand, () => !anyDialogOpen.value)

const cableSourceName = computed(() =>
  cmd.cable.source !== null ? (store.nodesById.get(cmd.cable.source)?.name ?? null) : null
)
const sheetOpen = computed({
  get: () => sheets.isTop('inspector') && Boolean(view.selectedTarget.value),
  set: (open) => {
    if (!open) view.selection.value = null
  },
})

// Selecionar põe o inspetor por cima (mesmo com o teste aberto); limpar o tira.
watch(
  () => view.selection.value,
  (selection) => (selection ? sheets.open('inspector') : sheets.close('inspector'))
)

/**
 * Enquadra agora ou, se o canvas ainda não desenhou (troca de site, carga da
 * página vinda de "Ver no mapa"), assim que ele avisar que está pronto.
 */
let queuedFit: { ids?: number[] } | null = null
function fitMap(ids?: number[]) {
  if (canvasReady.value && canvas.value) canvas.value.fit(ids)
  else queuedFit = { ids }
}

function onCanvasReady() {
  canvasReady.value = true
  const request = queuedFit
  queuedFit = null
  canvas.value?.fit(request?.ids)
}

watch(
  () => layout.ready.value,
  (ready) => {
    if (!ready) canvasReady.value = false
  }
)

function onNodeTap(id: number) {
  cmd.onNodeTap(id)
  // O painel/folha que abre pode cobrir o nó: traz ele para a área livre.
  if (!cmd.cable.active) void nextTick(() => canvas.value?.reveal(id))
}

function onContext(event: CanvasPointerEvent) {
  quickTest.hide()
  cmd.openContext(event)
}

function onHover(event: CanvasPointerEvent | null) {
  view.hover.value = event
  quickTest.onHover(event)
}

function onViewport(viewport: CanvasViewport) {
  if (viewport.zoom !== zoom.value) quickTest.hide()
  zoom.value = viewport.zoom
}

function testQuickEdge() {
  const edge = quickTest.state.value ? store.edgesById.get(quickTest.state.value.edgeId) : null
  const request = edge ? edgeLinkTestRequest(edge, nodeOf) : null
  quickTest.hide()
  if (request) host.requestLinkTest(request)
}

function toggleMinimap() {
  prefs.minimap = minimapOpen.value ? 'off' : 'on'
}
const anyDialogOpen = computed(
  () =>
    cmd.searchOpen.value ||
    cmd.shortcutsOpen.value ||
    cmd.context.open ||
    moreOpen.value ||
    dialogs.link.open ||
    dialogs.unmanagedSwitch.open ||
    dialogs.device.open ||
    Boolean(linkTestLayer.value?.dialogOpen)
)

function toggleProblems() {
  prefs.filters = { ...prefs.filters, problemsOnly: !prefs.filters.problemsOnly }
}

async function loadSite() {
  view.selection.value = null
  quickTest.hide()
  await store.fetchTopology(prefs.siteId)
  if (!store.error) await layout.initialize()
}

watch(
  () => prefs.siteId,
  async () => {
    await layout.flush()
    overlay.setPathHighlight(null)
    await loadSite()
  }
)

onMounted(async () => {
  if (sites.sites.length === 0) void sites.fetchSites()
  void pluginActions.load()
  // O cadastro alimenta a edição e os seletores do teste de enlace.
  if (devices.devices.length === 0) void devices.fetchDevices()
  await loadSite()
})
</script>

<style scoped>
.topology-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: rgb(var(--v-theme-background));
}
.topology-body {
  flex: 1;
  display: flex;
  min-height: 0;
}
.map-area {
  position: relative;
  flex: 1;
  min-width: 0;
  overflow: hidden;
}
.tips-wrap {
  position: absolute;
  z-index: 6;
  top: 56px;
  left: 50%;
  transform: translateX(-50%);
}
.tips-wrap.narrow {
  top: 8px;
  left: 8px;
  right: 8px;
  transform: none;
}
</style>

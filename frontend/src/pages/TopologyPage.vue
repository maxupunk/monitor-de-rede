<template>
  <div ref="root" class="topology-page">
    <TopologyToolbar
      v-model:site-id="prefs.siteId"
      v-model:filters="prefs.filters"
      v-model:legend-open="prefs.legendOpen"
      v-model:group-by-site="prefs.groupBySite"
      v-model:always-detail="prefs.alwaysDetail"
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
          :selection="view.selection.value"
          :insets="insets"
          @ready="canvas?.fit()"
          @node-tap="cmd.onNodeTap"
          @edge-tap="(id) => (view.selection.value = { kind: 'edge', id })"
          @background-tap="view.selection.value = null"
          @context="cmd.openContext"
          @hover="(event) => (view.hover.value = event)"
          @positions-changed="layout.onUserMoved"
          @drag-state="layout.setDragging"
          @viewport="(v) => (zoom = v.zoom)"
        />
        <TopologyTooltip
          v-if="view.hover.value && !cmd.context.open"
          :node="view.hoveredNode.value"
          :edge="view.hoveredEdge.value"
          :x="view.hover.value.x"
          :y="view.hover.value.y"
        />

        <TopologyMapHud
          v-model:legend-open="prefs.legendOpen"
          :has-nodes="hasNodes"
          :narrow="smAndDown"
          :edit-mode="editMode"
          :cable-active="cmd.cable.active"
          :cable-source-name="cableSourceName"
          :highlighted="overlay.highlight !== null"
          :computing="layout.computing.value !== null"
          :minimap-source="canvas?.minimapSource ?? null"
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
          @fit="canvas?.fit()"
          @reset-zoom="canvas?.resetZoom()"
        />
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
      </div>

      <TopologyInspector
        v-if="view.selectedTarget.value && !smAndDown"
        :target="view.selectedTarget.value"
        :host="host"
        @close="view.selection.value = null"
      />
    </div>

    <v-bottom-sheet v-if="smAndDown" v-model="sheetOpen">
      <TopologyInspector
        v-if="view.selectedTarget.value"
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
import { computed, onMounted, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useAuthStore } from '@/stores/auth'
import { useDevicesStore } from '@/stores/devices'
import { useSitesStore } from '@/stores/sites'
import { useTopologyStore } from '@/stores/topology'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import { activeFilterCount } from '@/utils/topology/filters'
import { registerBuiltinTopologyActions } from '@/utils/topology/builtinActions'
import { useTopologyPrefs } from '@/composables/topology/useTopologyPrefs'
import { useTopologyPalette } from '@/composables/topology/useTopologyPalette'
import { useTopologyView } from '@/composables/topology/useTopologyView'
import { useTopologyLayout } from '@/composables/topology/useTopologyLayout'
import { useTopologyDialogs } from '@/composables/topology/useTopologyDialogs'
import { useTopologyHost } from '@/composables/topology/useTopologyHost'
import { useTopologyCommands } from '@/composables/topology/useTopologyCommands'
import { useTopologyKeyboard } from '@/composables/topology/useTopologyKeyboard'
import { useTopologyPluginActions } from '@/composables/topology/useTopologyPluginActions'
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
const zoom = ref(1)
const moreOpen = ref(false)

const editMode = computed(() => prefs.editMode && auth.canWrite)
const hasNodes = computed(() => store.nodes.length > 0)
const insets = computed(() => ({
  top: editMode.value ? 64 : 0,
  right: smAndDown.value ? 0 : 64,
  // No celular o inspetor é uma folha que cobre a parte de baixo do mapa.
  bottom: smAndDown.value && view.selectedTarget.value ? Math.round(height.value * 0.55) : 56,
  left: prefs.legendOpen && !smAndDown.value ? 230 : 0,
}))

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
  runPlugin: (node, item) => void pluginRunner.value?.open(node, item),
})
registerBuiltinTopologyActions(() => store.pluginActions)
useTopologyKeyboard(cmd.onCommand, () => !anyDialogOpen.value)

const cableSourceName = computed(() =>
  cmd.cable.source !== null ? (store.nodesById.get(cmd.cable.source)?.name ?? null) : null
)
const sheetOpen = computed({
  get: () => Boolean(view.selectedTarget.value),
  set: (open) => {
    if (!open) view.selection.value = null
  },
})
const anyDialogOpen = computed(
  () =>
    cmd.searchOpen.value ||
    cmd.shortcutsOpen.value ||
    cmd.context.open ||
    moreOpen.value ||
    dialogs.link.open ||
    dialogs.unmanagedSwitch.open ||
    dialogs.device.open
)

function toggleProblems() {
  prefs.filters = { ...prefs.filters, problemsOnly: !prefs.filters.problemsOnly }
}

async function loadSite() {
  view.selection.value = null
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
  // O cadastro diz qual nó é o servidor NetMonitor (e alimenta a edição).
  if (devices.devices.length === 0) await devices.fetchDevices()
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
</style>

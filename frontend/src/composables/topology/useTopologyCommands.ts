import { onBeforeUnmount, onMounted, reactive, ref, type Ref } from 'vue'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import { useTopologyStore } from '@/stores/topology'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import { triggerDownload } from '@/utils/download'
import type { CanvasPointerEvent } from '@/utils/topology/canvasTypes'
import type { TopologyHost, TopologyTarget } from '@/utils/topology/inspectorRegistry'
import { LAYOUT_OPTIONS, type LayoutKind } from '@/utils/topology/layouts'
import type { TopologyPrefs } from '@/utils/topology/prefs'
import { nudgeDelta, type TopologyCommand } from '@/utils/topology/shortcuts'
import type { AddKind } from '@/components/topology/TopologyAddList.vue'
import type { Selection } from './useTopologyHost'

/** O que os comandos usam do canvas. */
export interface CommandCanvas {
  fit(ids?: number[]): void
  zoomBy(factor: number): void
  resetZoom(): void
  nudge(deviceId: number, dx: number, dy: number): void
  exportPng(): Promise<Blob | null>
}

interface Deps {
  prefs: TopologyPrefs
  selection: Ref<Selection | null>
  selectedTarget: Ref<TopologyTarget | null>
  targetFor: (event: CanvasPointerEvent) => TopologyTarget | null
  editMode: () => boolean
  canvas: () => CommandCanvas | null
  root: Ref<HTMLElement | null>
  runLayout: (kind: LayoutKind) => Promise<void>
  getHost: () => TopologyHost
  notify: (text: string, color?: 'success' | 'error' | 'info' | 'warning') => void
}

/**
 * Comandos do mapa (teclado, barra, toques): cabo, menu de contexto, layout,
 * exportação e tela cheia. A página só liga eventos a estas funções.
 */
export function useTopologyCommands(deps: Deps) {
  const store = useTopologyStore()
  const overlay = useTopologyOverlayStore()
  const auth = useAuthStore()

  const cable = reactive({ active: false, source: null as number | null })
  const context = reactive({ open: false, x: 0, y: 0, target: null as TopologyTarget | null })
  const fullscreen = ref(false)
  const searchOpen = ref(false)
  const shortcutsOpen = ref(false)

  function setEditMode(value: boolean) {
    deps.prefs.editMode = value
    if (!value) cancelCable()
  }
  function startCableMode() {
    Object.assign(cable, { active: true, source: null })
  }
  function cancelCable() {
    Object.assign(cable, { active: false, source: null })
  }

  function onNodeTap(id: number) {
    if (cable.active) {
      if (cable.source === null) cable.source = id
      else if (cable.source !== id) {
        deps.getHost().connectFrom(cable.source, id)
        cancelCable()
      }
      return
    }
    deps.selection.value = { kind: 'node', id }
  }

  function openContext(event: CanvasPointerEvent) {
    const target = deps.targetFor(event)
    if (!target) return
    if (target.kind !== 'background' && event.id !== undefined) {
      deps.selection.value = { kind: target.kind, id: event.id }
    }
    Object.assign(context, { open: true, x: event.clientX, y: event.clientY, target })
  }

  async function runLayout(kind: LayoutKind) {
    const title = LAYOUT_OPTIONS.find((option) => option.value === kind)?.title ?? 'Layout'
    const ok = await confirm({
      title: `Reorganizar tudo: ${title}`,
      message: `Os ${store.nodes.length} dispositivos serão reposicionados, inclusive os que você ajustou à mão. A nova arrumação vale para todos que abrem este mapa.`,
      confirmText: 'Reorganizar',
      confirmColor: 'warning',
    })
    if (!ok) return
    deps.prefs.groupBySite = kind === 'sites'
    try {
      await deps.runLayout(kind)
    } catch (err: unknown) {
      deps.notify(err instanceof Error ? err.message : 'Falha ao organizar o mapa', 'error')
    }
  }

  function onAdd(kind: AddKind) {
    const host = deps.getHost()
    if (kind === 'link') host.addLink()
    else if (kind === 'switch') host.addUnmanagedSwitch()
    else if (kind === 'discover') host.discoverLinks()
    else host.addDevice()
  }

  async function exportPng() {
    const blob = await deps.canvas()?.exportPng()
    if (!blob) {
      deps.notify('Nada para exportar ainda.', 'warning')
      return
    }
    triggerDownload(blob, `topologia-${new Date().toISOString().slice(0, 10)}.png`)
  }

  function toggleFullscreen() {
    if (document.fullscreenElement) void document.exitFullscreen()
    else void deps.root.value?.requestFullscreen?.()
  }
  function onFullscreenChange() {
    fullscreen.value = Boolean(document.fullscreenElement)
  }
  onMounted(() => document.addEventListener('fullscreenchange', onFullscreenChange))
  onBeforeUnmount(() => document.removeEventListener('fullscreenchange', onFullscreenChange))

  function onCommand(command: TopologyCommand, event: KeyboardEvent) {
    const selected = deps.selection.value
    const canvas = deps.canvas()
    const nudge = nudgeDelta(command, event.shiftKey)
    if (nudge) {
      if (deps.editMode() && selected?.kind === 'node') canvas?.nudge(selected.id, ...nudge)
      return
    }
    switch (command) {
      case 'search':
        searchOpen.value = true
        break
      case 'fit':
        canvas?.fit()
        break
      case 'zoom-in':
        canvas?.zoomBy(1.25)
        break
      case 'zoom-out':
        canvas?.zoomBy(0.8)
        break
      case 'zoom-reset':
        canvas?.resetZoom()
        break
      case 'toggle-legend':
        deps.prefs.legendOpen = !deps.prefs.legendOpen
        break
      case 'toggle-edit':
        if (auth.canWrite) setEditMode(!deps.prefs.editMode)
        break
      case 'help':
        shortcutsOpen.value = true
        break
      case 'escape':
        if (cable.active) cancelCable()
        else if (overlay.highlight) overlay.setPathHighlight(null)
        else deps.selection.value = null
        break
      case 'delete': {
        const target = deps.selectedTarget.value
        if (!deps.editMode() || !target) break
        if (target.kind === 'node') deps.getHost().deleteNode(target.node)
        else if (target.kind === 'edge') deps.getHost().deleteEdge(target.edge)
        break
      }
    }
  }

  return {
    cable,
    context,
    fullscreen,
    searchOpen,
    shortcutsOpen,
    setEditMode,
    startCableMode,
    cancelCable,
    onNodeTap,
    openContext,
    runLayout,
    onAdd,
    exportPng,
    toggleFullscreen,
    onCommand,
  }
}

import { onBeforeUnmount, ref, watch } from 'vue'
import { useTopologyStore, type TopologyPosition } from '@/stores/topology'
import {
  computeLayout,
  gridLayout,
  placeNewNodes,
  type LayoutKind,
  type PositionMap,
} from '@/utils/topology/layouts'
import { createLayoutSaver } from '@/utils/topology/layoutSaver'

/** O que o layout precisa do canvas. */
export interface CanvasLayoutApi {
  applyPositions(
    positions: Record<number, TopologyPosition>,
    animate?: boolean,
    fit?: boolean
  ): void
  fit(deviceIds?: number[]): void
}

interface Options {
  canvas: () => CanvasLayoutApi | null
  /** Perfil com escrita: posições vão para o servidor. */
  canPersist: () => boolean
}

function describe(err: unknown): string {
  return err instanceof Error && err.message ? err.message : 'Falha ao salvar as posições do mapa'
}

/**
 * Posições do mapa: carrega as do servidor, posiciona só os nós novos, roda
 * layouts sob pedido e grava o que mudou (em lote, com nova tentativa).
 */
export function useTopologyLayout(options: Options) {
  const store = useTopologyStore()
  const ready = ref(false)
  const computing = ref<LayoutKind | null>(null)
  const dragging = ref(false)
  const saveError = ref<string | null>(null)
  /** Nós colocados automaticamente e ainda não ajustados à mão. */
  const autoPlaced = ref<Set<number>>(new Set())
  let reloadAfterDrag = false

  const saver = createLayoutSaver({
    save: (nodes) => store.patchLayout(nodes),
    onError: (err) => {
      saveError.value = describe(err)
    },
    onSaved: () => {
      saveError.value = null
    },
  })

  function commit(positions: PositionMap, auto: boolean) {
    if (Object.keys(positions).length === 0) return
    store.mergeLayout(positions)
    const next = new Set(autoPlaced.value)
    for (const id of Object.keys(positions)) {
      if (auto) next.add(Number(id))
      else next.delete(Number(id))
    }
    autoPlaced.value = next
    if (options.canPersist()) saver.queue(positions)
  }

  /** Posiciona os nós sem posição, sem mexer nos outros. */
  function placeMissing(): PositionMap {
    const placed = placeNewNodes(store.nodes, store.edges, store.layout)
    commit(placed, true)
    return placed
  }

  /** Primeira carga (ou troca de site): mapa sem nada salvo ganha o hierárquico. */
  async function initialize() {
    ready.value = false
    autoPlaced.value = new Set()
    await store.fetchLayout()
    const known = store.nodes.filter((node) => store.layout[node.id])
    if (known.length === 0 && store.nodes.length > 0) {
      computing.value = 'hierarchical'
      let positions: PositionMap
      try {
        positions = await computeLayout('hierarchical', store.nodes, store.edges)
      } catch {
        positions = gridLayout(store.nodes)
      } finally {
        computing.value = null
      }
      commit(positions, true)
    } else {
      placeMissing()
    }
    ready.value = true
  }

  /** "Reorganizar tudo" — a página já confirmou com o usuário. */
  async function runLayout(kind: LayoutKind) {
    if (store.nodes.length === 0) return
    computing.value = kind
    try {
      const positions = await computeLayout(kind, store.nodes, store.edges)
      options.canvas()?.applyPositions(positions, true, true)
      commit(positions, false)
      autoPlaced.value = new Set()
      await saver.flush()
    } finally {
      computing.value = null
    }
  }

  /** "Organizar só os novos": recoloca os nós que chegaram sozinhos. */
  function arrangeNew() {
    const ids = [...autoPlaced.value].filter((id) => store.nodesById.has(id))
    if (ids.length === 0) return
    const base: PositionMap = { ...store.layout }
    for (const id of ids) delete base[id]
    const placed = placeNewNodes(store.nodes, store.edges, base)
    options.canvas()?.applyPositions(placed, true)
    commit(placed, true)
  }

  function onUserMoved(positions: PositionMap) {
    commit(positions, false)
  }

  async function reloadRemote() {
    const before = { ...store.layout }
    await store.fetchLayout()
    // Arranjo local de quem não grava continua valendo onde o servidor não diz nada.
    const localOnly: PositionMap = {}
    for (const [id, pos] of Object.entries(before)) {
      if (!store.layout[Number(id)]) localOnly[Number(id)] = pos
    }
    store.mergeLayout(localOnly)
    const changed: PositionMap = {}
    for (const [id, pos] of Object.entries(store.layout)) {
      const old = before[Number(id)]
      if (!old || Math.abs(old.x - pos.x) > 0.5 || Math.abs(old.y - pos.y) > 0.5) {
        changed[Number(id)] = pos
      }
    }
    options.canvas()?.applyPositions(changed, true)
  }

  function setDragging(value: boolean) {
    dragging.value = value
    if (!value && reloadAfterDrag && saver.pendingCount === 0) {
      reloadAfterDrag = false
      void reloadRemote()
    }
  }

  // Outra aba salvou: relê — mas nunca no meio de um arraste ou envio.
  watch(
    () => store.remoteLayoutRevision,
    () => {
      if (!ready.value) return
      if (dragging.value || saver.pendingCount > 0 || saver.saving) {
        reloadAfterDrag = true
        return
      }
      void reloadRemote()
    }
  )

  // O grafo foi recarregado (SSE ou ação): só os nós novos ganham lugar.
  watch(
    () => store.revision,
    () => {
      if (ready.value) placeMissing()
    }
  )

  onBeforeUnmount(() => {
    void saver.flush()
  })

  return {
    ready,
    computing,
    dragging,
    saveError,
    autoPlaced,
    initialize,
    runLayout,
    arrangeNew,
    onUserMoved,
    setDragging,
    retrySave: () => saver.flush(),
    flush: () => saver.flush(),
  }
}

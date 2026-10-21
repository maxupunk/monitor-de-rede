import { computed, nextTick, provide, reactive, ref, type InjectionKey, type Ref } from 'vue'
import { useRouter } from 'vue-router'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import { useLinkTestRequestsStore } from '@/stores/linkTestRequests'
import {
  useTopologyStore,
  type TopologyEdge,
  type TopologyNode,
  type TopologyPluginAction,
} from '@/stores/topology'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import type { TopologyHost } from '@/utils/topology/inspectorRegistry'
import { isUnmanagedSwitch, isVirtualEdge } from '@/utils/topology/presentation'
import { TOPOLOGY_DIALOGS, type TopologyDialogs } from './useTopologyDialogs'

export interface Selection {
  kind: 'node' | 'edge'
  id: number
}

export interface FeedbackAction {
  label: string
  run: () => void
}

export type FeedbackColor = 'success' | 'error' | 'info' | 'warning'

export interface TopologyFeedbackState {
  show: boolean
  text: string
  color: FeedbackColor
  action: FeedbackAction | null
}

/** Aviso atual do mapa — a página fornece, `TopologyFeedback` exibe. */
export const TOPOLOGY_FEEDBACK: InjectionKey<TopologyFeedbackState> = Symbol('topologyFeedback')

interface HostDeps {
  selection: Ref<Selection | null>
  editMode: () => boolean
  cableSource: Ref<number | null>
  dialogs: TopologyDialogs
  canvas: () => { centerOn(id: number): void } | null
  /** Enquadra (agora ou assim que o canvas ficar pronto). */
  fit: (deviceIds?: number[]) => void
  /** Escolhe a aba do inspetor para o item selecionado. */
  openTab: (tabId: string) => void
  runPlugin: (node: TopologyNode, item: TopologyPluginAction) => void
}

/**
 * Implementa o `TopologyHost` que as ações (nativas e de extensões) usam:
 * abre diálogos, confirma o que é destrutivo e mostra o resultado.
 */
export function useTopologyHost(deps: HostDeps) {
  const store = useTopologyStore()
  const auth = useAuthStore()
  const overlay = useTopologyOverlayStore()
  const linkTests = useLinkTestRequestsStore()
  const router = useRouter()

  const feedback = reactive<TopologyFeedbackState>({
    show: false,
    text: '',
    color: 'success',
    action: null,
  })
  provide(TOPOLOGY_FEEDBACK, feedback)
  provide(TOPOLOGY_DIALOGS, deps.dialogs)
  const busy = ref(false)

  function notify(
    text: string,
    color: FeedbackColor = 'success',
    action: FeedbackAction | null = null
  ) {
    Object.assign(feedback, { text, color, action, show: true })
  }

  const systemNodeId = computed(() => store.nodes.find((node) => node.isSystem)?.id ?? null)

  async function guarded(task: () => Promise<boolean>, ok: string, fallback: string) {
    busy.value = true
    try {
      if (await task()) notify(ok)
      else notify(store.actionError ?? fallback, 'error')
    } finally {
      busy.value = false
    }
  }

  const host: TopologyHost = {
    get canWrite() {
      return auth.canWrite
    },
    get editMode() {
      return deps.editMode()
    },
    get selectedNodeId() {
      const current = deps.selection.value
      return current?.kind === 'node' ? current.id : null
    },
    get systemNodeId() {
      return systemNodeId.value
    },
    nodeById(deviceId) {
      return store.nodesById.get(deviceId)
    },
    openDevice(deviceId) {
      void router.push(`/devices/${deviceId}`)
    },
    editNode(node) {
      if (isUnmanagedSwitch(node)) {
        deps.dialogs.openSwitch(node)
        return
      }
      void deps.dialogs.openDevice(node.id).then((found) => {
        if (!found) notify('Não foi possível carregar o cadastro do dispositivo.', 'error')
      })
    },
    async deleteNode(node) {
      const ok = await confirm({
        title: isUnmanagedSwitch(node) ? 'Excluir switch' : 'Excluir dispositivo',
        message: `Remover "${node.name}" do sistema? Os enlaces e portas dele também serão excluídos.`,
        confirmText: 'Excluir',
        confirmColor: 'error',
      })
      if (!ok) return
      await guarded(
        async () => {
          const done = await store.deleteDevice(node.id)
          if (done && deps.selection.value?.id === node.id) deps.selection.value = null
          return done
        },
        `"${node.name}" excluído.`,
        'Não foi possível excluir o dispositivo.'
      )
    },
    connectFrom(sourceId, targetId) {
      deps.dialogs.openNewLink(sourceId, targetId)
    },
    startCable(sourceId) {
      deps.cableSource.value = sourceId
    },
    editEdge(edge) {
      deps.dialogs.openEditLink(edge)
    },
    async deleteEdge(edge: TopologyEdge) {
      const virtual = isVirtualEdge(edge)
      const ok = await confirm({
        title: virtual ? 'Desvincular hierarquia' : 'Remover enlace',
        message: virtual
          ? `"${edge.targetDeviceName ?? 'O dispositivo'}" deixa de estar atrás de "${edge.sourceDeviceName ?? 'seu pai'}".`
          : `Remover o enlace entre "${edge.sourceDeviceName ?? 'origem'}" e "${edge.targetDeviceName ?? 'destino'}"? Se ele foi descoberto, pode voltar na próxima descoberta.`,
        confirmText: virtual ? 'Desvincular' : 'Remover',
        confirmColor: 'error',
      })
      if (!ok) return
      await guarded(
        async () => {
          const done = await store.deleteLink(edge.id)
          if (done && deps.selection.value?.kind === 'edge') deps.selection.value = null
          return done
        },
        virtual ? 'Hierarquia desvinculada.' : 'Enlace removido.',
        'Não foi possível remover o enlace.'
      )
    },
    async confirmEdge(edge) {
      await guarded(
        () => store.confirmLink(edge),
        'Enlace confirmado.',
        'Não foi possível confirmar o enlace.'
      )
    },
    focusNode(deviceId) {
      deps.selection.value = { kind: 'node', id: deviceId }
      // Depois do painel abrir — o centro do mapa muda com ele.
      void nextTick(() => deps.canvas()?.centerOn(deviceId))
    },
    selectEdge(edgeId) {
      deps.selection.value = { kind: 'edge', id: edgeId }
    },
    async highlightPath(fromId, toId) {
      const path = await store.fetchPath(fromId, toId)
      if (!path || path.deviceIds.length === 0) {
        notify(store.actionError ?? 'Nenhum caminho conhecido entre os dois pontos.', 'warning')
        return
      }
      overlay.setPathHighlight({ nodeIds: path.deviceIds, edgeIds: path.edgeIds })
      deps.fit(path.deviceIds)
      notify(`Caminho com ${path.edgeIds.length} enlace(s) destacado.`, 'info', {
        label: 'Limpar',
        run: () => overlay.setPathHighlight(null),
      })
    },
    requestLinkTest(request) {
      // A camada do teste de enlace assume o pedido e abre o assistente.
      linkTests.requestLinkTest(request)
    },
    openInspectorTab(tabId) {
      deps.openTab(tabId)
    },
    runPluginAction(node, action) {
      deps.runPlugin(node, action)
    },
    addUnmanagedSwitch() {
      deps.dialogs.openSwitch(null)
    },
    async discoverLinks() {
      await guarded(
        () => store.recalculateTopology(),
        'Descoberta concluída: enlaces atualizados.',
        'Não foi possível descobrir os enlaces.'
      )
    },
    addLink() {
      deps.dialogs.openNewLink()
    },
    addDevice() {
      deps.dialogs.openNewDevice()
    },
    fit() {
      deps.fit()
    },
  }

  return { host, feedback, notify, busy, systemNodeId }
}

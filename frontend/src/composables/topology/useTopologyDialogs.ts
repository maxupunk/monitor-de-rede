import { reactive, type InjectionKey } from 'vue'
import { useDevicesStore, type Device } from '@/stores/devices'
import type { TopologyEdge, TopologyNode } from '@/stores/topology'
import { edgeEndpoints, isVirtualEdge } from '@/utils/topology/presentation'

/** Estado dos diálogos de edição do mapa (enlace, switch, dispositivo). */
export function useTopologyDialogs() {
  const devicesStore = useDevicesStore()

  const link = reactive({
    open: false,
    editingId: null as number | null,
    sourceId: null as number | null,
    targetId: null as number | null,
    sourceInterfaceId: null as number | null,
    targetInterfaceId: null as number | null,
    linkType: 'manual' as string,
  })

  const unmanagedSwitch = reactive({
    open: false,
    toEdit: null as TopologyNode | null,
  })

  const device = reactive({
    open: false,
    toEdit: null as Device | null,
  })

  function openNewLink(sourceId?: number | null, targetId?: number | null) {
    Object.assign(link, {
      editingId: null,
      sourceId: sourceId ?? null,
      targetId: targetId ?? null,
      sourceInterfaceId: null,
      targetInterfaceId: null,
      linkType: 'manual',
      open: true,
    })
  }

  /** Edita um enlace real; a aresta de hierarquia vira um enlace físico novo. */
  function openEditLink(edge: TopologyEdge) {
    const { source, target } = edgeEndpoints(edge)
    Object.assign(link, {
      editingId: isVirtualEdge(edge) ? null : edge.id,
      sourceId: source,
      targetId: target,
      sourceInterfaceId: edge.sourceInterfaceId ?? null,
      targetInterfaceId: edge.targetInterfaceId ?? null,
      linkType: edge.linkType || 'manual',
      open: true,
    })
  }

  function openSwitch(node: TopologyNode | null = null) {
    unmanagedSwitch.toEdit = node
    unmanagedSwitch.open = true
  }

  /** Cadastro de um dispositivo novo sobre o mapa (o mesmo formulário da tela). */
  function openNewDevice() {
    device.toEdit = null
    device.open = true
  }

  async function openDevice(deviceId: number) {
    let found = devicesStore.devices.find((item) => item.id === deviceId)
    if (!found) {
      await devicesStore.fetchDevices()
      found = devicesStore.devices.find((item) => item.id === deviceId)
    }
    device.toEdit = found ?? null
    device.open = Boolean(found)
    return Boolean(found)
  }

  return {
    link,
    unmanagedSwitch,
    device,
    openNewLink,
    openEditLink,
    openSwitch,
    openDevice,
    openNewDevice,
  }
}

export type TopologyDialogs = ReturnType<typeof useTopologyDialogs>

/** Estado compartilhado entre a página (que abre) e `TopologyDialogs` (que exibe). */
export const TOPOLOGY_DIALOGS: InjectionKey<TopologyDialogs> = Symbol('topologyDialogs')

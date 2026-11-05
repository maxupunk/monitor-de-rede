import type { SseBus, SseHandler, SseRoute, SseRoutes } from '@/stores/events'
import { useTopologyStore } from '@/stores/topology'
import { useProbesStore } from '@/stores/probes'
import { useDiscoveryStore } from '@/stores/discovery'
import { usePluginsStore } from '@/stores/plugins'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { useLinkTestsStore } from '@/stores/linkTests'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { isWifiDeviceSnapshot, isWifiWatchedSnapshot } from '@/utils/wifiPresentation'

/** Estado ao vivo (snapshot por etapa, amostra por segundo): fora do feed. */
const live = (handle: SseHandler): SseRoute => ({ ephemeral: true, handle })

function refreshDiscovery(bus: SseBus) {
  const discoveryStore = useDiscoveryStore()
  bus.scheduleRefresh('discovery', () => {
    discoveryStore.fetchDiscoveryRuns()
    discoveryStore.fetchConflicts()
  })
}

/** Rede: topologia, descoberta, sondas, plugins, testes de enlace e Wi-Fi. */
export const networkSse: SseRoutes = {
  'monitor:result': (data) => useTopologyStore().applyRealtimeHealth(data),
  'device:status': (data) => useTopologyStore().applyRealtimeStatus(data),
  'device:updated': (data) => useTopologyStore().applyRealtimeStatus(data),
  'interface:traffic': live((data) => useTopologyStore().applyRealtimeTraffic(data)),
  'interface:status_change': (data, payload) =>
    useTopologyStore().applyInterfaceChange(payload.type, data),
  'interface:speed_change': (data, payload) =>
    useTopologyStore().applyInterfaceChange(payload.type, data),
  'interface:speed_downgrade': (data, payload) =>
    useTopologyStore().applyInterfaceChange(payload.type, data),

  // Novos enlaces exigem recarregar o grafo: não dá para deduzir a geometria
  // do mapa a partir do evento. Recarrega em background (sem spinner, no site
  // em exibição) e só para quem já abriu o mapa.
  'topology:updated': (_data, _payload, bus) => {
    const topologyStore = useTopologyStore()
    bus.scheduleRefresh('topology', () => topologyStore.refreshIfLoaded())
  },
  'topology:layout_updated': (data) => useTopologyStore().applyLayoutUpdated(data),

  'probe:status': (data) => useProbesStore().applyRealtimeStatus(data),

  // Estado inteiro da varredura ao vivo — e o snapshot de quem conecta.
  'discovery:scan': live((data) => useDiscoveryStore().applyScanSnapshot(data)),
  'discovery:started': (_data, _payload, bus) => refreshDiscovery(bus),
  'discovery:completed': (_data, _payload, bus) => refreshDiscovery(bus),
  'discovery:cancelled': (_data, _payload, bus) => refreshDiscovery(bus),
  'discovery:failed': (_data, _payload, bus) => refreshDiscovery(bus),
  'discovery:conflicts_detected': (data) => {
    const discoveryStore = useDiscoveryStore()
    if (Array.isArray(data.conflicts)) {
      discoveryStore.conflicts = data.conflicts as never
    } else {
      discoveryStore.fetchConflicts()
    }
  },

  'plugin:run_started': (data) => usePluginsStore().applyRunStarted(data),
  'plugin:run_output': (data) => {
    usePluginsStore().applyRunOutput(data)
    usePluginAppsStore().applyRunOutput(data)
  },
  'plugin:run_finished': (data) => usePluginsStore().applyRunFinished(data),
  'plugin:run_progress': (data) => usePluginsStore().applyRunProgress(data),
  'plugin:run_step': (data) => usePluginAppsStore().applyRunOutput(data),
  'plugin:approval_required': (data) => usePluginsStore().applyApprovalRequired(data),
  'plugin:approval_resolved': (data) => usePluginsStore().applyApprovalResolved(data),
  'plugin:batch_updated': (data) => usePluginAppsStore().applyBatchUpdated(data),
  'plugin:apps_changed': () => void usePluginAppsStore().fetchApps(),

  // O teste de enlace publica um snapshot por etapa e uma amostra por segundo.
  'link_test:updated': live((data) => useLinkTestsStore().applyUpdated(data)),
  'link_test:sample': live((data) => useLinkTestsStore().applySample(data)),
  'link_test:health': live((data) => useLinkTestsStore().applyHealth(data)),
  'link_test_schedule:updated': (data) => useLinkTestsStore().applyScheduleUpdated(data),

  // Clientes Wi-Fi: estado ao vivo de cada roteador, não notícia.
  'wifi:snapshot': live((data) => {
    if (isWifiDeviceSnapshot(data)) useWifiClientsStore().applySnapshot(data)
  }),
  'wifi:watched': live((data) => {
    if (isWifiWatchedSnapshot(data)) useWifiClientsStore().applyWatched(data)
  }),
}

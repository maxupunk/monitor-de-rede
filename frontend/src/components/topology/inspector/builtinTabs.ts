/** Abas nativas do inspetor, registradas no mesmo registro das extensões. */
import { registerInspectorTab } from '@/utils/topology/inspectorRegistry'
import NodeOverviewTab from './NodeOverviewTab.vue'
import NodeLinksTab from './NodeLinksTab.vue'
import EdgeOverviewTab from './EdgeOverviewTab.vue'
import ActionsTab from './ActionsTab.vue'

let registered = false

export function registerBuiltinInspectorTabs() {
  if (registered) return
  registered = true
  registerInspectorTab({
    id: 'node.overview',
    title: 'Visão geral',
    icon: 'mdi-information-outline',
    appliesTo: ['node'],
    order: 10,
    component: NodeOverviewTab,
  })
  registerInspectorTab({
    id: 'node.links',
    title: 'Enlaces',
    icon: 'mdi-lan',
    appliesTo: ['node'],
    order: 20,
    component: NodeLinksTab,
  })
  registerInspectorTab({
    id: 'edge.overview',
    title: 'Visão geral',
    icon: 'mdi-information-outline',
    appliesTo: ['edge'],
    order: 10,
    component: EdgeOverviewTab,
  })
  registerInspectorTab({
    id: 'actions',
    title: 'Ações',
    icon: 'mdi-lightning-bolt',
    appliesTo: ['node', 'edge'],
    order: 90,
    component: ActionsTab,
  })
}

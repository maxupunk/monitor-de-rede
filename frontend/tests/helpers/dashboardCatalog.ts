import type { Area } from '@/bindings/Area'
import type { DashboardCatalog } from '@/bindings/DashboardCatalog'
import type { DashboardWidgetSize } from '@/bindings/DashboardWidgetSize'
import type { DashboardWidgetType } from '@/bindings/DashboardWidgetType'
import type { WidgetCatalogEntry } from '@/bindings/WidgetCatalogEntry'

/** Uma entrada do catálogo com o essencial — o resto não importa aos testes. */
function entry(
  type: DashboardWidgetType,
  area: Area,
  defaultSize: DashboardWidgetSize,
  allowMultiple = false
): WidgetCatalogEntry {
  return {
    type,
    title: `Card ${type}`,
    description: `Descrição de ${type}`,
    category: 'charts',
    icon: 'mdi-view-dashboard-outline',
    allowMultiple,
    defaultSize,
    configKeys: [],
    compatibleResources: allowMultiple ? ['numeric'] : [],
    area,
  }
}

/** O formato do `GET /api/dashboard/catalog`, como o backend o monta. */
export const CATALOG: DashboardCatalog = {
  widgets: [
    entry('stat_cards', 'core', 'full'),
    entry('health_gauge', 'core', 'half'),
    entry('active_alerts', 'core', 'half'),
    entry('latency_time_series', 'core', 'half'),
    entry('event_distribution', 'core', 'half'),
    entry('unstable_targets', 'core', 'half'),
    entry('events_feed', 'core', 'half'),
    entry('network_monitors', 'core', 'full'),
    entry('docker_summary', 'servers', 'full'),
    entry('dns_latency', 'network', 'half', true),
    entry('saas_services', 'network', 'full'),
    entry('saas_heatmap', 'network', 'full', true),
    entry('cpu_usage', 'core', 'half', true),
    entry('ether_bandwidth', 'network', 'half', true),
  ],
  defaultLayout: [
    { type: 'stat_cards', size: 'full', visible: true },
    { type: 'health_gauge', size: 'half', visible: true },
    { type: 'active_alerts', size: 'half', visible: true },
    { type: 'latency_time_series', size: 'half', visible: true },
    { type: 'event_distribution', size: 'half', visible: true },
    { type: 'unstable_targets', size: 'half', visible: true },
    { type: 'events_feed', size: 'half', visible: true },
    { type: 'network_monitors', size: 'full', visible: true },
    { type: 'docker_summary', size: 'full', visible: true },
    { type: 'dns_latency', size: 'half', visible: false },
    { type: 'saas_services', size: 'full', visible: false },
    { type: 'saas_heatmap', size: 'full', visible: false },
  ],
}

/** Snapshot de `areas:updated` com as áreas opcionais no estado pedido. */
export function areasSnapshot(enabled: Partial<Record<Area, boolean>>) {
  const areas: Area[] = ['core', 'network', 'servers', 'vpn']
  return {
    areas: areas.map((area) => ({
      area,
      title: area,
      summary: `Resumo de ${area}`,
      icon: 'mdi-circle',
      enabled: area === 'core' || (enabled[area] ?? true),
      optional: area !== 'core',
      evidence: null,
    })),
  }
}

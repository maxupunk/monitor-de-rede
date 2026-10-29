import { defineAsyncComponent, type Component } from 'vue'
import type { WidgetConfig } from '@/stores/dashboard'

/**
 * Registro dos widgets do Dashboard: tipo → componente e as props que ele
 * recebe. A página não conhece widget nenhum — ela pergunta aqui. Widget
 * novo entra com uma linha, sem tocar na página (aberto para extensão,
 * fechado para modificação).
 *
 * Cada widget cuida sozinho dos dados, diálogos e ações que são dele; os
 * configuráveis recebem a própria configuração (`widget`). Os componentes
 * carregam sob demanda: widget desligado não pesa no carregamento do painel.
 */
interface WidgetEntry {
  component: Component
  props?: (widget: WidgetConfig) => Record<string, unknown>
}

const lazy = (loader: () => Promise<{ default: Component }>) => defineAsyncComponent(loader)
const withConfig = (widget: WidgetConfig) => ({ widget })

const WIDGETS: Record<string, WidgetEntry> = {
  stat_cards: { component: lazy(() => import('@/components/dashboard/StatCardsWidget.vue')) },
  health_gauge: { component: lazy(() => import('@/components/widgets/GaugeHealthWidget.vue')) },
  active_alerts: { component: lazy(() => import('@/components/dashboard/ActiveAlertsWidget.vue')) },
  latency_time_series: {
    component: lazy(() => import('@/components/widgets/LatencyTimeSeriesWidget.vue')),
    props: withConfig,
  },
  event_distribution: {
    component: lazy(() => import('@/components/widgets/EventDistributionWidget.vue')),
  },
  unstable_targets: {
    component: lazy(() => import('@/components/widgets/UnstableTargetsWidget.vue')),
  },
  events_feed: { component: lazy(() => import('@/components/dashboard/EventsFeedWidget.vue')) },
  network_monitors: {
    component: lazy(() => import('@/components/dashboard/NetworkMonitorsWidget.vue')),
  },
  docker_summary: { component: lazy(() => import('@/components/dashboard/DockerSummaryCard.vue')) },
  dns_latency: { component: lazy(() => import('@/components/DnsLatencyCard.vue')) },
  saas_services: { component: lazy(() => import('@/components/widgets/SaasServicesWidget.vue')) },
  saas_heatmap: {
    component: lazy(() => import('@/components/widgets/SaasLatencyHeatmapWidget.vue')),
    props: (widget) => ({
      monitorId: typeof widget.config?.monitorId === 'number' ? widget.config.monitorId : undefined,
    }),
  },
  ether_bandwidth: {
    component: lazy(() => import('@/components/widgets/EtherBandwidthWidget.vue')),
    props: withConfig,
  },
  bandwidth_vs_latency: {
    component: lazy(() => import('@/components/widgets/BandwidthVsLatencyWidget.vue')),
    props: withConfig,
  },
  cpu_usage: {
    component: lazy(() => import('@/components/widgets/CpuUsageWidget.vue')),
    props: withConfig,
  },
  ram_usage: {
    component: lazy(() => import('@/components/widgets/RamUsageWidget.vue')),
    props: withConfig,
  },
  binary_status: {
    component: lazy(() => import('@/components/widgets/BinaryStatusWidget.vue')),
    props: withConfig,
  },
}

/** Tipo efetivo do widget: layouts antigos guardavam só o `id`. */
export function widgetTypeOf(widget: Pick<WidgetConfig, 'id' | 'type'>): string {
  return widget.type || widget.id
}

/** Componente do widget, ou `null` para um tipo que esta versão não conhece. */
export function widgetComponentOf(widget: Pick<WidgetConfig, 'id' | 'type'>): Component | null {
  return WIDGETS[widgetTypeOf(widget)]?.component ?? null
}

/** Props que o componente do widget recebe. */
export function widgetPropsOf(widget: WidgetConfig): Record<string, unknown> {
  return WIDGETS[widgetTypeOf(widget)]?.props?.(widget) ?? {}
}

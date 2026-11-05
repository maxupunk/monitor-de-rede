import { defineAsyncComponent, type Component } from 'vue'
import type { RouteRecordRaw } from 'vue-router'
import type { Area } from '@/bindings/Area'
import type { DashboardWidgetType } from '@/bindings/DashboardWidgetType'
import type { NavEntry } from '@/navigation'
import type { SseRoutes } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'

/** Um tipo de card do dashboard: o componente e as props que ele recebe. */
export interface WidgetEntry {
  component: Component
  props?: (widget: WidgetConfig) => Record<string, unknown>
}

/** Card carregado sob demanda: widget desligado não pesa no painel. */
export const lazyWidget = (loader: () => Promise<{ default: Component }>) =>
  defineAsyncComponent(loader)

/** Props dos cards configuráveis: a própria configuração. */
export const withWidgetConfig = (widget: WidgetConfig) => ({ widget })

/**
 * O que uma área do produto (ADR 017) traz para a SPA. Ligar, desligar,
 * acrescentar ou retirar uma área é mexer no módulo dela — o centro (router,
 * menu, barramento SSE, dashboard) só compõe o que os módulos declaram.
 */
export interface AreaDefinition {
  id: Area
  /** Rotas filhas do layout principal. Recebem `meta.area` sozinhas. */
  routes?: RouteRecordRaw[]
  /** Entradas do menu; a seção e a ordem dentro dela vêm de cada entrada. */
  nav?: Omit<NavEntry, 'area'>[]
  /** Componentes dos cards do dashboard que a área desenha. */
  widgets?: Partial<Record<DashboardWidgetType, WidgetEntry>>
  /** Rotas do barramento SSE; só ficam registradas com a área ligada. */
  sse?: SseRoutes
}

export interface AreaModule {
  id: Area
  routes: RouteRecordRaw[]
  nav: NavEntry[]
  widgets: Partial<Record<DashboardWidgetType, WidgetEntry>>
  sse: SseRoutes
}

function withArea(route: RouteRecordRaw, area: Area): RouteRecordRaw {
  const children = route.children?.map((child) => withArea(child, area))
  return {
    ...route,
    meta: { ...route.meta, area },
    ...(children ? { children } : {}),
  } as RouteRecordRaw
}

/** Declara uma área: carimba a área nas rotas e no menu. */
export function defineArea(definition: AreaDefinition): AreaModule {
  const { id } = definition
  return {
    id,
    routes: (definition.routes ?? []).map((route) => withArea(route, id)),
    nav: (definition.nav ?? []).map((entry) => ({ ...entry, area: id })),
    widgets: definition.widgets ?? {},
    sse: definition.sse ?? {},
  }
}

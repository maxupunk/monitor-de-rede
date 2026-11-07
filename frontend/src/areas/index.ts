import type { RouteRecordRaw } from 'vue-router'
import type { Area } from '@/bindings/Area'
import type { DashboardWidgetType } from '@/bindings/DashboardWidgetType'
import type { NavEntry } from '@/navigation'
import type { AreaModule, WidgetEntry } from './defineArea'
import { coreArea } from './core'
import { networkArea } from './network'
import { serversArea } from './servers'
import { vpnArea } from './vpn'

export type { AreaModule, WidgetEntry } from './defineArea'

/**
 * Registro das áreas da SPA (ADR 017). Área nova entra aqui com uma linha;
 * o resto vem do módulo dela.
 */
export const AREAS: readonly AreaModule[] = [coreArea, networkArea, serversArea, vpnArea]

export function areaModule(id: Area): AreaModule | undefined {
  return AREAS.find((area) => area.id === id)
}

/** Rotas filhas do layout principal, de todas as áreas. */
export const AREA_ROUTES: RouteRecordRaw[] = AREAS.flatMap((area) => area.routes)

/** Entradas do menu, de todas as áreas — `navigation.ts` as organiza. */
export const NAV_ENTRIES: NavEntry[] = AREAS.flatMap((area) => area.nav)

/** Componentes dos cards do dashboard, de todas as áreas. */
export const AREA_WIDGETS: Partial<Record<DashboardWidgetType, WidgetEntry>> = Object.assign(
  {},
  ...AREAS.map((area) => area.widgets)
)

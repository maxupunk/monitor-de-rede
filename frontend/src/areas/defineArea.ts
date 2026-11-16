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

/** Componente carregado sob demanda: card ou cartão de área desligada não pesa. */
export const lazyComponent = (loader: () => Promise<{ default: Component }>) =>
  defineAsyncComponent(loader)

/** Props dos cards configuráveis: a própria configuração. */
export const withWidgetConfig = (widget: WidgetConfig) => ({ widget })

/** Uma aba da página de Configurações. Aba sem nenhum cartão visível some. */
export interface SettingsTabSpec {
  /** Vai na URL (`?tab=`): é o link direto da aba. */
  value: string
  title: string
  icon: string
  order: number
}

/** Um cartão de Configurações, numa aba de qualquer área (inclusive de outra). */
export interface SettingsCardSpec {
  tab: string
  order: number
  /** Cartão autossuficiente: carrega e salva sozinho; avisa com `saved`. */
  component: Component
  /** Largura no grid a partir de `md`/`lg`; sem valor, a linha inteira. */
  cols?: { md?: number; lg?: number }
}

/**
 * Um caminho pelo qual um servidor remoto alcança esta central: rede local,
 * VPN, internet. Quem conecta um servidor escolhe um deles.
 */
export interface ServerConnectPathSpec {
  id: string
  order: number
  title: string
  /** Quando escolher este caminho — a frase que orienta a decisão. */
  description: string
  icon: string
  /** O caminho indicado para um servidor fora da rede da central. */
  recommended?: boolean
  /**
   * Tipos de "Endereços deste servidor" que servem ao caminho, em ordem de
   * preferência: o comando de instalação já sai com o primeiro que existir.
   */
  addressKinds?: string[]
  /**
   * Fluxo próprio do caminho — um diálogo autossuficiente que recebe
   * `v-model`, `serverName` e `agentId` e avisa com `done`. Sem ele, vale o
   * comando de instalação com o endereço escolhido.
   */
  component?: Component
  /** O fluxo também leva para este caminho um servidor já conectado. */
  moveExisting?: { label: string; icon: string }
}

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
  /** Abas e cartões de Configurações; somem com a área desligada. */
  settingsTabs?: SettingsTabSpec[]
  settingsCards?: SettingsCardSpec[]
  /** Caminhos para conectar um servidor remoto; somem com a área desligada. */
  serverConnectPaths?: ServerConnectPathSpec[]
}

export interface AreaModule {
  id: Area
  routes: RouteRecordRaw[]
  nav: NavEntry[]
  widgets: Partial<Record<DashboardWidgetType, WidgetEntry>>
  sse: SseRoutes
  settingsTabs: SettingsTabSpec[]
  settingsCards: SettingsCardSpec[]
  serverConnectPaths: ServerConnectPathSpec[]
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
    settingsTabs: definition.settingsTabs ?? [],
    settingsCards: definition.settingsCards ?? [],
    serverConnectPaths: definition.serverConnectPaths ?? [],
  }
}

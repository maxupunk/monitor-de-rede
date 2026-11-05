import type { Component } from 'vue'
import type { WidgetConfig } from '@/stores/dashboard'
import { AREA_WIDGETS, type WidgetEntry } from '@/areas'

/**
 * Registro dos widgets do Dashboard: tipo → componente e as props que ele
 * recebe. A página não conhece widget nenhum — ela pergunta aqui, e aqui só
 * se compõe o que cada área declara (`src/areas/<área>`, chave `widgets`).
 * Widget novo entra no módulo da área, sem tocar na página (aberto para
 * extensão, fechado para modificação).
 *
 * Cada widget cuida sozinho dos dados, diálogos e ações que são dele; os
 * configuráveis recebem a própria configuração (`widget`). Os componentes
 * carregam sob demanda: widget desligado não pesa no carregamento do painel.
 */
const WIDGETS: Record<string, WidgetEntry | undefined> = AREA_WIDGETS

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

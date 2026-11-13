import type { AiScreenContext } from '@/bindings/AiScreenContext'

/**
 * Como uma rota se apresenta ao chat (`meta.aiScreen`, declarado pela área
 * dona da rota). O `id` é o do catálogo do backend
 * (`crates/assistant/src/screens.rs`), que diz à IA o que a tela mostra.
 */
export interface AiScreenMeta {
  id: string
  /** Rótulo do chip no chat. */
  label: string
  icon: string
  /** O `:id` da rota é um equipamento ou um monitor: vai junto como contexto. */
  entity?: 'device' | 'monitor'
}

/** O pedaço da rota que importa (o `useRoute()` serve). */
export interface ScreenRoute {
  meta: { aiScreen?: AiScreenMeta }
  params: Record<string, string | string[] | undefined>
}

/** A tela aberta, pronta para o chip e para a pergunta. */
export interface CurrentScreen {
  meta: AiScreenMeta
  /** Identifica a tela e o recurso dela: dispensar o chip vale até ela mudar. */
  key: string
  entityId: number | null
}

function entityId(route: ScreenRoute): number | null {
  const raw = route.params.id
  const id = Number(Array.isArray(raw) ? raw[0] : raw)
  return Number.isInteger(id) && id > 0 ? id : null
}

export function currentScreen(route: ScreenRoute): CurrentScreen | null {
  const meta = route.meta.aiScreen
  if (!meta) return null
  const id = meta.entity ? entityId(route) : null
  return { meta, key: id === null ? meta.id : `${meta.id}:${id}`, entityId: id }
}

/** O que a pergunta leva da tela: o id, o que está filtrado e o recurso aberto. */
export interface ScreenRequest {
  screen: AiScreenContext
  deviceId?: number
  monitorId?: number
}

export function screenRequest(screen: CurrentScreen, detail: string | null): ScreenRequest {
  const request: ScreenRequest = {
    screen: { id: screen.meta.id, detail: detail?.trim() || null },
  }
  if (screen.entityId !== null && screen.meta.entity === 'device') {
    request.deviceId = screen.entityId
  }
  if (screen.entityId !== null && screen.meta.entity === 'monitor') {
    request.monitorId = screen.entityId
  }
  return request
}

/** A tela como ela vai junto de uma pergunta: o chip na conversa e o pedido. */
export interface ScreenAttachment {
  label: string
  icon: string
  request: ScreenRequest
}

export function screenAttachment(screen: CurrentScreen, detail: string | null): ScreenAttachment {
  return {
    label: screen.meta.label,
    icon: screen.meta.icon,
    request: screenRequest(screen, detail),
  }
}

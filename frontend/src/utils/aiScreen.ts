import type { AiScreenContext } from '@/bindings/AiScreenContext'

/** Pergunta pronta oferecida quando a conversa está vazia. */
export interface AiSuggestion {
  title: string
  prompt: string
  icon: string
  color: string
}

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
  /** Perguntas sobre esta tela, oferecidas primeiro no chat vazio. */
  suggestions?: AiSuggestion[]
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
  /** A pessoa já disse de qual tela fala: não vai a tela anterior junto. */
  chosen?: boolean
}

export function screenAttachment(screen: CurrentScreen, detail: string | null): ScreenAttachment {
  return {
    label: screen.meta.label,
    icon: screen.meta.icon,
    request: screenRequest(screen, detail),
  }
}

/** As sugestões da tela primeiro, depois as gerais sem repetir, até `limit`. */
export function mergeSuggestions(
  own: AiSuggestion[],
  general: AiSuggestion[],
  limit?: number
): AiSuggestion[] {
  const seen = new Set(own.map((item) => item.prompt))
  const all = [...own, ...general.filter((item) => !seen.has(item.prompt))]
  return limit === undefined ? all : all.slice(0, limit)
}

/**
 * O anexo com a tela da pergunta anterior, quando a pessoa mudou de tela no
 * meio da conversa ("isso" pode ser de uma ou de outra).
 */
export function withPreviousScreen(
  attached: ScreenAttachment,
  messages: { role: string; screen?: { id: string } | null }[]
): ScreenAttachment {
  if (attached.chosen) return attached
  const previous = lastAskedScreen(messages)?.id
  if (!previous || previous === attached.request.screen.id) return attached
  return {
    ...attached,
    request: { ...attached.request, screen: { ...attached.request.screen, previous } },
  }
}

/** A tela de uma pergunta já feita, como ficou na conversa. */
export interface AskedScreen {
  id: string
  label: string
  icon: string
}

function lastAskedScreen<T extends { id: string }>(
  messages: { role: string; screen?: T | null }[]
): T | null {
  const last = [...messages].reverse().find((message) => message.role === 'user')
  return last?.screen ?? null
}

// "isso", "esse erro", "aquela VLAN": aponta para algo sem dizer o quê.
const VAGUE = /(?<!\p{L})(isso|isto|aquilo|ess[ea]s?|aquel[ea]s?)(?!\p{L})/iu
// "aqui", "nesta tela": aponta para a tela aberta, sem dúvida.
const HERE = /(?<!\p{L})(aqui|nesta tela|desta tela|dessa tela|nessa tela)(?!\p{L})/iu

/**
 * Quando perguntar antes de enviar "sobre qual tela?": a pessoa mudou de tela
 * desde a pergunta anterior e a nova aponta para algo vago ("e isso, é
 * grave?") sem marcar nada com `@` nem dizer "aqui". Decidir isso no campo, e
 * não deixar para o modelo, é o que garante a pergunta — modelo pequeno
 * escolhe uma das telas sozinho.
 */
export function screenToConfirm(
  attached: ScreenAttachment,
  messages: { role: string; screen?: AskedScreen | null }[],
  text: string,
  marked: boolean
): AskedScreen[] | null {
  if (marked || !VAGUE.test(text) || HERE.test(text)) return null
  const previous = lastAskedScreen(messages)
  if (!previous || previous.id === attached.request.screen.id) return null
  const current = { id: attached.request.screen.id, label: attached.label, icon: attached.icon }
  return [current, previous]
}

/** O anexo da tela escolhida na pergunta "sobre qual tela?". */
export function chooseScreen(attached: ScreenAttachment, choice: AskedScreen): ScreenAttachment {
  if (choice.id === attached.request.screen.id) return { ...attached, chosen: true }
  // A tela anterior já não está aberta: vai o que ela é, sem o filtro de agora.
  return {
    label: choice.label,
    icon: choice.icon,
    request: { screen: { id: choice.id, detail: null } },
    chosen: true,
  }
}

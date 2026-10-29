import { ref, watch, type Ref } from 'vue'
import type { WidgetConfig } from '@/stores/dashboard'

export const TIMEFRAMES = ['5m', '15m', '1h', '24h'] as const
export type Timeframe = (typeof TIMEFRAMES)[number]

/** Duração de cada janela, em milissegundos. */
export const TIMEFRAME_MS: Record<Timeframe, number> = {
  '5m': 5 * 60_000,
  '15m': 15 * 60_000,
  '1h': 3_600_000,
  '24h': 86_400_000,
}

export function isTimeframe(value: unknown): value is Timeframe {
  return typeof value === 'string' && (TIMEFRAMES as readonly string[]).includes(value)
}

/**
 * Janela de tempo de um widget. O valor inicial vem da configuração salva do
 * widget (o que o layout — ou a IA — definiu como padrão); trocar no seletor é
 * estado da tela, não muda o layout de ninguém. Se a configuração mudar (outra
 * aba, a IA), a tela acompanha.
 */
export function useWidgetTimeframe(
  widget: () => WidgetConfig | undefined,
  fallback: Timeframe = '1h'
): Ref<Timeframe> {
  const configured = () => {
    const value = widget()?.config?.timeframe
    return isTimeframe(value) ? value : fallback
  }
  const timeframe = ref<Timeframe>(configured())
  watch(configured, (value) => {
    timeframe.value = value
  })
  return timeframe
}

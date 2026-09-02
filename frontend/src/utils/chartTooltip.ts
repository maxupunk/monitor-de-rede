import type { CSSProperties } from 'vue'

export interface ChartTooltipPosition {
  x: number
  y: number
  containerWidth: number
  containerHeight: number
  maxWidth?: number
  estimatedHeight?: number
  offset?: number
  padding?: number
}

/**
 * Posiciona tooltips dentro da área visível do gráfico sem depender de uma
 * largura fixa. O lado com mais espaço é escolhido automaticamente e textos
 * longos quebram dentro do limite disponível em vez de atravessar o card.
 */
export function chartTooltipStyle({
  x,
  y,
  containerWidth,
  containerHeight,
  maxWidth = 360,
  estimatedHeight = 90,
  offset = 16,
  padding = 8,
}: ChartTooltipPosition): CSSProperties {
  const spaceLeft = Math.max(0, x - offset - padding)
  const spaceRight = Math.max(0, containerWidth - x - offset - padding)
  const placeLeft = spaceLeft > spaceRight
  const availableWidth = placeLeft ? spaceLeft : spaceRight

  const spaceAbove = Math.max(0, y - offset - padding)
  const spaceBelow = Math.max(0, containerHeight - y - offset - padding)
  const placeAbove = spaceAbove >= estimatedHeight || spaceAbove > spaceBelow
  const availableHeight = placeAbove ? spaceAbove : spaceBelow

  const horizontal: CSSProperties = placeLeft
    ? { right: `${Math.max(padding, containerWidth - x + offset)}px` }
    : { left: `${Math.max(padding, x + offset)}px` }
  const vertical: CSSProperties = placeAbove
    ? { bottom: `${Math.max(padding, containerHeight - y + offset)}px` }
    : { top: `${Math.max(padding, y + offset)}px` }

  return {
    position: 'absolute',
    ...horizontal,
    ...vertical,
    width: 'max-content',
    maxWidth: `${Math.max(0, Math.min(maxWidth, availableWidth))}px`,
    maxHeight: `${Math.max(0, availableHeight)}px`,
    whiteSpace: 'normal',
    overflowWrap: 'anywhere',
    overflowY: 'auto',
    pointerEvents: 'none',
  }
}

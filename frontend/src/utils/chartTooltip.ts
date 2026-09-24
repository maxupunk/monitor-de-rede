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
  /**
   * Largura mínima legível. Quando nenhum lado do cursor comporta, o cartão
   * deixa de seguir o cursor pelo lado e fica inteiro dentro do gráfico.
   */
  minWidth?: number
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
  minWidth = 0,
}: ChartTooltipPosition): CSSProperties {
  const spaceLeft = Math.max(0, x - offset - padding)
  const spaceRight = Math.max(0, containerWidth - x - offset - padding)
  const placeLeft = spaceLeft > spaceRight
  const availableWidth = placeLeft ? spaceLeft : spaceRight

  const spaceAbove = Math.max(0, y - offset - padding)
  const spaceBelow = Math.max(0, containerHeight - y - offset - padding)
  const placeAbove = spaceAbove >= estimatedHeight || spaceAbove > spaceBelow
  const availableHeight = placeAbove ? spaceAbove : spaceBelow

  const cramped = minWidth > 0 && availableWidth < minWidth
  const width = Math.min(maxWidth, Math.max(minWidth, containerWidth - 2 * padding))
  const horizontal: CSSProperties = cramped
    ? {
        left: `${Math.round(
          Math.min(
            Math.max(padding, x - width / 2),
            Math.max(padding, containerWidth - width - padding)
          )
        )}px`,
      }
    : placeLeft
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
    maxWidth: `${Math.max(0, cramped ? width : Math.min(maxWidth, availableWidth))}px`,
    ...(minWidth > 0 ? { minWidth: `${Math.min(minWidth, width)}px` } : {}),
    maxHeight: `${Math.max(0, availableHeight)}px`,
    whiteSpace: 'normal',
    overflowWrap: 'anywhere',
    overflowY: 'auto',
    pointerEvents: 'none',
  }
}

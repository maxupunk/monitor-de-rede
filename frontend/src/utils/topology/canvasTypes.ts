/** Contratos entre o canvas (Cytoscape) e quem o usa (página, minimapa). */
import type { TopologyPosition } from '@/stores/topology'

export type CanvasTargetKind = 'node' | 'edge' | 'background'

export interface CanvasPointerEvent {
  kind: CanvasTargetKind
  /** Id do dispositivo/enlace (ausente no fundo). */
  id?: number
  /** Posição relativa ao contêiner do canvas, em px. */
  x: number
  y: number
  /** Posição na tela (para menus). */
  clientX: number
  clientY: number
}

export interface CanvasViewport {
  zoom: number
  nodeCount: number
}

export interface MinimapNode {
  x: number
  y: number
  status: string
  system: boolean
}

export interface MinimapData {
  nodes: MinimapNode[]
  /** Caixa (modelo) que contém todos os nós. */
  bounds: { x1: number; y1: number; x2: number; y2: number }
  /** Área visível (modelo). */
  viewport: { x1: number; y1: number; x2: number; y2: number }
}

/** O que o minimapa precisa do canvas — injetado, sem acoplamento ao Cytoscape. */
export interface MinimapSource {
  getMinimapData(): MinimapData | null
  onSceneChange(listener: () => void): () => void
  panToModel(position: TopologyPosition): void
}

/** Largura do cartão da legenda aberta. */
export const TOPOLOGY_LEGEND_WIDTH = 312

/** Faixas do canvas cobertas por painéis flutuantes (px). */
export interface CanvasInsets {
  top: number
  right: number
  bottom: number
  left: number
}

export interface InsetsInput {
  /** Celular: painéis viram folhas inferiores. */
  narrow: boolean
  editMode: boolean
  legendOpen: boolean
  minimap: boolean
  /** Altura da folha inferior aberta (celular), 0 sem folha. */
  sheetHeight: number
}

/**
 * O que cada painel flutuante cobre — o enquadramento ("F", enquadrar
 * caminho) usa só a área livre, para nenhum nó ficar embaixo de um painel.
 */
export function mapInsets(input: InsetsInput): CanvasInsets {
  if (input.narrow) {
    return {
      top: input.editMode ? 104 : 0,
      right: 0,
      bottom: input.sheetHeight > 0 ? input.sheetHeight : 64,
      left: 0,
    }
  }
  return {
    // Fileira de cima: botão da legenda, avisos e a barra de edição.
    top: 44 + (input.editMode ? 60 : 0),
    // Coluna do zoom.
    right: 64,
    // Resumo embaixo; o minimapa, quando aberto, é mais alto.
    bottom: input.minimap ? 146 : 60,
    left: input.legendOpen ? TOPOLOGY_LEGEND_WIDTH + 20 : 0,
  }
}

/** Zoom máximo ao enquadrar: dois ou três nós não viram um close de 300%. */
export const FIT_MAX_ZOOM = 1.6
/** Folga em volta do que é enquadrado (px). */
const FIT_PADDING = 40
/** Abaixo disso a área livre não serve para enquadrar (px). */
const FIT_MIN_AREA = 120

/** Caixa (coordenadas do modelo) do que se quer enquadrar. */
export interface FitBox {
  x1: number
  y1: number
  x2: number
  y2: number
  w: number
  h: number
}

/**
 * Zoom e deslocamento para enquadrar `box` na área livre do canvas. Quando
 * os painéis (inspetor, teste, legenda) deixam pouca área livre, abre mão
 * primeiro da legenda (que recolhe) e, se ainda faltar espaço, usa o canvas
 * inteiro — sempre com o mesmo teto de zoom.
 */
export function fitViewport(
  box: FitBox,
  canvas: { width: number; height: number },
  insets: CanvasInsets,
  minZoom: number
): { zoom: number; pan: { x: number; y: number } } {
  const sizeOf = (inset: CanvasInsets) => ({
    width: canvas.width - inset.left - inset.right - FIT_PADDING * 2,
    height: canvas.height - inset.top - inset.bottom - FIT_PADDING * 2,
  })
  const candidates = [insets, { ...insets, left: 0 }, { top: 0, right: 0, bottom: 0, left: 0 }]
  const area =
    candidates.find((inset) => {
      const size = sizeOf(inset)
      return size.width >= FIT_MIN_AREA && size.height >= FIT_MIN_AREA
    }) ?? candidates[candidates.length - 1]
  const { width, height } = sizeOf(area)
  const zoom = Math.max(
    minZoom,
    Math.min(FIT_MAX_ZOOM, width / Math.max(1, box.w), height / Math.max(1, box.h))
  )
  return {
    zoom,
    pan: {
      x: area.left + FIT_PADDING + width / 2 - ((box.x1 + box.x2) / 2) * zoom,
      y: area.top + FIT_PADDING + height / 2 - ((box.y1 + box.y2) / 2) * zoom,
    },
  }
}

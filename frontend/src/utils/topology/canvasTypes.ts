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
  /** Largura que um painel à direita tira do mapa (teste de velocidade). */
  panelRight: number
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
    // Coluna do zoom (ou o painel do teste, mais largo).
    right: Math.max(64, input.panelRight),
    // Resumo embaixo; o minimapa, quando aberto, é mais alto.
    bottom: input.minimap ? 146 : 60,
    left: input.legendOpen ? TOPOLOGY_LEGEND_WIDTH + 20 : 0,
  }
}

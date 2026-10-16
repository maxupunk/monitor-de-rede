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

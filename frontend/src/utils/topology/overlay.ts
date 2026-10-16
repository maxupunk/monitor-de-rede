/**
 * Tradução pura da camada de sobreposição (`stores/topologyOverlay`) em
 * classes do canvas. O estilo (`style.ts`) dá a cara de cada classe.
 */
import type { EdgeOverlay, NodeOverlay, PathHighlight } from '@/stores/topologyOverlay'

export interface OverlaySnapshot {
  highlight: PathHighlight | null
  edgeOverlays: Readonly<Record<number, EdgeOverlay>>
  nodeOverlays: Readonly<Record<number, NodeOverlay>>
}

export const EMPTY_OVERLAY: OverlaySnapshot = {
  highlight: null,
  edgeOverlays: {},
  nodeOverlays: {},
}

export function edgeOverlayClasses(overlay?: EdgeOverlay): string[] {
  if (!overlay) return []
  const classes = ['ov', `ov-${overlay.state}`]
  if (overlay.state === 'running') classes.push(`flow-${overlay.flow ?? 'forward'}`)
  if (overlay.label) classes.push('ov-labeled')
  return classes
}

/** No nó, o estado vira halo; o texto (`label`) aparece no tooltip e no inspetor. */
export function nodeOverlayClasses(overlay?: NodeOverlay): string[] {
  if (!overlay) return []
  return ['ov', `ov-${overlay.state}`]
}

/** `path-highlight` para quem está no caminho, `path-dimmed` para o resto. */
export function highlightClass(
  highlight: PathHighlight | null,
  kind: 'node' | 'edge',
  id: number
): string | null {
  if (!highlight) return null
  const ids = kind === 'node' ? highlight.nodeIds : highlight.edgeIds
  return ids.includes(id) ? 'path-highlight' : 'path-dimmed'
}

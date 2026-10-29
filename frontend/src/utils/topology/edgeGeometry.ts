/**
 * O traçado de um enlace como linha quebrada, em coordenadas do modelo — o
 * arranjo dos rótulos desliza sobre ele.
 */
import type { EdgeSingular } from 'cytoscape'
import { quadraticPoints, type Point } from './labelLayout'

/** Pontas, dobras (traçado ortogonal) ou a curva amostrada. */
export function edgePath(edge: EdgeSingular): Point[] {
  const source = edge.sourceEndpoint()
  const target = edge.targetEndpoint()
  const bends = edge.segmentPoints()
  if (bends?.length) return [source, ...bends, target]
  const controls = edge.controlPoints()
  if (controls?.length === 1) return quadraticPoints(source, controls[0], target)
  return [source, ...(controls ?? []), target]
}

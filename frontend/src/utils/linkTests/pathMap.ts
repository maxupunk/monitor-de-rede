/**
 * Posições do mini mapa do teste (`LinkTestPathMap`): o caminho em linha, da
 * origem ao destino, na ordem medida. Puro.
 *
 * As posições do próprio mapa foram a primeira tentativa, mas num recorte
 * pequeno elas espremem os vizinhos próximos e a pílula com os Mbps cobre os
 * ícones (visto com a rede real). Em linha, cada enlace tem espaço para o
 * resultado, e o sentido do fluxo se lê da origem ao destino — o desenho
 * (ícones, cores, animação) continua o do mapa. Na tela estreita (celular) o
 * caminho vai em pé, de cima para baixo.
 */
import type { TopologyPosition } from '@/stores/topology'

/** Espaço entre equipamentos: cabe "↓ 999 Mbps ↑ 999 Mbps" entre os ícones. */
export const LINE_SPACING = 260
/** Em pé, a pílula fica deitada sobre o enlace: basta o ícone e o nome. */
export const COLUMN_SPACING = 175

export function pathMapPositions(
  nodeIds: ReadonlyArray<number>,
  vertical = false
): Record<number, TopologyPosition> {
  return Object.fromEntries(
    nodeIds.map((id, index) => [
      id,
      vertical ? { x: 0, y: index * COLUMN_SPACING } : { x: index * LINE_SPACING, y: 0 },
    ])
  )
}

/** Altura do mini mapa em pé: cresce com o caminho, até um teto. */
export function verticalMapHeight(nodeCount: number): number {
  return Math.min(140 + Math.max(0, nodeCount - 1) * COLUMN_SPACING * 0.75, 460)
}

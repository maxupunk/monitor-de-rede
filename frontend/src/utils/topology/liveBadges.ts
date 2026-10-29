/**
 * Selos coloridos sobre os rótulos do mapa: o tráfego do enlace (`↓` e `↑`,
 * cada um na cor do sentido) e a linha de CPU/memória do dispositivo (verde,
 * laranja, vermelho pela faixa de uso).
 *
 * O Cytoscape pinta texto de uma cor só por rótulo. Então o rótulo dele
 * continua lá — define o tamanho, o arranjo sem sobreposição, a exportação em
 * PNG e o que aparece em cada nível de zoom — e o selo HTML é desenhado
 * exatamente por cima, nas coordenadas do modelo. Aqui só se mede; quem
 * desenha é `TopologyLiveBadges.vue`.
 */
import type { BoundingBox12, Core, EdgeSingular } from 'cytoscape'
import { nodeHealthMetrics, type HealthMetric } from './presentation'

interface BadgeBase {
  id: string
  /** Centro, em coordenadas do modelo. */
  x: number
  y: number
  /** Tamanho da fonte do rótulo (modelo). */
  fontSize: number
  /** Abaixo disso (fonte × zoom), o Cytoscape não desenha o rótulo; o selo some junto. */
  minFont: number
  opacity: number
  /** Largura mínima (modelo): cobre o texto do rótulo de baixo. */
  width: number
  height: number
}

export interface HealthBadge extends BadgeBase {
  kind: 'health'
  metrics: HealthMetric[]
}

export interface TrafficBadge extends BadgeBase {
  kind: 'traffic'
  inBps: number
  outBps: number
}

export type LiveBadge = HealthBadge | TrafficBadge

/** Folga que o Cytoscape soma à caixa do rótulo ("margin of error"). */
const BOX_SLACK = 2
/** Altura de linha dos rótulos (`line-height` da folha de estilo). */
const LINE_HEIGHT = 1.2

const LABEL_BOX = {
  includeNodes: false,
  includeEdges: false,
  includeLabels: true,
  includeOverlays: false,
} as const

function inner(box: BoundingBox12, border: number) {
  return {
    x1: box.x1 + BOX_SLACK + border,
    x2: box.x2 - BOX_SLACK - border,
    y1: box.y1 + BOX_SLACK + border,
    y2: box.y2 - BOX_SLACK - border,
  }
}

function healthBadges(core: Core): HealthBadge[] {
  const badges: HealthBadge[] = []
  core.nodes('[health]').forEach((node) => {
    const health = String(node.data('health') ?? '')
    const label = String(node.style('label') ?? '')
    // De longe o rótulo é só o nome (ou nada): a linha de saúde não está lá.
    if (!health || !node.visible() || !label.endsWith(health)) return
    const metrics = nodeHealthMetrics({
      cpuPercent: node.data('cpuPercent') as number | null,
      memoryPercent: node.data('memoryPercent') as number | null,
    })
    if (metrics.length === 0) return
    const fontSize = node.numericStyle('font-size')
    const border = node.numericStyle('text-border-width') / 2
    const box = inner(node.boundingBox(LABEL_BOX), border)
    if (!(box.x2 > box.x1)) return
    const lineHeight = fontSize * LINE_HEIGHT
    // A última linha do rótulo: acima do acolchoamento de baixo.
    const bottom = box.y2 - node.numericStyle('text-background-padding')
    badges.push({
      kind: 'health',
      id: node.id(),
      x: (box.x1 + box.x2) / 2,
      y: bottom - lineHeight / 2,
      fontSize,
      minFont: node.numericStyle('min-zoomed-font-size'),
      opacity: node.effectiveOpacity(),
      width: box.x2 - box.x1,
      height: lineHeight,
      metrics,
    })
  })
  return badges
}

/** Onde o tráfego aparece: no meio do enlace ou perto de uma ponta (enlace focado). */
const TRAFFIC_LABELS = [
  ['label', { includeMainLabels: true, includeSourceLabels: false, includeTargetLabels: false }],
  [
    'source-label',
    { includeMainLabels: false, includeSourceLabels: true, includeTargetLabels: false },
  ],
  [
    'target-label',
    { includeMainLabels: false, includeSourceLabels: false, includeTargetLabels: true },
  ],
] as const

function trafficBadges(core: Core): TrafficBadge[] {
  const badges: TrafficBadge[] = []
  core.edges('[edgeId]').forEach((edge: EdgeSingular) => {
    const text = String(edge.data('trafficLabel') ?? '')
    if (!text || !edge.visible()) return
    for (const [property, parts] of TRAFFIC_LABELS) {
      if (String(edge.style(property) ?? '') !== text) continue
      const border = edge.numericStyle('text-border-width') / 2
      const box = inner(edge.boundingBox({ ...LABEL_BOX, ...parts }), border)
      if (!(box.x2 > box.x1)) continue
      badges.push({
        kind: 'traffic',
        id: `${edge.id()}:${property}`,
        x: (box.x1 + box.x2) / 2,
        y: (box.y1 + box.y2) / 2,
        fontSize: edge.numericStyle('font-size'),
        minFont: edge.numericStyle('min-zoomed-font-size'),
        opacity: edge.effectiveOpacity(),
        width: box.x2 - box.x1,
        height: box.y2 - box.y1,
        inBps: Number(edge.data('inBps') ?? 0),
        outBps: Number(edge.data('outBps') ?? 0),
      })
    }
  })
  return badges
}

export function collectLiveBadges(core: Core): LiveBadge[] {
  return [...trafficBadges(core), ...healthBadges(core)]
}

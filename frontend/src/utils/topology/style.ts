/**
 * Folha de estilo do Cytoscape construída a partir das cores do tema
 * Vuetify ativo. O canvas a reaplica quando o tema muda — nenhuma cor do mapa
 * é fixa no código, e o contraste vale no claro e no escuro (AGENTS §12).
 */
import type { StylesheetJson, StylesheetStyle } from 'cytoscape'
import { NODE_ICON_PATHS, type NodeIconKey } from './presentation'

export interface TopologyPalette {
  dark: boolean
  background: string
  surface: string
  onSurface: string
  primary: string
  secondary: string
  success: string
  error: string
  warning: string
  info: string
}

const FALLBACK: TopologyPalette = {
  dark: true,
  background: '#121212',
  surface: '#212121',
  onSurface: '#FFFFFF',
  primary: '#1976D2',
  secondary: '#00ACC1',
  success: '#4CAF50',
  error: '#FF5252',
  warning: '#FFC107',
  info: '#2196F3',
}

/** Paleta a partir de `useTheme().current.value` (cores em hex). */
export function paletteFromTheme(theme: {
  dark: boolean
  colors: Readonly<Record<string, unknown>>
}): TopologyPalette {
  const c: Record<string, string | undefined> = {}
  for (const [key, value] of Object.entries(theme.colors)) {
    if (typeof value === 'string') c[key] = value
  }
  return {
    dark: theme.dark,
    background: c.background ?? FALLBACK.background,
    surface: c.surface ?? FALLBACK.surface,
    onSurface: c['on-surface'] ?? (theme.dark ? '#FFFFFF' : '#000000'),
    primary: c.primary ?? FALLBACK.primary,
    secondary: c.secondary ?? FALLBACK.secondary,
    success: c.success ?? FALLBACK.success,
    error: c.error ?? FALLBACK.error,
    warning: c.warning ?? FALLBACK.warning,
    info: c.info ?? FALLBACK.info,
  }
}

function parseHex(hex: string): [number, number, number] {
  let value = hex.replace('#', '').trim()
  if (value.length === 3) value = [...value].map((ch) => ch + ch).join('')
  const int = Number.parseInt(value.slice(0, 6), 16)
  if (!Number.isFinite(int)) return [128, 128, 128]
  return [(int >> 16) & 255, (int >> 8) & 255, int & 255]
}

/** Mistura `a` com `b` (0 = só `a`, 1 = só `b`) e devolve hex. */
export function mix(a: string, b: string, amount: number): string {
  const [r1, g1, b1] = parseHex(a)
  const [r2, g2, b2] = parseHex(b)
  const channel = (x: number, y: number) =>
    Math.round(x + (y - x) * amount)
      .toString(16)
      .padStart(2, '0')
  return `#${channel(r1, r2)}${channel(g1, g2)}${channel(b1, b2)}`
}

/** Ícone MDI como `data:` URI de SVG na cor pedida. */
export function iconDataUri(path: string, color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="48" height="48"><path fill="${color}" d="${path}"/></svg>`
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
}

/** Zoom a partir do qual todo enlace mostra o tráfego. */
export const TRAFFIC_ZOOM = 1.9

/**
 * Nível de detalhe dos rótulos dos nós, pelo zoom:
 * - `near`: nome e IP;
 * - `mid`: só o nome (o IP vira ruído quando os nós se aproximam);
 * - `far`: só o nome da infraestrutura e de quem tem problema, em fonte maior;
 * - `xfar`: só os marcos (servidor, roteadores, firewalls).
 */
export type LodLevel = 'near' | 'mid' | 'far' | 'xfar'

export const LOD_CLASSES = 'lod-mid lod-far lod-xfar'

export function lodLevel(zoom: number): LodLevel {
  if (zoom >= 0.95) return 'near'
  if (zoom >= 0.55) return 'mid'
  if (zoom >= 0.3) return 'far'
  return 'xfar'
}

/** Diferença vertical mínima para o enlace virar ortogonal (abaixo, curva). */
export const ROUTE_MIN_DY = 70
export const ROUTE_CLASSES = 'route-down route-up'

/**
 * Traçado ortogonal de árvore (`taxi`): a curva vira barramento quando uma
 * ponta está bem acima da outra. A dobra fica sempre junto do nó de BAIXO
 * (o filho) — os irmãos de uma fileira dividem o mesmo barramento logo acima
 * deles, e o tronco desce pelo corredor que o layout deixou livre.
 */
export function edgeRouteClass(
  source: { x: number; y: number },
  target: { x: number; y: number }
): 'route-down' | 'route-up' | null {
  const dy = target.y - source.y
  if (Math.abs(dy) < ROUTE_MIN_DY) return null
  return dy > 0 ? 'route-down' : 'route-up'
}

/**
 * Deslocamento do rótulo abaixo do nó. O rótulo é centralizado e empurrado
 * para baixo (em vez de `text-valign: bottom`): com fundo e várias linhas, o
 * Cytoscape corta a segunda linha (o IP) quando o alinhamento é `bottom`.
 */
export function labelOffset(nodeSize: number, lines: number, fontSize = 12): number {
  const border = 4
  const gap = 5
  const padding = 3
  const height = lines * fontSize * 1.2 + padding * 2
  return Math.round(nodeSize / 2 + border / 2 + gap + height / 2)
}

function block(selector: string, style: StylesheetStyle['style']): StylesheetStyle {
  return { selector, style }
}

/** Rótulo de uma linha (só o nome) num tamanho de fonte, para cada forma de nó. */
function nameLabel(selector: string, fontSize: number, maxWidth: number): StylesheetStyle[] {
  const sizes: Array<[string, number]> = [
    ['', 46],
    ['.unmanaged', 40],
    ['.system', 60],
  ]
  return sizes.map(([shape, size]) =>
    block(
      selector
        .split(',')
        .map((part) => `${part.trim()}${shape}`)
        .join(', '),
      {
        label: 'data(name)',
        'font-size': fontSize,
        'text-max-width': `${maxWidth}px`,
        'text-margin-y': labelOffset(size, 1, fontSize),
      }
    )
  )
}

/** Nível de detalhe: o canvas põe `lod-*` nos nós; aqui só se decide o rótulo. */
function lodBlocks(): StylesheetStyle[] {
  return [
    ...nameLabel('node.lod-mid', 14, 170),
    block('node.lod-far, node.lod-xfar', { label: '' }),
    ...nameLabel(
      'node.lod-far.rank-core, node.lod-far.rank-infra, node.lod-far.status-offline, node.lod-far.status-warning, node.lod-far.path-highlight, node.lod-far:selected',
      28,
      320
    ),
    ...nameLabel(
      'node.lod-xfar.rank-core, node.lod-xfar.path-highlight, node.lod-xfar:selected',
      42,
      460
    ),
  ]
}

export function buildStylesheet(p: TopologyPalette = FALLBACK): StylesheetJson {
  const nodeFill = p.dark ? mix(p.surface, '#FFFFFF', 0.1) : '#FFFFFF'
  const iconColor = p.dark ? mix(p.onSurface, p.surface, 0.06) : mix(p.onSurface, '#FFFFFF', 0.15)
  const labelBg = p.dark ? mix(p.background, p.surface, 0.5) : '#FFFFFF'
  const edgeNeutral = mix(p.onSurface, p.background, p.dark ? 0.45 : 0.5)

  const iconBlocks = (Object.keys(NODE_ICON_PATHS) as NodeIconKey[]).map((key) =>
    block(`node[iconKey = "${key}"]`, {
      'background-image': iconDataUri(
        NODE_ICON_PATHS[key],
        key === 'system' ? '#FFFFFF' : iconColor
      ),
    })
  )

  const pill = (color: string) => ({
    'text-background-color': color,
    'text-background-opacity': 1,
    color: '#FFFFFF',
    'text-border-width': 0,
  })

  return [
    block('node', {
      width: 46,
      height: 46,
      shape: 'ellipse',
      'background-color': nodeFill,
      'background-fit': 'none',
      'background-width': '56%',
      'background-height': '56%',
      'background-clip': 'none',
      'border-width': 4,
      'border-color': p.info,
      label: 'data(label)',
      'font-family': 'Roboto, system-ui, sans-serif',
      'font-size': 12,
      'font-weight': 600,
      color: p.onSurface,
      'text-valign': 'center',
      'text-halign': 'center',
      'text-margin-y': labelOffset(46, 1),
      'text-wrap': 'wrap',
      'text-max-width': '160px',
      'line-height': 1.2,
      'text-background-color': labelBg,
      'text-background-opacity': 0.92,
      'text-background-padding': '3px',
      'text-background-shape': 'roundrectangle',
      'text-border-width': 1,
      'text-border-color': mix(p.onSurface, labelBg, 0.82),
      'text-border-opacity': 1,
      'min-zoomed-font-size': 7,
      'underlay-color': p.primary,
      'underlay-opacity': 0,
      'underlay-padding': 8,
      'underlay-shape': 'ellipse',
      'transition-property': 'opacity, underlay-opacity',
      'transition-duration': 150,
    }),
    ...iconBlocks,
    block('node.status-online', { 'border-color': p.success }),
    block('node.status-offline', {
      'border-color': p.error,
      'background-color': mix(nodeFill, p.error, p.dark ? 0.28 : 0.16),
    }),
    block('node.status-warning', {
      'border-color': p.warning,
      'background-color': mix(nodeFill, p.warning, p.dark ? 0.2 : 0.14),
    }),
    block('node.status-unknown', { 'border-color': p.info, 'border-style': 'dashed' }),
    block('node.unmanaged', { shape: 'round-rectangle', width: 40, height: 40 }),
    block('node.system', {
      shape: 'round-rectangle',
      width: 60,
      height: 60,
      'background-color': p.primary,
      'outline-width': 4,
      'outline-color': p.primary,
      'outline-opacity': 0.35,
      'outline-offset': 3,
      'font-size': 13,
      'font-weight': 700,
    }),
    block('node.two-line', { 'text-margin-y': labelOffset(46, 2) }),
    block('node.unmanaged', { 'text-margin-y': labelOffset(40, 1) }),
    block('node.unmanaged.two-line', { 'text-margin-y': labelOffset(40, 2) }),
    block('node.system', { 'text-margin-y': labelOffset(60, 1) }),
    block('node.system.two-line', { 'text-margin-y': labelOffset(60, 2) }),
    ...lodBlocks(),
    block('node.hover', { 'underlay-opacity': 0.18 }),
    block('node:selected', { 'underlay-opacity': 0.4, 'underlay-padding': 10 }),
    block('node.connect-source', {
      'underlay-color': p.secondary,
      'underlay-opacity': 0.55,
      'underlay-padding': 12,
    }),

    // Grupo por site (nó composto)
    block('node.site-group', {
      shape: 'round-rectangle',
      'background-color': p.primary,
      'background-opacity': p.dark ? 0.07 : 0.05,
      'border-width': 2,
      'border-style': 'dashed',
      'border-color': mix(p.primary, p.background, 0.35),
      'border-opacity': 1,
      label: 'data(label)',
      'text-valign': 'top',
      'text-halign': 'center',
      'text-margin-y': -8,
      'font-size': 22,
      'min-zoomed-font-size': 0,
      'font-weight': 700,
      color: p.onSurface,
      padding: '36px',
      'underlay-opacity': 0,
    }),

    block('edge', {
      width: 2,
      'curve-style': 'bezier',
      'control-point-step-size': 36,
      'line-color': edgeNeutral,
      'line-cap': 'round',
      'target-arrow-shape': 'none',
      'font-family': 'Roboto, system-ui, sans-serif',
      'font-size': 10,
      'font-weight': 600,
      color: p.onSurface,
      'text-background-color': labelBg,
      'text-background-opacity': 0.95,
      'text-background-padding': '3px',
      'text-background-shape': 'roundrectangle',
      'text-border-width': 1,
      'text-border-color': mix(p.onSurface, labelBg, 0.8),
      'text-border-opacity': 1,
      'min-zoomed-font-size': 6,
      'overlay-color': p.primary,
      'overlay-padding': 5,
      'overlay-opacity': 0,
      'transition-property': 'opacity, line-color, width',
      'transition-duration': 150,
    }),
    // Traçado ortogonal (árvore): o canvas escolhe a classe pela posição das pontas.
    block('edge.route-down, edge.route-up', {
      'curve-style': 'round-taxi',
      'taxi-direction': 'vertical',
      'taxi-radius': 10,
      'taxi-turn-min-distance': 6,
    }),
    block('edge.route-down', { 'taxi-turn': '-16px' }),
    block('edge.route-up', { 'taxi-turn': '16px' }),
    block('edge.tier-slow', { width: 1.5 }),
    block('edge.tier-gigabit', { width: 2.75 }),
    block('edge.tier-fast', { width: 4.5 }),
    block('edge.kind-fiber', {
      'line-outline-width': 1.5,
      'line-outline-color': p.secondary,
    }),
    block('edge.kind-wireless', { 'line-style': 'dashed', 'line-dash-pattern': [7, 5] }),
    block('edge.kind-vpn', { 'line-style': 'dashed', 'line-dash-pattern': [14, 6] }),
    block('edge.kind-inferred', { 'line-style': 'dashed', 'line-dash-pattern': [4, 6] }),
    block('edge.kind-parent', {
      'line-style': 'dashed',
      'line-dash-pattern': [2, 6],
      width: 1.5,
      'line-color': mix(p.secondary, p.background, 0.25),
    }),
    block('edge.util-normal', { 'line-color': p.success }),
    block('edge.util-high', { 'line-color': p.warning }),
    block('edge.util-critical', { 'line-color': p.error }),
    block('edge.status-down', {
      'line-color': p.error,
      'line-style': 'solid',
      'mid-target-arrow-shape': 'tee',
      'mid-target-arrow-color': p.error,
    }),
    block('edge.stale', { 'line-style': 'dotted', opacity: 0.5 }),
    block('edge.lod-detail[trafficLabel != ""]', { label: 'data(trafficLabel)' }),
    block('edge.hover[trafficLabel != ""]', { label: 'data(trafficLabel)' }),
    block('edge.hover', { 'overlay-opacity': 0.12 }),
    block('edge:selected', { 'overlay-opacity': 0.25, 'z-index': 10 }),
    block('edge:selected[trafficLabel != ""]', { label: 'data(trafficLabel)' }),
    block('edge.focus', { 'z-index': 8 }),
    block('edge.focus-out[trafficLabel != ""]', {
      'target-label': 'data(trafficLabel)',
      'target-text-offset': 62,
    }),
    block('edge.focus-in[trafficLabel != ""]', {
      'source-label': 'data(trafficLabel)',
      'source-text-offset': 62,
    }),
    block('edge.lod-detail.focus, edge.hover.focus, edge:selected.focus', {
      'target-label': '',
      'source-label': '',
    }),
    block('edge.ghost', {
      'line-color': p.secondary,
      'line-style': 'dashed',
      'line-dash-pattern': [6, 6],
      width: 3,
      'target-arrow-shape': 'triangle',
      'target-arrow-color': p.secondary,
    }),
    block('node.ghost', { width: 1, height: 1, opacity: 0, label: '' }),

    // Filtro
    block('.filtered-dimmed', { opacity: 0.14 }),
    block('.filtered-hidden', { display: 'none' }),

    // Caminho destacado
    block('.path-dimmed', { opacity: 0.12 }),
    block('node.path-highlight', {
      'underlay-color': p.primary,
      'underlay-opacity': 0.35,
      'underlay-padding': 10,
    }),
    block('edge.path-highlight', {
      'line-color': p.primary,
      width: 5,
      'z-index': 20,
    }),

    // Sobreposição (teste de enlace)
    block('edge.ov-pending', {
      'line-color': mix(p.info, p.background, 0.3),
      'line-style': 'dashed',
      'line-dash-pattern': [3, 5],
      width: 4,
    }),
    block('edge.ov-running', {
      'line-color': p.info,
      'line-style': 'dashed',
      'line-dash-pattern': [12, 8],
      width: 6,
      'z-index': 30,
    }),
    block('edge.ov-running.flow-forward', {
      'mid-target-arrow-shape': 'triangle',
      'mid-target-arrow-color': p.info,
      'arrow-scale': 1.4,
    }),
    block('edge.ov-running.flow-reverse', {
      'mid-source-arrow-shape': 'triangle',
      'mid-source-arrow-color': p.info,
      'arrow-scale': 1.4,
    }),
    block('edge.ov-running.flow-both', {
      'mid-target-arrow-shape': 'triangle',
      'mid-target-arrow-color': p.info,
      'mid-source-arrow-shape': 'triangle',
      'mid-source-arrow-color': p.info,
      'arrow-scale': 1.4,
    }),
    block('edge.ov-succeeded', {
      'line-color': p.success,
      'line-style': 'solid',
      width: 5,
      'z-index': 25,
    }),
    block('edge.ov-failed', {
      'line-color': p.error,
      'line-style': 'solid',
      width: 5,
      'z-index': 25,
    }),
    block('edge.ov-skipped', {
      'line-color': p.warning,
      'line-style': 'dotted',
      width: 3,
    }),
    block('edge.ov-labeled', {
      label: 'data(overlayLabel)',
      'font-size': 13,
      'font-weight': 700,
      'text-background-padding': '5px',
      // O resultado do teste é o assunto da tela: aparece em qualquer zoom.
      'min-zoomed-font-size': 0,
      'z-index': 40,
      ...pill(p.info),
    }),
    block('edge.ov-labeled.ov-succeeded', pill(mix(p.success, '#000000', 0.25))),
    block('edge.ov-labeled.ov-failed', pill(mix(p.error, '#000000', 0.2))),
    block('edge.ov-labeled.ov-skipped', pill(mix(p.warning, '#000000', 0.35))),
    // De longe, a pílula do resultado cresce com o nível de detalhe: o número
    // da medição continua legível com o caminho inteiro enquadrado.
    block('edge.ov-labeled.lod-mid', { 'font-size': 20 }),
    block('edge.ov-labeled.lod-far', { 'font-size': 34, 'text-background-padding': '8px' }),
    block('edge.ov-labeled.lod-xfar', { 'font-size': 56, 'text-background-padding': '12px' }),
    block('node.ov-running', {
      'underlay-color': p.info,
      'underlay-opacity': 0.45,
      'underlay-padding': 10,
    }),
    block('node.ov-succeeded', {
      'underlay-color': p.success,
      'underlay-opacity': 0.4,
      'underlay-padding': 9,
    }),
    block('node.ov-failed', {
      'underlay-color': p.error,
      'underlay-opacity': 0.45,
      'underlay-padding': 9,
    }),
  ]
}

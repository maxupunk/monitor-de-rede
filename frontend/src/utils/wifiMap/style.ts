/**
 * A cara do mapa Wi-Fi no Cytoscape, a partir das cores do tema (a mesma
 * paleta do mapa de topologia, `TopologyPalette`): reaplicada quando o tema
 * muda, contraste no claro e no escuro (AGENTS §12).
 *
 * A cor da linha AP↔aparelho é contínua, do vermelho (`error`) ao verde
 * (`success`) passando pelo amarelo (`warning`), pela nota do enlace que o
 * backend calcula (`quality::link_score`) — a tela só escolhe a cor.
 */
import type { StylesheetJson, StylesheetStyle } from 'cytoscape'
import { labelColors, labelOffset, mix, type TopologyPalette } from '@/utils/topology/style'

/** A cor da nota do enlace: 0 vermelho, 0,5 amarelo, 1 verde. */
export function linkColor(score: number, palette: TopologyPalette): string {
  const value = Number.isFinite(score) ? Math.min(Math.max(score, 0), 1) : 0
  return value <= 0.5
    ? mix(palette.error, palette.warning, value / 0.5)
    : mix(palette.warning, palette.success, (value - 0.5) / 0.5)
}

/** As paradas do gradiente da legenda, na mesma conta da linha. */
export function linkGradient(palette: TopologyPalette): string {
  const stops = [0, 0.25, 0.5, 0.75, 1].map(
    (score) => `${linkColor(score, palette)} ${score * 100}%`
  )
  return `linear-gradient(90deg, ${stops.join(', ')})`
}

/** Espessura da linha pela taxa física: 1,5 px a 6 Mbit/s, 6 px a 1,2 Gbit/s. */
export function linkWidth(txRateBps: number | null): number {
  if (txRateBps === null || txRateBps <= 0) return 2
  const ratio = Math.log2(txRateBps / 6e6) / Math.log2(1200e6 / 6e6)
  return Math.round((1.5 + Math.min(Math.max(ratio, 0), 1) * 4.5) * 10) / 10
}

export const STATION_SIZE = 34
export const AP_SIZE = 58

function block(selector: string, style: StylesheetStyle['style']): StylesheetStyle {
  return { selector, style }
}

export function wifiMapStylesheet(p: TopologyPalette): StylesheetJson {
  const nodeFill = p.dark ? mix(p.surface, '#FFFFFF', 0.1) : '#FFFFFF'
  const { background: labelBg } = labelColors(p)
  const ringColor = mix(p.onSurface, p.background, p.dark ? 0.62 : 0.7)
  const labelBorder = mix(p.onSurface, labelBg, 0.8)

  return [
    block('node', {
      label: 'data(label)',
      'font-family': 'Roboto, system-ui, sans-serif',
      'font-weight': 600,
      color: p.onSurface,
      'text-valign': 'center',
      'text-halign': 'center',
      'text-wrap': 'wrap',
      'line-height': 1.2,
      'text-background-color': labelBg,
      'text-background-opacity': 0.92,
      'text-background-padding': '2px',
      'text-background-shape': 'roundrectangle',
      'text-border-width': 1,
      'text-border-color': labelBorder,
      'text-border-opacity': 1,
      'min-zoomed-font-size': 8,
      'transition-property': 'opacity',
      'transition-duration': 200,
    }),
    // Anéis de distância: só referência, atrás de tudo e sem toque.
    block('node.ring', {
      shape: 'ellipse',
      width: 'data(size)',
      height: 'data(size)',
      'background-opacity': 0,
      'border-width': 1,
      'border-style': 'dashed',
      'border-color': ringColor,
      events: 'no',
      'z-index': 0,
      'z-compound-depth': 'bottom',
      'font-size': 10,
      'font-weight': 500,
      color: mix(p.onSurface, p.background, 0.25),
      'text-valign': 'top',
      'text-margin-y': 7,
      'text-background-opacity': 0.85,
      'text-border-width': 0,
    }),
    block('node.ap', {
      shape: 'round-rectangle',
      width: AP_SIZE,
      height: AP_SIZE,
      'background-color': p.primary,
      'background-image': 'data(icon)',
      'background-fit': 'none',
      'background-width': '62%',
      'background-height': '62%',
      'border-width': 3,
      'border-color': p.success,
      'font-size': 13,
      'font-weight': 700,
      'text-max-width': '180px',
      'text-margin-y': labelOffset(AP_SIZE, 2, 13),
      'z-index': 20,
    }),
    block('node.ap.ap-reading', { 'border-color': p.info, 'border-style': 'dashed' }),
    block('node.ap.ap-failed', {
      'border-color': p.error,
      'border-width': 5,
      'border-style': 'dashed',
      'background-color': mix(p.primary, p.error, 0.55),
    }),
    block('node.station', {
      shape: 'ellipse',
      width: STATION_SIZE,
      height: STATION_SIZE,
      'background-color': nodeFill,
      'background-image': 'data(icon)',
      'background-fit': 'none',
      'background-width': '58%',
      'background-height': '58%',
      'border-width': 3,
      'border-color': 'data(color)',
      'font-size': 11,
      'text-max-width': '130px',
      'text-margin-y': labelOffset(STATION_SIZE, 2, 11),
      'z-index': 10,
    }),
    // Em repouso: borda pontilhada (não é problema da rede).
    block('node.station.sleeping', { 'border-style': 'dotted' }),
    // Precisa de atenção: borda grossa — cor nunca é o único sinal.
    block('node.station.attention', { 'border-width': 5 }),
    // Vigiado: anel na cor primária, por fora da borda.
    block('node.station.watched', {
      'outline-width': 3,
      'outline-color': p.primary,
      'outline-opacity': 1,
      'outline-offset': 3,
    }),
    // Saiu do Wi-Fi: esmaece e some (a animação tira o nó).
    block('node.station.departed', {
      opacity: 0.35,
      'border-style': 'dashed',
      'border-color': ringColor,
    }),
    block('edge.link', {
      width: 'data(width)',
      'line-color': 'data(color)',
      'curve-style': 'straight',
      opacity: 0.9,
      'z-index': 5,
      label: '',
      'font-size': 10,
      'font-weight': 600,
      color: p.onSurface,
      'text-background-color': labelBg,
      'text-background-opacity': 0.92,
      'text-background-padding': '2px',
      'text-background-shape': 'roundrectangle',
      'text-border-width': 1,
      'text-border-color': labelBorder,
      'text-border-opacity': 1,
      'text-rotation': 'autorotate',
    }),
    block('edge.link.sleeping', { 'line-style': 'dotted' }),
    block('edge.link.departed', { opacity: 0.25, 'line-style': 'dashed' }),
    // Destaque ao passar o mouse e ao selecionar: o aparelho, a linha e o AP.
    block('.dim', { opacity: 0.18 }),
    block('edge.link.focus', { label: 'data(distanceLabel)', opacity: 1, 'z-index': 40 }),
    block('node.focus', { 'z-index': 50 }),
    block('node.station.selected', {
      'underlay-color': p.primary,
      'underlay-opacity': 0.3,
      'underlay-padding': 9,
      'underlay-shape': 'ellipse',
    }),
    // A troca de AP: o aparelho desliza com destaque e a linha anda (as
    // "formigas" do mapa de topologia, `antsAnimator`).
    block('node.station.roaming', {
      'underlay-color': p.info,
      'underlay-opacity': 0.45,
      'underlay-padding': 12,
      'underlay-shape': 'ellipse',
      'z-index': 60,
    }),
    block('edge.ov-running', {
      'line-style': 'dashed',
      'line-dash-pattern': [10, 7],
      'line-color': p.info,
      width: 5,
      opacity: 1,
      'z-index': 45,
    }),
  ]
}

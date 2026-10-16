/**
 * Busca do mapa (Ctrl/⌘+K): ranqueia nós por nome, IP, fabricante e modelo
 * com tolerância a digitação parcial ("sw cor" acha "Switch Core"). Pura e
 * testada — o componente só exibe.
 */
import type { TopologyNode } from '@/stores/topology'
import { nodeIp, nodeTypeLabel } from './presentation'

export interface SearchHit {
  node: TopologyNode
  score: number
  /** Campo que mais pesou — a lista mostra "IP", "Fabricante"… */
  matchedField: 'name' | 'ip' | 'vendor' | 'model' | 'type' | 'site'
}

function normalize(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().trim()
}

/** Subsequência: todas as letras da consulta, na ordem, dentro do texto. */
function subsequenceScore(query: string, text: string): number {
  let qi = 0
  let gaps = 0
  let last = -1
  for (let ti = 0; ti < text.length && qi < query.length; ti += 1) {
    if (text[ti] === query[qi]) {
      if (last >= 0) gaps += ti - last - 1
      last = ti
      qi += 1
    }
  }
  // Letras muito espalhadas ("core" em "sw-acesso-terreo") são coincidência.
  if (qi < query.length || gaps > query.length) return 0
  return Math.max(1, 20 - gaps)
}

/** Pontuação de um termo contra um campo (0 = não casa). */
export function termScore(term: string, field: string): number {
  if (!term || !field) return 0
  if (field === term) return 100
  if (field.startsWith(term)) return 80
  const words = field.split(/[\s\-_./:]+/)
  if (words.some((word) => word.startsWith(term))) return 65
  if (field.includes(term)) return 50
  return subsequenceScore(term, field)
}

const FIELD_WEIGHT: Record<SearchHit['matchedField'], number> = {
  name: 1,
  ip: 0.95,
  vendor: 0.6,
  model: 0.6,
  type: 0.5,
  site: 0.4,
}

export function searchNodes(
  nodes: readonly TopologyNode[],
  query: string,
  limit = 12
): SearchHit[] {
  const terms = normalize(query).split(/\s+/).filter(Boolean)
  if (terms.length === 0) return []

  const hits: SearchHit[] = []
  for (const node of nodes) {
    const fields: Array<[SearchHit['matchedField'], string]> = [
      ['name', normalize(node.name ?? '')],
      ['ip', normalize(nodeIp(node) ?? '')],
      ['vendor', normalize(node.vendor ?? '')],
      ['model', normalize(node.model ?? '')],
      ['type', normalize(nodeTypeLabel(node))],
      ['site', normalize(node.siteName ?? '')],
    ]
    let total = 0
    let best: { field: SearchHit['matchedField']; score: number } | null = null
    let allMatched = true
    for (const term of terms) {
      let termBest = 0
      for (const [field, value] of fields) {
        // IP só casa por prefixo/trecho — subsequência em números é ruído.
        const raw = termScore(term, value)
        const score = (field === 'ip' && raw < 50 ? 0 : raw) * FIELD_WEIGHT[field]
        if (score > termBest) termBest = score
        if (!best || score > best.score) best = { field, score }
      }
      if (termBest === 0) {
        allMatched = false
        break
      }
      total += termBest
    }
    if (!allMatched || !best) continue
    hits.push({ node, score: total / terms.length, matchedField: best.field })
  }

  return hits
    .sort((a, b) => b.score - a.score || a.node.name.localeCompare(b.node.name))
    .slice(0, limit)
}

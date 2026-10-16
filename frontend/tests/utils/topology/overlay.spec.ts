import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import { buildElements } from '@/utils/topology/elements'
import { edgeOverlayClasses, highlightClass, nodeOverlayClasses } from '@/utils/topology/overlay'
import { edge, node } from './fixtures'

describe('camada de sobreposição → classes do canvas', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('enlace em execução anima no sentido pedido; concluído leva o rótulo', () => {
    expect(edgeOverlayClasses({ state: 'running', flow: 'reverse' })).toEqual([
      'ov',
      'ov-running',
      'flow-reverse',
    ])
    expect(edgeOverlayClasses({ state: 'running' })).toContain('flow-forward')
    expect(edgeOverlayClasses({ state: 'succeeded', label: '↓ 487 Mbps' })).toEqual([
      'ov',
      'ov-succeeded',
      'ov-labeled',
    ])
    expect(edgeOverlayClasses(undefined)).toEqual([])
    expect(nodeOverlayClasses({ state: 'failed', label: 'x' })).toEqual(['ov', 'ov-failed'])
  })

  it('destaque de caminho: quem está nele brilha, o resto esmaece', () => {
    const path = { nodeIds: [1, 2], edgeIds: [10] }
    expect(highlightClass(path, 'node', 1)).toBe('path-highlight')
    expect(highlightClass(path, 'node', 3)).toBe('path-dimmed')
    expect(highlightClass(path, 'edge', 10)).toBe('path-highlight')
    expect(highlightClass(null, 'edge', 10)).toBeNull()
  })

  it('o estado do store chega aos elementos (teste de enlace)', () => {
    const overlay = useTopologyOverlayStore()
    overlay.setPathHighlight({ nodeIds: [1, 2, 2], edgeIds: [10] })
    overlay.setEdgeOverlay(10, { state: 'running', flow: 'forward' })
    overlay.setEdgeOverlay(11, { state: 'succeeded', label: '↓ 487 Mbps ↑ 471 Mbps' })
    overlay.setNodeOverlay(2, { state: 'failed' })
    expect(overlay.hasRunning).toBe(true)
    expect(overlay.highlight?.nodeIds).toEqual([1, 2])

    const elements = buildElements({
      nodes: [node(1), node(2), node(3)],
      edges: [edge(10, 1, 2), edge(11, 2, 3)],
      positions: {},
      overlay: {
        highlight: overlay.highlight,
        edgeOverlays: overlay.edgeOverlays,
        nodeOverlays: overlay.nodeOverlays,
      },
    })
    const get = (id: string) => elements.find((el) => el.data.id === id)!
    expect(get('e10').classes).toContain('ov-running')
    expect(get('e10').classes).toContain('flow-forward')
    expect(get('e10').classes).toContain('path-highlight')
    expect(get('e11').classes).toContain('ov-succeeded')
    expect(get('e11').classes).toContain('path-dimmed')
    expect(get('e11').data.overlayLabel).toBe('↓ 487 Mbps ↑ 471 Mbps')
    expect(get('n2').classes).toContain('ov-failed')
    expect(get('n3').classes).toContain('path-dimmed')

    overlay.setEdgeOverlay(10, null)
    expect(overlay.hasRunning).toBe(false)
    overlay.clear()
    expect(overlay.isActive).toBe(false)
  })
})

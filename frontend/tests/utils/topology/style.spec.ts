import { describe, expect, it } from 'vitest'

import { buildStylesheet } from '@/utils/topology/style'

type Block = { selector: string; style: Record<string, unknown> }

function blocksFor(selector: string): Block[] {
  return (buildStylesheet() as Block[]).filter((block) => block.selector === selector)
}

describe('folha de estilo da topologia — puxar cabo', () => {
  it('o fantasma do cabo não recebe clique nem hover', () => {
    // Ele fica exatamente sob o cursor: se recebesse eventos, o clique no
    // dispositivo de destino caía nele e o cabo nunca ligava, e o tooltip do
    // destino abria e fechava a cada movimento.
    for (const selector of ['node.ghost', 'edge.ghost']) {
      const blocks = blocksFor(selector)
      expect(blocks.length, selector).toBeGreaterThan(0)
      expect(
        blocks.some((block) => block.style.events === 'no'),
        selector
      ).toBe(true)
    }
  })

  it('o destino válido sob o cursor tem destaque próprio', () => {
    expect(blocksFor('node.connect-target').length).toBeGreaterThan(0)
  })
})

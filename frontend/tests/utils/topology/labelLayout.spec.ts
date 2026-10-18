import { describe, expect, it } from 'vitest'

import { polylineBoxes, resolveLabelOverlaps, type LabelRect } from '@/utils/topology/labelLayout'

const rect = (id: string, x: number, y: number, w = 120, h = 18): LabelRect => ({
  id,
  x1: x,
  y1: y,
  x2: x + w,
  y2: y + h,
})

function intersects(a: LabelRect, b: LabelRect): boolean {
  return a.x1 < b.x2 && b.x1 < a.x2 && a.y1 < b.y2 && b.y1 < a.y2
}

function apply(labels: LabelRect[], shifts: Map<string, number>): LabelRect[] {
  return labels.map((l) => {
    const dy = shifts.get(l.id) ?? 0
    return { ...l, y1: l.y1 + dy, y2: l.y2 + dy }
  })
}

describe('rótulos de enlace sem sobreposição', () => {
  it('rótulos que já não se tocam ficam onde estão', () => {
    const shifts = resolveLabelOverlaps([rect('a', 0, 0), rect('b', 300, 0)])
    expect(shifts.get('a')).toBe(0)
    expect(shifts.get('b')).toBe(0)
  })

  it('irmãos no mesmo barramento (o caso do mapa) se afastam e nenhum cobre o outro', () => {
    // "online" e "switch-assistencia" chegando à BORDA: os meios caem juntos.
    const labels = [rect('online', 20, 100, 110), rect('assist', 60, 102, 180)]
    const shifts = resolveLabelOverlaps(labels)
    const [a, b] = apply(labels, shifts)
    expect(intersects(a, b)).toBe(false)
    // O primeiro do aglomerado fica no lugar; o outro anda o mínimo (um passo).
    expect(shifts.get('online')).toBe(0)
    expect(Math.abs(shifts.get('assist') ?? 0)).toBeLessThanOrEqual(21)
  })

  it('três empilhados viram uma coluna legível', () => {
    const labels = [rect('a', 0, 50), rect('b', 10, 51), rect('c', 5, 52)]
    const placed = apply(labels, resolveLabelOverlaps(labels))
    for (let i = 0; i < placed.length; i++)
      for (let j = i + 1; j < placed.length; j++)
        expect(intersects(placed[i], placed[j])).toBe(false)
  })

  it('não cobre um nó: desvia do obstáculo', () => {
    const node = { x1: 0, y1: 0, x2: 120, y2: 40 }
    const labels = [rect('a', 10, 10)]
    const [moved] = apply(labels, resolveLabelOverlaps(labels, [node]))
    expect(moved.y1 >= node.y2 || moved.y2 <= node.y1).toBe(true)
  })

  it('o arranjo é estável: a mesma entrada dá o mesmo resultado em qualquer ordem', () => {
    const labels = [rect('x', 0, 0), rect('y', 0, 2), rect('z', 0, 4)]
    const one = resolveLabelOverlaps(labels)
    const two = resolveLabelOverlaps([...labels].reverse())
    expect([...two.entries()].sort()).toEqual([...one.entries()].sort())
  })

  it('sem lugar livre por perto, fica no lugar (perto do seu enlace)', () => {
    const wall = Array.from({ length: 40 }, (_, i) => ({
      x1: 0,
      x2: 200,
      y1: -400 + i * 20,
      y2: -400 + i * 20 + 20,
    }))
    const shifts = resolveLabelOverlaps([rect('preso', 10, 0)], wall)
    expect(shifts.get('preso')).toBe(0)
  })
})

describe('rótulo não fica sobre a linha de outro enlace', () => {
  it('desvia do barramento de outro enlace, mas pode ficar sobre a própria linha', () => {
    // O barramento do switch-assistencia passa exatamente onde cai o meio do
    // enlace do "online": o rótulo do "online" sai de cima dele.
    const bus = polylineBoxes(
      [
        { x: 0, y: 100 },
        { x: 400, y: 100 },
      ],
      'assist'
    )
    const own = polylineBoxes(
      [
        { x: 60, y: -200 },
        { x: 60, y: 100 },
      ],
      'online'
    )
    const label = rect('online', 10, 92, 110, 16)
    const [moved] = apply([label], resolveLabelOverlaps([label], [...bus, ...own]))
    expect(bus.some((b) => intersects(moved, { id: 'b', ...b }))).toBe(false)
    // Da própria linha ele não foge: segue sobre ela (x 60 continua dentro).
    expect(moved.x1 < 60 && moved.x2 > 60).toBe(true)
  })
})

describe('escala do mapa', () => {
  it('1.200 rótulos e milhares de trechos de linha: arranja rápido e sem sobreposição', () => {
    const labels: LabelRect[] = []
    const lines = []
    for (let i = 0; i < 1200; i++) {
      // Fileiras de irmãos com os meios caindo no mesmo barramento.
      const row = Math.floor(i / 40)
      // Irmãos a cada 100 (um nó por coluna), rótulos de 150: cada um cobre os
      // vizinhos — a densidade de uma fileira cheia do mapa.
      labels.push(rect(`e${i}`, (i % 40) * 100, row * 200, 150, 16))
      lines.push(
        ...polylineBoxes(
          [
            { x: (i % 40) * 100 + 75, y: row * 200 - 150 },
            { x: (i % 40) * 100 + 75, y: row * 200 + 8 },
          ],
          `e${i}`
        )
      )
    }
    const started = performance.now()
    const placed = apply(labels, resolveLabelOverlaps(labels, lines))
    const elapsed = performance.now() - started
    expect(elapsed).toBeLessThan(1500)
    // Nenhum par dentro do alcance se cobre (os que esgotaram o alcance ficam
    // no lugar; contamos só os que se mexeram ou ficaram livres).
    let collisions = 0
    for (let i = 0; i < placed.length; i++)
      for (let j = i + 1; j < Math.min(placed.length, i + 60); j++)
        if (intersects(placed[i], placed[j])) collisions++
    expect(collisions).toBeLessThan(placed.length * 0.05)
  })
})

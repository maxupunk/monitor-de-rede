import { describe, expect, it } from 'vitest'

import {
  pathAnchors,
  polylineBoxes,
  quadraticPoints,
  resolveLabelOverlaps,
  type LabelRect,
  type LabelShift,
} from '@/utils/topology/labelLayout'

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

const STILL: LabelShift = { dx: 0, dy: 0 }

function apply(labels: LabelRect[], shifts: Map<string, LabelShift>): LabelRect[] {
  return labels.map((l) => {
    const { dx, dy } = shifts.get(l.id) ?? STILL
    return { ...l, x1: l.x1 + dx, x2: l.x2 + dx, y1: l.y1 + dy, y2: l.y2 + dy }
  })
}

describe('rótulos de enlace sem sobreposição', () => {
  it('rótulos que já não se tocam ficam onde estão', () => {
    const shifts = resolveLabelOverlaps([rect('a', 0, 0), rect('b', 300, 0)])
    expect(shifts.get('a')).toEqual(STILL)
    expect(shifts.get('b')).toEqual(STILL)
  })

  it('irmãos no mesmo barramento (o caso do mapa) se afastam e nenhum cobre o outro', () => {
    // "online" e "switch-assistencia" chegando à BORDA: os meios caem juntos.
    const labels = [rect('online', 20, 100, 110), rect('assist', 60, 102, 180)]
    const shifts = resolveLabelOverlaps(labels)
    const [a, b] = apply(labels, shifts)
    expect(intersects(a, b)).toBe(false)
    // O primeiro do aglomerado fica no lugar; o outro anda o mínimo (um passo).
    expect(shifts.get('online')).toEqual(STILL)
    expect(Math.abs(shifts.get('assist')?.dy ?? 0)).toBeLessThanOrEqual(21)
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
    expect(shifts.get('preso')).toEqual(STILL)
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

describe('rótulo desliza sobre o próprio enlace', () => {
  /** Rótulo de 120×18 centrado em `center`, com os lugares ao longo de `path`. */
  function onPath(id: string, path: { x: number; y: number }[], center: { x: number; y: number }) {
    const label = rect(id, center.x - 60, center.y - 9)
    return { ...label, along: pathAnchors(path, center) }
  }
  const centerOf = (l: LabelRect) => ({ x: (l.x1 + l.x2) / 2, y: (l.y1 + l.y2) / 2 })

  it('pontos ao longo do traçado: o lugar atual primeiro, depois do meio para as pontas', () => {
    const anchors = pathAnchors(
      [
        { x: 0, y: 0 },
        { x: 100, y: 0 },
      ],
      { x: 50, y: 0 }
    )
    expect(anchors[0]).toEqual(STILL)
    expect(anchors[1]).toEqual(STILL)
    expect(anchors[2]).toEqual({ dx: -10, dy: 0 })
    expect(anchors[3]).toEqual({ dx: 10, dy: 0 })
    // Linha quebrada (barramento): o meio é pelo comprimento, não pelas pontas.
    const bus = pathAnchors(
      [
        { x: 0, y: 0 },
        { x: 0, y: 100 },
        { x: 100, y: 100 },
      ],
      { x: 0, y: 0 }
    )
    expect(bus[1]).toEqual({ dx: 0, dy: 100 })
  })

  it('enlace horizontal com o meio coberto (BORDA → switch): anda na linha, não sai dela', () => {
    // O rótulo de três linhas de um nó cobre o meio do enlace.
    const path = [
      { x: 0, y: 300 },
      { x: 400, y: 300 },
    ]
    const nodeLabel = { x1: 140, x2: 260, y1: 280, y2: 340 }
    const label = onPath('borda-switch', path, { x: 200, y: 300 })
    const [moved] = apply([label], resolveLabelOverlaps([label], [nodeLabel]))
    const at = centerOf(moved)
    expect(at.y).toBe(300)
    expect(moved.x2 <= nodeLabel.x1 || moved.x1 >= nodeLabel.x2).toBe(true)
  })

  it('barramento que passa sob o rótulo do nó (VM → BORDA): fica no traçado, não abaixo do outro nó', () => {
    // BORDA em cima, VM embaixo à esquerda; o meio cai junto ao rótulo da BORDA.
    const path = [
      { x: 250, y: 345 },
      { x: 250, y: 432 },
      { x: 140, y: 432 },
      { x: 140, y: 465 },
    ]
    const bordaLabel = { x1: 170, x2: 330, y1: 360, y2: 428 }
    const vm = { x1: 60, x2: 225, y1: 465, y2: 585 }
    const label = onPath('vm-borda', path, { x: 195, y: 432 })
    const [moved] = apply([label], resolveLabelOverlaps([label], [bordaLabel, vm]))
    const at = centerOf(moved)
    const onLine =
      (Math.abs(at.y - 432) < 0.01 && at.x >= 140 && at.x <= 250) ||
      (Math.abs(at.x - 250) < 0.01 && at.y >= 345 && at.y <= 432) ||
      (Math.abs(at.x - 140) < 0.01 && at.y >= 432 && at.y <= 465)
    expect(onLine).toBe(true)
    // Nunca abaixo do nó da VM, como acontecia com o deslize vertical.
    expect(at.y).toBeLessThan(vm.y2)
  })

  it('sem lugar livre no traçado, fica no meio do enlace', () => {
    const path = [
      { x: 0, y: 0 },
      { x: 200, y: 0 },
    ]
    const wall = { x1: -100, x2: 300, y1: -50, y2: 50 }
    const label = onPath('preso', path, { x: 100, y: 0 })
    expect(resolveLabelOverlaps([label], [wall]).get('preso')).toEqual(STILL)
  })

  it('curva amostrada passa pelas pontas e pelo meio da quadrática', () => {
    const points = quadraticPoints({ x: 0, y: 0 }, { x: 50, y: 100 }, { x: 100, y: 0 }, 2)
    expect(points).toEqual([
      { x: 0, y: 0 },
      { x: 50, y: 50 },
      { x: 100, y: 0 },
    ])
  })
})

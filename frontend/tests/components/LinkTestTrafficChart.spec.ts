import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
import LinkTestTrafficChart from '@/components/linkTests/LinkTestTrafficChart.vue'
import type { LinkTestHealthSample } from '@/types/linkTests'

/** O gráfico mede a largura pelo ResizeObserver: aqui, 600 px. */
class WideObserver {
  constructor(private callback: ResizeObserverCallback) {}
  observe() {
    this.callback(
      [{ contentRect: { width: 600 } } as ResizeObserverEntry],
      this as unknown as ResizeObserver
    )
  }
  disconnect() {}
}

const health = (
  deviceId: number,
  atMs: number,
  phase: LinkTestHealthSample['phase'],
  cpu: number,
  memory: number
): LinkTestHealthSample => ({ deviceId, atMs, phase, cpu, memory })

afterEach(() => vi.unstubAllGlobals())

describe('linha do tempo do teste', () => {
  it('cada faixa tem uma série só, e a leitura do instante usa a mesma cor dela', async () => {
    vi.stubGlobal('ResizeObserver', WideObserver)
    const wrapper = mount(LinkTestTrafficChart, {
      global: { plugins: [createVuetify({ components, directives })] },
      props: {
        forward: [1, 2, 3].map((s) => ({
          flow: 'forward' as const,
          bps: 2.5e8,
          tMs: s * 1000,
          atMs: 8000 + s * 1000,
        })),
        reverse: [],
        health: [
          health(3, 0, 'before', 10, 74),
          health(3, 5000, 'before', 11, 74),
          health(3, 10000, 'during', 96, 86),
          health(3, 15000, 'after', 13, 76),
          health(4, 0, 'before', 2, 7),
          health(4, 10000, 'during', 26, 7),
        ],
        names: { 3: 'AP-Terraço', 4: 'AP-assistencia' },
        order: [3, 4],
      },
    })
    await wrapper.vm.$nextTick()

    const lanes = wrapper.findAll('.lane').filter((lane) => lane.text().startsWith('CPU ·'))
    expect(lanes.map((lane) => lane.find('strong').text())).toEqual([
      'CPU · AP-Terraço',
      'CPU · AP-assistencia',
    ])
    // Uma linha por faixa de CPU (nada de séries misturadas).
    for (const lane of lanes) expect(lane.findAll('path.line')).toHaveLength(1)
    // Antes → no teste → depois, ao lado da faixa.
    expect(lanes[0].text()).toContain('CPU 11% → 96% no teste, 13% depois · RAM 74% → 86%')

    // Passa o ponteiro no meio do teste.
    const svg = wrapper.find('svg')
    svg.element.getBoundingClientRect = () => ({ left: 0, top: 0, width: 600 }) as DOMRect
    svg.element.dispatchEvent(new MouseEvent('pointermove', { clientX: 400 }))
    await wrapper.vm.$nextTick()
    const readout = wrapper.find('.readout')
    expect(readout.text()).toContain('AP-Terraço')
    const strokeOf = (index: number) =>
      (lanes[index].find('path.line').element as SVGPathElement).style.stroke
    const swatchOf = (name: string) =>
      (
        readout
          .findAll('.readout-item')
          .find((item) => item.text().includes(name))
          ?.find('.swatch').element as HTMLElement
      ).style.background
    expect(swatchOf('AP-Terraço')).toBe(strokeOf(0))
    expect(swatchOf('AP-assistencia')).toBe(strokeOf(1))
    // E o pico define a cor: vermelho no teto, verde folgado.
    expect(strokeOf(0)).toContain('--v-theme-error')
    expect(strokeOf(1)).toContain('--v-theme-success')
  })
})

import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import StackedUsageChart from '@/components/StackedUsageChart.vue'

const samples = [
  { time: '2026-09-23T10:00:00Z', values: { a: 10, b: 30 } },
  { time: '2026-09-23T10:00:05Z', values: { a: 15, b: 25 } },
  { time: '2026-09-23T10:00:10Z', values: { a: 40, b: 5 } },
]

function montar() {
  return mount(StackedUsageChart, {
    props: {
      samples,
      labels: { a: 'api', b: 'banco' },
      formatValue: (value: number) => `${value}%`,
      totalLabel: 'CPU total',
    },
    global: { stubs: { 'v-icon': true } },
  })
}

describe('StackedUsageChart.vue', () => {
  it('desenha uma faixa por série, a linha do total e a legenda com o valor atual', () => {
    const wrapper = montar()
    expect(wrapper.findAll('polygon')).toHaveLength(2)
    expect(wrapper.find('polyline').exists()).toBe(true)
    const legenda = wrapper.find('.stacked-usage__legend').text()
    expect(legenda).toContain('CPU total 45%')
    expect(legenda).toContain('api 40%')
    expect(legenda).toContain('banco 5%')
  })

  it('ao passar o mouse mostra só a faixa sob o cursor, destacada, com o total no rodapé', async () => {
    const wrapper = montar()
    const plot = wrapper.find('.stacked-usage__plot')
    const element = plot.element as HTMLElement
    element.getBoundingClientRect = () =>
      ({ left: 0, top: 0, width: 200, height: 100, right: 200, bottom: 100 }) as DOMRect

    // Eixo até 50%; y=50 é 25%: dentro de "banco" (10–40) no primeiro instante.
    await plot.trigger('mousemove', { clientX: 0, clientY: 50 })
    let tooltip = wrapper.find('.stacked-usage__tooltip')
    const linhas = tooltip.findAll('.stacked-usage__row')
    expect(linhas).toHaveLength(1)
    expect(linhas[0].text()).toContain('banco')
    expect(linhas[0].text()).toContain('30%')
    expect(linhas[0].text()).toContain('75%')
    expect(tooltip.find('.stacked-usage__tooltip-foot').text()).toContain('+1 outro')
    expect(tooltip.find('.stacked-usage__tooltip-foot').text()).toContain('CPU total 40%')
    const opacidades = wrapper.findAll('polygon').map((p) => p.attributes('fill-opacity'))
    expect(opacidades).toEqual(['0.3', '0.85'])

    // Acima da pilha: os maiores do instante, sem esconder ninguém aqui.
    await plot.trigger('mousemove', { clientX: 200, clientY: 2 })
    tooltip = wrapper.find('.stacked-usage__tooltip')
    expect(tooltip.findAll('.stacked-usage__row')).toHaveLength(2)
    expect(tooltip.find('.stacked-usage__tooltip-foot').text()).toContain('CPU total 45%')

    await plot.trigger('mouseleave')
    expect(wrapper.find('.stacked-usage__tooltip').exists()).toBe(false)
  })

  it('sem amostras mostra que está aguardando o SSE', () => {
    const wrapper = mount(StackedUsageChart, {
      props: { samples: [], labels: {}, formatValue: String, totalLabel: 'RAM total' },
      global: { stubs: { 'v-icon': true } },
    })
    expect(wrapper.text()).toContain('Aguardando a primeira amostra')
  })
})

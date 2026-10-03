import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import PluginReport from '@/components/plugins/PluginReport.vue'

const passthrough = { template: '<div><slot /></div>' }
const stubs = {
  'v-card': passthrough,
  'v-chip': { template: '<span><slot /></span>' },
  'v-table': { template: '<table><slot /></table>' },
  'v-alert': { props: ['text', 'title'], template: '<section>{{ title }} {{ text }}</section>' },
  'v-expansion-panels': {
    props: ['modelValue'],
    template: '<div class="panels" :data-open="(modelValue ?? []).length"><slot /></div>',
  },
  'v-expansion-panel': passthrough,
  'v-expansion-panel-title': passthrough,
  'v-expansion-panel-text': passthrough,
}

describe('PluginReport.vue', () => {
  it('esconde as colunas `_` e recolhe os comandos quando há o que ler antes', () => {
    const wrapper = mount(PluginReport, {
      props: {
        output: {
          summary: '1 mudança(s) neste equipamento.',
          changes: [
            { change: 'criar', ssid: 'Loja', _section: 'nm_70feed55_2g', detail: '2,4 GHz' },
          ],
          commands: ["uci set wireless.nm_70feed55_2g='wifi-iface'"],
          _counts: { create: 1 },
        },
        presentation: {
          labels: {
            changes: 'Mudanças',
            change: 'Mudança',
            detail: 'Detalhe',
            commands: 'Comandos (técnico)',
            ssid: 'Rede',
          },
        },
      },
      global: { stubs },
    })

    const headers = wrapper.findAll('th').map((th) => th.text())
    expect(headers).toEqual(['Mudança', 'Rede', 'Detalhe'])
    expect(wrapper.find('td').exists()).toBe(true)
    expect(wrapper.findAll('td').map((td) => td.text())).not.toContain('nm_70feed55_2g')
    expect(wrapper.text()).not.toContain('create')
    expect(wrapper.find('.panels').attributes('data-open')).toBe('0')
    expect(wrapper.text()).toContain('Comandos (técnico)')
  })

  it('abre as linhas quando elas são o resultado todo', () => {
    const wrapper = mount(PluginReport, {
      props: { output: { log: ['linha 1', 'linha 2'] } },
      global: { stubs },
    })

    expect(wrapper.find('.panels').attributes('data-open')).toBe('1')
    expect(wrapper.text()).toContain('linha 2')
  })

  it('formata pelo que a ação declara e mostra o resumo do cartão no topo', () => {
    const wrapper = mount(PluginReport, {
      props: {
        output: {
          _card: [{ label: '5 GHz', value: 'canal 36 · 80 MHz' }],
          uptime_seconds: 90061,
          link: 'up',
        },
        presentation: {
          labels: { uptime_seconds: 'Ligado há', link: 'Link' },
          formats: { uptime_seconds: 'uptime', link: 'state' },
        },
      },
      global: { stubs },
    })

    const text = wrapper.text()
    expect(text).toContain('5 GHz')
    expect(text).toContain('canal 36 · 80 MHz')
    expect(text).toContain('Ligado há')
    expect(text).toContain('1 dia')
    expect(text).not.toContain('90061')
    expect(text).toContain('Up')
  })

  it('as colunas seguem a ordem que a ação declara, não a alfabética', () => {
    const wrapper = mount(PluginReport, {
      props: {
        output: {
          assignments: [{ band: '5 GHz', channel: '40', current: '36', device: 'AP Sala' }],
        },
        presentation: {
          labels: {
            device: 'Roteador',
            current: 'Canal atual',
            channel: 'Canal sugerido',
            band: 'Banda',
          },
          order: ['device', 'band', 'current', 'channel'],
        },
      },
      global: { stubs },
    })

    expect(wrapper.findAll('th').map((th) => th.text())).toEqual([
      'Roteador',
      'Banda',
      'Canal atual',
      'Canal sugerido',
    ])
  })
})

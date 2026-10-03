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
        labels: {
          changes: 'Mudanças',
          change: 'Mudança',
          detail: 'Detalhe',
          commands: 'Comandos (técnico)',
          ssid: 'Rede',
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
})

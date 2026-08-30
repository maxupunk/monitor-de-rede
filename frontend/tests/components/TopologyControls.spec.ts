import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import TopologyControls from '@/components/topology/TopologyControls.vue'

describe('TopologyControls.vue', () => {
  it('renderiza corretamente com props padrão', () => {
    const wrapper = mount(TopologyControls, {
      props: {
        zoomLevel: 1.25,
        isConnectMode: false,
        activeTypeFilter: null,
        recalculating: false,
        canWrite: true,
      },
      global: {
        stubs: {
          'v-tooltip': { template: '<div><slot name="activator" :props="{}" /></div>' },
          'v-btn': {
            template: '<button @click="$emit(\'click\')"><slot /></button>',
          },
          'v-icon': { template: '<i><slot /></i>' },
          'v-divider': { template: '<hr />' },
          'v-badge': { template: '<span><slot /></span>' },
          'v-menu': { template: '<div><slot name="activator" :props="{}" /><slot /></div>' },
          'v-list': { template: '<div><slot /></div>' },
          'v-list-item': { template: '<div @click="$emit(\'click\')"><slot /></div>' },
          'v-card': { template: '<div><slot /></div>' },
          'v-chip': { template: '<div><slot /></div>' },
        },
      },
    })

    expect(wrapper.text()).toContain('125%')
    expect(wrapper.text()).toContain('Conexão')
  })

  it('exibe indicador de modo leitura quando canWrite for false', () => {
    const wrapper = mount(TopologyControls, {
      props: {
        zoomLevel: 1,
        isConnectMode: false,
        canWrite: false,
      },
      global: {
        stubs: {
          'v-tooltip': { template: '<div><slot name="activator" :props="{}" /></div>' },
          'v-btn': { template: '<button><slot /></button>' },
          'v-icon': { template: '<i><slot /></i>' },
          'v-divider': { template: '<hr />' },
          'v-badge': { template: '<span><slot /></span>' },
          'v-menu': { template: '<div><slot name="activator" :props="{}" /><slot /></div>' },
          'v-list': { template: '<div><slot /></div>' },
          'v-list-item': { template: '<div><slot /></div>' },
          'v-card': { template: '<div><slot /></div>' },
          'v-chip': { template: '<div><slot /></div>' },
        },
      },
    })

    expect(wrapper.text()).toContain('Modo Leitura')
    expect(wrapper.text()).not.toContain('Conexão')
  })
})

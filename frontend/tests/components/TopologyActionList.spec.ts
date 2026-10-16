import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import TopologyActionList from '@/components/topology/TopologyActionList.vue'
import TopologyStatusBar from '@/components/topology/TopologyStatusBar.vue'
import type { ResolvedAction } from '@/utils/topology/inspectorRegistry'

const stubs = {
  'v-list': { template: '<div><slot /></div>' },
  'v-list-subheader': { template: '<h6><slot /></h6>' },
  'v-divider': { template: '<hr />' },
  'v-tooltip': { template: '<div><slot name="activator" :props="{}" /></div>' },
  'v-list-item': {
    props: ['title', 'disabled'],
    emits: ['click'],
    template:
      '<button class="item" :disabled="disabled" @click="$emit(\'click\')">{{ title }}<slot name="append" /></button>',
  },
  'v-icon': { template: '<i><slot /></i>' },
}

function action(patch: Partial<ResolvedAction>): ResolvedAction {
  return {
    id: 'a',
    title: 'Ação',
    icon: 'mdi-lan',
    group: 'navigate',
    disabledReason: null,
    run: vi.fn(),
    ...patch,
  }
}

describe('TopologyActionList.vue', () => {
  it('agrupa por seção e executa a ação habilitada', async () => {
    const run = vi.fn()
    const wrapper = mount(TopologyActionList, {
      props: {
        actions: [
          action({ id: 'focus', title: 'Centralizar', run }),
          action({ id: 'del', title: 'Excluir', group: 'danger', shortcut: 'Del' }),
        ],
      },
      global: { stubs },
    })
    expect(wrapper.findAll('h6').map((h) => h.text())).toEqual(['Navegar', 'Remover'])
    await wrapper.findAll('.item')[0].trigger('click')
    expect(run).toHaveBeenCalledTimes(1)
    expect(wrapper.emitted('ran')?.[0]).toEqual(['focus'])
  })

  it('ação desabilitada (perfil leitura) não roda', async () => {
    const run = vi.fn()
    const wrapper = mount(TopologyActionList, {
      props: { actions: [action({ id: 'edit', title: 'Editar', run, disabledReason: 'leitura' })] },
      global: { stubs },
    })
    const item = wrapper.find('.item')
    expect(item.attributes('disabled')).toBeDefined()
    await item.trigger('click')
    expect(run).not.toHaveBeenCalled()
  })
})

describe('TopologyStatusBar.vue', () => {
  const summary = { nodes: 58, edges: 57, offline: 3, warning: 2, linksDown: 1, saturated: 0 }

  it('resume a rede e alterna "só problemas"', async () => {
    const wrapper = mount(TopologyStatusBar, {
      props: { summary, problemsOnly: false },
      global: { stubs: { 'v-icon': true } },
    })
    expect(wrapper.text()).toContain('58 dispositivos')
    expect(wrapper.text()).toContain('3 offline')
    expect(wrapper.text()).not.toContain('saturados')
    await wrapper.find('button.problems').trigger('click')
    expect(wrapper.emitted('toggle-problems')).toHaveLength(1)
  })

  it('sem problema nenhum, diz que está tudo operando', () => {
    const wrapper = mount(TopologyStatusBar, {
      props: { summary: { ...summary, offline: 0, warning: 0, linksDown: 0 }, problemsOnly: false },
      global: { stubs: { 'v-icon': true } },
    })
    expect(wrapper.text()).toContain('Tudo operando')
  })
})

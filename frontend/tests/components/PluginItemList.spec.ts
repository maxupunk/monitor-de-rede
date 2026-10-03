import { describe, expect, it } from 'vitest'
import { defineComponent, h } from 'vue'
import { mount } from '@vue/test-utils'
import type { ItemList } from '@/bindings/ItemList'
import PluginItemList from '@/components/plugins/items/PluginItemList.vue'
import type { ItemRow } from '@/utils/itemList'

const passthrough = { template: '<div><slot /><slot name="append" /></div>' }
const stubs = {
  'v-row': passthrough,
  'v-col': passthrough,
  'v-card': {
    emits: ['click'],
    template: '<div class="card" @click="$emit(\'click\')"><slot /></div>',
  },
  'v-card-item': passthrough,
  'v-card-title': passthrough,
  'v-card-subtitle': passthrough,
  'v-card-text': passthrough,
  'v-avatar': passthrough,
  'v-icon': { template: '<i><slot /></i>' },
  'v-chip': { template: '<span class="chip"><slot /></span>' },
  'v-spacer': { template: '<span />' },
  'v-progress-linear': { template: '<div />' },
  'v-text-field': { template: '<input />' },
  'v-menu': { template: '<div><slot name="activator" :props="{}" /><slot /></div>' },
  'v-list': passthrough,
  'v-list-item': {
    props: ['title'],
    emits: ['click'],
    template: '<button class="menu-item" @click="$emit(\'click\')">{{ title }}</button>',
  },
  'v-btn': {
    emits: ['click'],
    template: '<button class="btn" @click="$emit(\'click\')"><slot /></button>',
  },
  'v-data-table': defineComponent({
    props: { items: { type: Array, default: () => [] } },
    emits: ['click:row'],
    setup(props, { emit }) {
      return () =>
        h(
          'div',
          (props.items as { key: string }[]).map((item) =>
            h(
              'button',
              { class: 'row', onClick: (event: Event) => emit('click:row', event, { item }) },
              item.key
            )
          )
        )
    },
  }),
}

function row(
  key: string,
  item: Record<string, unknown>,
  entries: ItemRow['entries'] = []
): ItemRow {
  return { key, item, subtitle: '', active: true, entries, detailTotal: null }
}

describe('PluginItemList.vue', () => {
  it('em cartões, o clique edita, o menu remove e o cartão diz quem tem o item', async () => {
    const list: ItemList = {
      title: 'Redes',
      itemName: 'rede',
      key: 'ssid',
      layout: 'cards',
      add: 'network',
      edit: { action: 'network', params: { ssid: 'ssid' } },
      remove: { action: 'remove_network', params: { ssid: 'ssid' } },
    }
    const wrapper = mount(PluginItemList, {
      props: {
        list,
        rows: [row('Loja', { ssid: 'Loja' }, [{ deviceId: 2, name: 'AP Sala', state: 'ativa' }])],
        actions: [
          { id: 'network', title: 'Rede', effect: 'write' },
          { id: 'remove_network', title: 'Remover rede', effect: 'write' },
        ],
        canWrite: true,
        members: 3,
      },
      global: { stubs },
    })

    expect(wrapper.text()).toContain('Adicionar rede')
    expect(wrapper.text()).toContain('Em 1 de 3 equipamento(s)')
    expect(wrapper.findAll('.chip').map((chip) => chip.text())).toContain('AP Sala')
    await wrapper.find('.card').trigger('click')
    expect(wrapper.emitted('edit')?.[0]?.[0]).toMatchObject({ key: 'Loja' })
    const remover = wrapper.findAll('.menu-item').find((item) => item.text() === 'Remover')
    await remover?.trigger('click')
    expect(wrapper.emitted('remove')).toHaveLength(1)
  })

  it('em tabela, o item escolhido mostra só os botões que valem para ele', async () => {
    const list: ItemList = {
      title: 'Pacotes',
      key: 'name',
      layout: 'table',
      source: 'list_packages',
      rowActions: [
        {
          action: 'install_package',
          label: 'Instalar',
          params: { name: 'name' },
          hideWhen: 'installed',
        },
        {
          action: 'remove_package',
          label: 'Desinstalar',
          params: { name: 'name' },
          showWhen: 'installed',
        },
      ],
    }
    const wrapper = mount(PluginItemList, {
      props: {
        list,
        rows: [row('usteer', { name: 'usteer', installed: false })],
        actions: [
          { id: 'install_package', title: 'Instalar pacote', effect: 'write' },
          { id: 'remove_package', title: 'Remover pacote', effect: 'write' },
        ],
        canWrite: true,
      },
      global: { stubs },
    })

    await wrapper.find('.row').trigger('click')
    const labels = wrapper.findAll('.btn').map((button) => button.text())
    expect(labels).toContain('Instalar')
    expect(labels).not.toContain('Desinstalar')
    await wrapper
      .findAll('.btn')
      .find((button) => button.text() === 'Instalar')
      ?.trigger('click')
    expect(wrapper.emitted('action')?.[0]?.[0]).toMatchObject({ action: 'install_package' })
  })
})

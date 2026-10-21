import { describe, expect, it, vi } from 'vitest'
import { defineComponent } from 'vue'
import {
  EDIT_MODE_REASON,
  groupActions,
  READ_ONLY_REASON,
  registerActionProvider,
  registerInspectorTab,
  resolveActions,
  resolveTabs,
  type TopologyHost,
  type TopologyTarget,
} from '@/utils/topology/inspectorRegistry'
import {
  backgroundActions,
  edgeActions,
  edgeLinkTestRequest,
  linkTestActions,
  nodeActions,
  pluginActionsProvider,
} from '@/utils/topology/builtinActions'
import { edge, node } from './fixtures'

function host(patch: Partial<TopologyHost> = {}): TopologyHost {
  return {
    canWrite: true,
    editMode: false,
    selectedNodeId: null,
    systemNodeId: 1,
    nodeById: () => undefined,
    openDevice: vi.fn(),
    editNode: vi.fn(),
    deleteNode: vi.fn(),
    connectFrom: vi.fn(),
    startCable: vi.fn(),
    editEdge: vi.fn(),
    deleteEdge: vi.fn(),
    confirmEdge: vi.fn(),
    focusNode: vi.fn(),
    selectEdge: vi.fn(),
    highlightPath: vi.fn(),
    requestLinkTest: vi.fn(),
    openInspectorTab: vi.fn(),
    runPluginAction: vi.fn(),
    addUnmanagedSwitch: vi.fn(),
    discoverLinks: vi.fn(),
    addLink: vi.fn(),
    addDevice: vi.fn(),
    fit: vi.fn(),
    ...patch,
  }
}

const nodeTarget: TopologyTarget = { kind: 'node', node: node(5, { name: 'SW' }) }

describe('registro de ações e abas da topologia', () => {
  it('extensões entram no menu sem editar o inspetor; cancelar remove', () => {
    const off = registerActionProvider('teste.extra', (target) =>
      target.kind === 'node'
        ? [{ id: 'x.ping', title: 'Ping', icon: 'mdi-lan', group: 'test', run: () => undefined }]
        : []
    )
    expect(resolveActions(nodeTarget, host()).some((a) => a.id === 'x.ping')).toBe(true)
    off()
    expect(resolveActions(nodeTarget, host()).some((a) => a.id === 'x.ping')).toBe(false)
  })

  it('perfil somente leitura desabilita (com motivo) o que escreve', () => {
    const off = registerActionProvider('builtin.node', nodeActions)
    const actions = resolveActions(nodeTarget, host({ canWrite: false }))
    const remove = actions.find((a) => a.id === 'node.delete')
    expect(remove?.disabledReason).toBe(READ_ONLY_REASON)
    expect(actions.find((a) => a.id === 'node.focus')?.disabledReason).toBeNull()
    const cable = resolveActions(nodeTarget, host()).find((a) => a.id === 'node.cable')
    expect(cable?.disabledReason).toBe(EDIT_MODE_REASON)
    off()
  })

  it('ordena por grupo: navegar, editar, testes, plugins, remover', () => {
    const offs = [
      registerActionProvider('builtin.node', nodeActions),
      registerActionProvider('builtin.link-tests', linkTestActions),
    ]
    const groups = groupActions(resolveActions(nodeTarget, host())).map((g) => g.group)
    expect(groups).toEqual(['navigate', 'edit', 'test', 'danger'])
    offs.forEach((off) => off())
  })

  it('pedidos de teste de enlace saem com o alvo certo', () => {
    const h = host({ selectedNodeId: 9 })
    const link = linkTestActions({ kind: 'edge', edge: edge(30, 2, 3) }, h)
    void link[0].run()
    expect(h.requestLinkTest).toHaveBeenCalledWith({
      kind: 'link',
      linkId: 30,
      sourceDeviceId: 2,
      targetDeviceId: 3,
    })
    const nodeEntries = linkTestActions(nodeTarget, h)
    // "Testar daqui até…": o equipamento é a origem; o destino é escolhido.
    void nodeEntries.find((a) => a.id === 'test.from')!.run()
    expect(h.requestLinkTest).toHaveBeenCalledWith({ kind: 'end_to_end', sourceDeviceId: 5 })
    void nodeEntries.find((a) => a.id === 'test.server')!.run()
    expect(h.requestLinkTest).toHaveBeenCalledWith({
      kind: 'server',
      targetDeviceId: 5,
      sourceDeviceId: 1,
    })
    // A hierarquia declarada também se mede (o backend aceita `linkId` negativo).
    const parent = linkTestActions(
      { kind: 'edge', edge: edge(-5, 1, 2, { linkType: 'parent' }) },
      h
    )
    void parent.find((a) => a.id === 'test.link')!.run()
    expect(h.requestLinkTest).toHaveBeenCalledWith({
      kind: 'link',
      linkId: -5,
      sourceDeviceId: 1,
      targetDeviceId: 2,
    })
  })

  it('enlace com um switch sem IP abre o assistente na ponta com IP', () => {
    const nodes = new Map([
      [2, node(2, { name: 'AP-terraço', ipAddress: '10.0.0.3' })],
      [3, node(3, { name: 'switch-assistencia', type: 'unmanaged_switch', ipAddress: null })],
      [4, node(4, { name: 'switch-b', type: 'unmanaged_switch', ipAddress: null })],
    ])
    const h = host({ nodeById: (id) => nodes.get(id) })
    const [action] = linkTestActions({ kind: 'edge', edge: edge(30, 3, 2) }, h)
    expect(action.subtitle).toBe('Passa pelo switch sem IP — escolha o destino')
    void action.run()
    // Sem `linkId`: o destino (depois do switch) é escolhido no assistente.
    expect(h.requestLinkTest).toHaveBeenCalledWith({ kind: 'link', sourceDeviceId: 2 })
    expect(edgeLinkTestRequest(edge(31, 3, 4), (id) => nodes.get(id))).toBeNull()
    // Duas pontas com IP: o enlace direto de sempre.
    nodes.set(5, node(5, { ipAddress: '10.0.0.5' }))
    expect(edgeLinkTestRequest(edge(32, 2, 5), (id) => nodes.get(id))).toEqual({
      kind: 'link',
      linkId: 32,
      sourceDeviceId: 2,
      targetDeviceId: 5,
    })
  })

  it('enlace descoberto oferece "Confirmar"; hierarquia vira "Desvincular"', () => {
    const discovered = edgeActions(
      { kind: 'edge', edge: edge(7, 1, 2, { discoveryMethod: 'lldp', confirmed: false }) },
      host()
    )
    expect(discovered.some((a) => a.id === 'edge.confirm')).toBe(true)
    const parent = edgeActions(
      { kind: 'edge', edge: edge(-2, 1, 2, { linkType: 'parent' }) },
      host()
    )
    expect(parent.find((a) => a.id === 'edge.delete')?.title).toBe('Desvincular hierarquia')
    expect(parent.some((a) => a.id === 'edge.confirm')).toBe(false)
  })

  it('ações de plugin do nó; escrita exige perfil com escrita', () => {
    const h = host()
    const provider = pluginActionsProvider(() => ({
      5: [
        {
          pluginId: 3,
          pluginSlug: 'openwrt',
          pluginName: 'OpenWrt',
          action: 'reboot',
          title: 'Reiniciar',
          effect: 'write',
          hasParams: false,
        },
      ],
    }))
    const [action] = provider(nodeTarget, h)
    expect(action.requiresWrite).toBe(true)
    expect(action.subtitle).toBe('OpenWrt')
    void action.run()
    expect(h.runPluginAction).toHaveBeenCalled()
  })

  it('fundo do mapa: "Cadastrar dispositivo…" primeiro, cadastrando no próprio mapa', () => {
    const h = host()
    const actions = backgroundActions({ kind: 'background' }, h)
    const edits = actions
      .filter((a) => a.group === 'edit')
      .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    expect(edits[0].id).toBe('map.add-device')
    expect(edits[0].requiresWrite).toBe(true)
    void edits[0].run()
    expect(h.addDevice).toHaveBeenCalledOnce()
  })

  it('abas: extensão entra na ordem e só para o tipo pedido', () => {
    const Tab = defineComponent({ template: '<div />' })
    const off = registerInspectorTab({
      id: 'link-tests',
      title: 'Testes',
      icon: 'mdi-speedometer',
      appliesTo: ['edge'],
      order: 30,
      component: Tab,
    })
    expect(resolveTabs({ kind: 'edge', edge: edge(1, 1, 2) }).map((t) => t.id)).toContain(
      'link-tests'
    )
    expect(resolveTabs(nodeTarget).map((t) => t.id)).not.toContain('link-tests')
    expect(resolveTabs({ kind: 'background' })).toEqual([])
    off()
  })
})

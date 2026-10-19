import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia, type Pinia } from 'pinia'
import { createMemoryHistory, createRouter } from 'vue-router'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
import { apiService } from '@/services/apiService'
import { useAuthStore } from '@/stores/auth'
import { useLinkTestsStore } from '@/stores/linkTests'
import LinkTestRunPanel from '@/components/linkTests/LinkTestRunPanel.vue'
import LinkTestPlanView from '@/components/linkTests/LinkTestPlanView.vue'
import LinkTestWizard from '@/components/linkTests/LinkTestWizard.vue'
import LinkTestTargetFields from '@/components/linkTests/LinkTestTargetFields.vue'
import LinkTestViaChip from '@/components/linkTests/LinkTestViaChip.vue'
import { useDevicesStore } from '@/stores/devices'
import { useTopologyStore } from '@/stores/topology'
import { edge, node } from '../utils/topology/fixtures'
import DeviceAccessHost from '@/components/devices/access/DeviceAccessHost.vue'
import { settleAccess } from '@/composables/useDeviceAccess'
import { useConfirm } from '@/composables/useConfirm'
import { usePluginsStore } from '@/stores/plugins'
import type { PluginRunView } from '@/bindings/PluginRunView'
import {
  accessFix,
  endpoint,
  installFix,
  leg,
  packageFix,
  plan,
  stage,
  test,
} from '../utils/linkTests/fixtures'

class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

function vuetify() {
  return createVuetify({ components, directives })
}

let pinia: Pinia

function setRole(role: 'admin' | 'operator' | 'viewer') {
  const auth = useAuthStore()
  auth.user = { id: 1, name: 'Ana', email: 'a@b.c', role } as never
}

function router() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/:path(.*)*', component: { template: '<div />' } }],
  })
}

function mountWith<T>(component: T, props: Record<string, unknown>) {
  return mount(component as never, {
    props,
    attachTo: document.body,
    global: { plugins: [pinia, vuetify(), router()] },
  })
}

describe('telas do teste de enlace', () => {
  beforeEach(() => {
    pinia = createPinia()
    setActivePinia(pinia)
    vi.stubGlobal('ResizeObserver', FakeResizeObserver)
    vi.stubGlobal('visualViewport', {
      width: 1280,
      height: 800,
      offsetLeft: 0,
      offsetTop: 0,
      addEventListener() {},
      removeEventListener() {},
    })
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
    })
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
    )
  })

  afterEach(() => {
    document.body.innerHTML = ''
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('painel ao vivo: etapas, Mbps da amostra e "Cancelar teste" à vista', async () => {
    setRole('admin')
    const store = useLinkTestsStore()
    const running = test(1, {
      legs: [
        leg(1, {
          status: 'running',
          stages: [
            stage('serve', { status: 'succeeded', label: 'Subir servidor', runId: 77 }),
            stage('forward', { status: 'running', label: 'Ida' }),
            stage('reverse', { label: 'Volta' }),
          ],
        }),
      ],
    })
    store.applyUpdated(running)
    store.applySample({ testId: 1, legSeq: 1, flow: 'forward', bps: 487e6, tMs: 1000 })
    const wrapper = mountWith(LinkTestRunPanel, { test: store.byId(1) })
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('Em andamento')
    expect(text).toContain('Dev-1 → Dev-2')
    expect(text).toContain('487 Mbps')
    expect(text).toContain('Subir servidor')
    expect(text).toContain('O enlace fica saturado')
    const cancel = wrapper.findAll('button').find((b) => b.text().includes('Cancelar teste'))
    expect(cancel?.attributes('disabled')).toBeUndefined()
    // Anúncio acessível da etapa atual.
    expect(wrapper.find('[aria-live="polite"]').text()).toContain('perna 1 de 1')
  })

  it('painel concluído: gargalo explicado e tabela por perna', async () => {
    setRole('viewer')
    const done = test(2, {
      kind: 'path',
      status: 'succeeded',
      finishedAt: '2026-10-16T12:00:40Z',
      legs: [
        leg(1, { status: 'succeeded', forwardBps: 900e6, reverseBps: 880e6 }),
        leg(2, { status: 'succeeded', forwardBps: 87e6, reverseBps: 85e6 }),
      ],
      summary: { bottleneckLegSeq: 2, minForwardBps: 87e6, minReverseBps: 85e6 },
    })
    const wrapper = mountWith(LinkTestRunPanel, { test: done })
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('Teste concluído')
    expect(text).toContain('O gargalo é Dev-2 → Dev-3')
    expect(wrapper.findAll('tbody tr')).toHaveLength(2)
    expect(wrapper.find('tr.bottleneck').text()).toContain('Dev-2 → Dev-3')
    expect(text).not.toContain('Cancelar teste')
    // Hora absoluta (sem relógio, um "há 0s" ficaria congelado).
    expect(text).toContain('terminou em')
    expect(text).not.toContain('há 0s')
    // Sem a velocidade da porta, nenhum selo repete o número do medidor.
    expect(wrapper.findAll('.gauge .v-chip')).toHaveLength(0)
  })

  it('plano: bloqueio com dica acionável, aviso de saturação e o pacote que falta', async () => {
    setRole('admin')
    const p = plan({
      blocking: ['MikroTik-Torre não tem IP cadastrado'],
      fixes: [{ ...packageFix(), deviceId: 1, deviceName: 'OpenWrt-Sala' }],
      warnings: ['O cliente btest limita a 29 s por sentido.'],
    })
    p.legs[0].a = endpoint(1, 'OpenWrt-Sala')
    p.legs[0].b = endpoint(2, 'MikroTik-Torre')
    const wrapper = mountWith(LinkTestPlanView, {
      plan: p,
      spec: { durationS: 5, direction: 'both', streams: 1, limitMbps: null },
    })
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('O teste não pode rodar assim')
    expect(text).toContain('satura o enlace durante a medição')
    expect(text).toContain('Instalar o iperf3 em OpenWrt-Sala')
    expect(text).toContain('29 s por sentido')
    expect(wrapper.find('a[href="/devices/2"]').exists()).toBe(true)
  })

  it('plano: perna através do switch sem IP mostra as pontas com IP e o chip "via"', async () => {
    setRole('admin')
    const p = plan()
    p.legs[0] = {
      ...p.legs[0],
      a: endpoint(1, 'AP-terraço'),
      b: endpoint(3, 'BORDA'),
      edgeId: null,
      edgeIds: [11, 12],
      viaDeviceIds: [2],
      viaNames: ['switch-assistencia'],
    }
    const wrapper = mountWith(LinkTestPlanView, {
      plan: p,
      spec: { durationS: 5, direction: 'both', streams: 1, limitMbps: null },
    })
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('AP-terraço')
    expect(text).toContain('BORDA')
    expect(text).toContain('via switch-assistencia')
  })

  it('chip "via": cadeia longa encurta e o texto inteiro fica na dica e no rótulo acessível', () => {
    const names = ['switch-assistencia', 'switch-b', 'switch-do-galpão-dos-fundos']
    const wrapper = mountWith(LinkTestViaChip, { leg: { viaNames: names } })
    const chip = wrapper.find('.via-chip')
    const full = `Passa por ${names.join(' › ')} (sem IP): a perna mede os cabos entre as duas pontas`
    expect(chip.text()).toBe(`via ${names.join(' › ')}`)
    expect(chip.attributes('aria-label')).toBe(full)
    expect(chip.attributes('title')).toBe(full)
    expect(chip.find('.via-text').exists()).toBe(true)
    // Contorno na cor viva: não se confunde com o chip cheio do motor btest.
    expect(chip.classes()).toContain('v-chip--variant-outlined')
    expect(chip.classes()).toContain('text-secondary')
    expect(
      mountWith(LinkTestViaChip, { leg: { viaNames: [] } })
        .find('.v-chip')
        .exists()
    ).toBe(false)
  })

  it('"Enlace": o switch sem IP some do seletor e vira "até BORDA, passando por…"', async () => {
    setRole('admin')
    vi.spyOn(apiService, 'get').mockResolvedValue([])
    const topology = useTopologyStore()
    topology.nodes = [
      node(1, { name: 'AP-terraço', ipAddress: '10.0.0.3' }),
      node(2, { name: 'switch-assistencia', type: 'unmanaged_switch', ipAddress: null }),
      node(3, { name: 'BORDA', ipAddress: '10.0.0.1' }),
    ]
    topology.edges = [edge(11, 1, 2), edge(12, 2, 3)]
    topology.loaded = true
    const devices = useDevicesStore()
    devices.devices = [
      { id: 1, name: 'AP-terraço', type: 'router', ipAddress: '10.0.0.3' },
      { id: 2, name: 'switch-assistencia', type: 'unmanaged_switch' },
      { id: 3, name: 'BORDA', type: 'router', ipAddress: '10.0.0.1' },
    ] as never
    const wrapper = mountWith(LinkTestTargetFields, {
      modelValue: { kind: 'link', linkId: null, sourceDeviceId: 1, targetDeviceId: null },
    })
    await flushPromises()
    expect(wrapper.text()).toContain('Switches sem IP não entram no teste')

    const select = wrapper.findAll('.v-select .v-field')
    await select[0].trigger('mousedown')
    await flushPromises()
    const body = document.body.textContent ?? ''
    expect(body).toContain('até BORDA')
    expect(body).toContain('passando por switch-assistencia')
    expect(body).not.toContain('até switch-assistencia')
    // O "passando por…" é parte da escolha: opacidade cheia no subtítulo.
    const list = document.body.querySelector('.v-overlay-container .v-list') as HTMLElement
    expect(list.style.getPropertyValue('--v-list-item-subtitle-opacity')).toBe('1')

    const option = [...document.body.querySelectorAll('.v-list-item')].find((item) =>
      item.textContent?.includes('até BORDA')
    ) as HTMLElement
    option.click()
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([
      { kind: 'link', linkId: null, sourceDeviceId: 1, targetDeviceId: 3 },
    ])
  })

  it('plano que não roda: sem tempo estimado nem aviso de saturação', async () => {
    setRole('admin')
    const p = plan({ blocking: ['Switch não tem plugin de teste de enlace ativo e instalado'] })
    p.legs[0].runnable = false
    const wrapper = mountWith(LinkTestPlanView, {
      plan: p,
      spec: { durationS: 5, direction: 'both', streams: 1, limitMbps: null },
    })
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('O teste não pode rodar assim')
    expect(text).toContain('0 de 1 vão rodar')
    expect(text).not.toContain('satura o enlace')
    expect(text).not.toContain('alguns segundos')
  })

  it('assistente: pedido do mapa começa no "Como"; plano carregado ao avançar; inicia com confirm', async () => {
    setRole('admin')
    vi.spyOn(apiService, 'get').mockResolvedValue([])
    const post = vi
      .spyOn(apiService, 'post')
      .mockResolvedValueOnce(plan())
      .mockResolvedValueOnce({ id: 33 })
    const wrapper = mountWith(LinkTestWizard, {
      modelValue: true,
      request: { kind: 'server', targetDeviceId: 2 },
    })
    await flushPromises()
    const body = () => document.body.textContent ?? ''
    expect(body()).toContain('Duração por sentido')

    const button = (label: string) =>
      [...document.body.querySelectorAll('button')].find((b) => b.textContent?.includes(label))
    button('Ver o plano')?.click()
    await flushPromises()
    expect(post).toHaveBeenNthCalledWith(
      1,
      '/link-tests/plan',
      expect.objectContaining({ kind: 'server', targetDeviceId: 2 }),
      { timeoutMs: 30_000 }
    )
    expect(body()).toContain('Ordem das medições')

    button('Iniciar teste')?.click()
    await flushPromises()
    expect(post).toHaveBeenLastCalledWith(
      '/link-tests',
      expect.objectContaining({ kind: 'server', targetDeviceId: 2, confirm: true }),
      { timeoutMs: 30_000 }
    )
    expect(wrapper.emitted('started')?.[0]).toEqual([33])
  })

  it('assistente: perfil somente leitura não pede o plano (o backend recusa) nem inicia', async () => {
    setRole('viewer')
    vi.spyOn(apiService, 'get').mockResolvedValue([])
    const post = vi.spyOn(apiService, 'post').mockResolvedValueOnce(plan())
    mountWith(LinkTestWizard, { modelValue: true, request: { kind: 'server', targetDeviceId: 2 } })
    await flushPromises()
    expect(document.body.textContent).toContain('somente leitura')
    const button = (label: string) =>
      [...document.body.querySelectorAll('button')].find((b) => b.textContent?.includes(label))
    expect(button('Ver o plano')?.hasAttribute('disabled')).toBe(true)
    button('Ver o plano')?.click()
    await flushPromises()
    expect(post).not.toHaveBeenCalled()
    expect(button('Iniciar teste')).toBeUndefined()
  })

  it('assistente: o plano oferece instalar e cadastrar o acesso ali mesmo, e se refaz a cada passo', async () => {
    setRole('admin')
    const waiting = () => {
      const p = plan({ fixes: [installFix(), accessFix()] })
      p.legs[0].runnable = false
      p.legs[0].b = endpoint(7, 'AP-terraço')
      p.legs[0].issues = [installFix().message, accessFix().message]
      p.legs[0].fixes = [installFix(), accessFix()]
      return p
    }
    const onlyAccess = () => {
      const p = waiting()
      p.fixes = [accessFix()]
      p.legs[0].issues = [accessFix().message]
      p.legs[0].fixes = [accessFix()]
      return p
    }
    vi.spyOn(apiService, 'get').mockImplementation(async (path: string) =>
      path === '/devices/7/access'
        ? { deviceId: 7, deviceName: 'AP-terraço', credentials: [], agents: [] }
        : []
    )
    const post = vi.spyOn(apiService, 'post').mockImplementation(async (path: string) => {
      if (path === '/devices/7/plugins/30/install') return { deviceId: 7 }
      throw new Error(`inesperado: ${path}`)
    })
    post
      .mockResolvedValueOnce(waiting())
      .mockResolvedValueOnce({ deviceId: 7 })
      .mockResolvedValueOnce(onlyAccess())
      .mockResolvedValueOnce(plan())
    const put = vi.spyOn(apiService, 'put').mockResolvedValue({
      kind: 'ssh',
      username: 'root',
      storage: 'vault',
      port: 22,
      https: false,
      viaProbeId: null,
      hasStoredSecret: true,
      sessionActive: false,
    })
    mountWith(DeviceAccessHost, {})
    mountWith(LinkTestWizard, { modelValue: true, request: { kind: 'server', targetDeviceId: 7 } })
    await flushPromises()
    const body = () => document.body.textContent ?? ''
    const button = (label: string) =>
      [...document.body.querySelectorAll('button')].find((b) => b.textContent?.includes(label))

    button('Ver o plano')?.click()
    await flushPromises()
    expect(body()).toContain('Antes de testar, falta')
    expect(body()).toContain('Instalar o plugin «OpenWrt — teste de enlace» em AP-terraço')
    expect(body()).toContain('Cadastrar o acesso SSH de AP-terraço')
    // Falta um passo não é erro: nada de "O teste não pode rodar assim".
    expect(body()).toContain('Falta um passo')
    expect(body()).not.toContain('O teste não pode rodar assim')
    expect(button('Iniciar teste')?.hasAttribute('disabled')).toBe(true)

    button('Instalar agora')?.click()
    await flushPromises()
    expect(post).toHaveBeenCalledWith('/devices/7/plugins/30/install', {})
    // Refez o plano sozinho; o passo feito continua marcado.
    expect(post).toHaveBeenCalledTimes(3)
    expect(body()).toContain('Feito.')

    button('Cadastrar acesso')?.click()
    await flushPromises()
    expect(body()).toContain('Acesso SSH')
    const password = document.body.querySelector<HTMLInputElement>('input[type="password"]')
    expect(password).not.toBeNull()
    ;(password as HTMLInputElement).value = 'segredo'
    password?.dispatchEvent(new Event('input'))
    await flushPromises()
    button('Salvar acesso')?.click()
    await flushPromises()
    expect(put).toHaveBeenCalledWith(
      '/devices/7/credentials',
      expect.objectContaining({ kind: 'ssh', secret: 'segredo' })
    )
    expect(post).toHaveBeenCalledTimes(4)
    expect(body()).toContain('Pronto: o que faltava foi feito')
    expect(button('Iniciar teste')?.hasAttribute('disabled')).toBe(false)
    // Volta para onde estava: o diálogo de acesso fechou e o assistente
    // continua aberto no mesmo passo (o plano), sem ter saído da tela.
    const accessDialogOpen = [...document.body.querySelectorAll('.v-overlay--active')].some((o) =>
      o.textContent?.includes('Salvar acesso')
    )
    expect(accessDialogOpen).toBe(false)
    expect(body()).toContain('Ordem das medições')
    expect(button('Ver o plano')).toBeUndefined()
    settleAccess(null)
  })

  function packageRun(patch: Partial<PluginRunView> = {}): PluginRunView {
    return {
      id: 55,
      pluginId: 30,
      pluginName: 'OpenWrt – teste de enlace',
      deviceId: 7,
      action: 'tp_prepare',
      origin: 'user',
      status: 'running',
      params: { engine: 'iperf3', role: 'serve' },
      output: null,
      transcript: [],
      error: null,
      createdAt: '2026-10-19T12:00:00Z',
      finishedAt: null,
      ...patch,
    }
  }

  function waitingForPackage() {
    const p = plan({ fixes: [packageFix()] })
    p.legs[0].runnable = false
    p.legs[0].b = endpoint(7, 'AP-terraço')
    p.legs[0].issues = [packageFix().message]
    p.legs[0].fixes = [packageFix()]
    return p
  }

  it('assistente: pacote que falta pede aceite, instala com confirmação, espera o SSE e refaz o plano', async () => {
    setRole('admin')
    const get = vi.spyOn(apiService, 'get').mockImplementation(async (path: string) => {
      if (path === '/plugin-runs/55') return packageRun()
      throw new Error(`inesperado: ${path}`)
    })
    const post = vi.spyOn(apiService, 'post').mockImplementation(async (path: string) => {
      throw new Error(`inesperado: ${path}`)
    })
    post
      .mockResolvedValueOnce(waitingForPackage())
      .mockResolvedValueOnce({ runId: 55 })
      .mockResolvedValueOnce(plan())
    mountWith(LinkTestWizard, { modelValue: true, request: { kind: 'server', targetDeviceId: 7 } })
    await flushPromises()
    const body = () => document.body.textContent ?? ''
    const button = (label: string) =>
      [...document.body.querySelectorAll('button')].find((b) => b.textContent?.trim() === label)

    button('Ver o plano')?.click()
    await flushPromises()
    expect(body()).toContain('Antes de testar, falta')
    expect(body()).toContain('Instalar o iperf3 em AP-terraço')
    expect(body()).toContain('AP-terraço não tem o iperf3')
    expect(body()).toContain('Falta um passo')
    expect(button('Iniciar teste')?.hasAttribute('disabled')).toBe(true)

    // Nada roda sem o aceite: a pergunta explica o que acontece no roteador.
    const dialog = useConfirm()
    button('Instalar')?.click()
    await flushPromises()
    expect(dialog.state.isOpen).toBe(true)
    expect(dialog.state.options.title).toBe('Instalar o iperf3 em AP-terraço?')
    expect(dialog.state.options.message).toContain('Instala o pacote iperf3 em AP-terraço')
    expect(dialog.state.options.message).toContain('não altera a configuração')
    expect(post).toHaveBeenCalledTimes(1)
    dialog.handleConfirm()
    await flushPromises()
    expect(post).toHaveBeenCalledWith('/devices/7/plugins/30/actions/tp_prepare', {
      params: { engine: 'iperf3', role: 'serve' },
      confirmWrite: true,
    })
    expect(body()).toContain(
      'Instalando… o roteador baixa e instala o pacote; pode levar alguns minutos.'
    )

    // O fim chega pelo SSE; o plano é refeito uma vez, sem consultas repetidas.
    usePluginsStore().applyRunFinished({
      run: packageRun({
        status: 'succeeded',
        output: {
          ready: true,
          installed: ['iperf3'],
          warning: 'Instalado pelo espelho HTTP. Recomendo acertar o NTP do roteador.',
        },
      }),
    })
    await flushPromises()
    expect(get).toHaveBeenCalledTimes(1)
    expect(post).toHaveBeenCalledTimes(3)
    expect(body()).toContain('Pronto: o que faltava foi feito')
    // Instalou por um caminho alternativo: o aviso fica no passo, com a execução.
    expect(body()).toContain('Recomendo acertar o NTP do roteador.')
    expect(button('Ver execução')).toBeDefined()
    expect(button('Iniciar teste')?.hasAttribute('disabled')).toBe(false)
  })

  it('assistente: instalação que falha mostra o motivo do plugin e a execução, sem refazer o plano', async () => {
    setRole('admin')
    vi.spyOn(apiService, 'get').mockImplementation(async (path: string) => {
      if (path === '/plugin-runs/55') return packageRun()
      throw new Error(`inesperado: ${path}`)
    })
    const post = vi.spyOn(apiService, 'post')
    post.mockResolvedValueOnce(waitingForPackage()).mockResolvedValueOnce({ runId: 55 })
    mountWith(LinkTestWizard, { modelValue: true, request: { kind: 'server', targetDeviceId: 7 } })
    await flushPromises()
    const body = () => document.body.textContent ?? ''
    const button = (label: string) =>
      [...document.body.querySelectorAll('button')].find((b) => b.textContent?.trim() === label)
    button('Ver o plano')?.click()
    await flushPromises()

    // Desistir na pergunta não roda nada.
    const dialog = useConfirm()
    button('Instalar')?.click()
    await flushPromises()
    dialog.handleCancel()
    await flushPromises()
    expect(post).toHaveBeenCalledTimes(1)

    button('Instalar')?.click()
    await flushPromises()
    dialog.handleConfirm()
    await flushPromises()
    usePluginsStore().applyRunFinished({
      run: packageRun({
        status: 'succeeded',
        output: { ready: false, installed: [], message: 'O roteador não alcança o repositório' },
      }),
    })
    await flushPromises()
    expect(body()).toContain('O roteador não alcança o repositório')
    expect(button('Ver execução')).toBeDefined()
    expect(post).toHaveBeenCalledTimes(2)
    expect(body()).not.toContain('Feito.')
  })

  it('plano visto por operador: os passos dizem quem pode fazê-los', async () => {
    setRole('operator')
    const p = plan({ fixes: [installFix(), accessFix(), packageFix()] })
    p.legs[0].runnable = false
    const wrapper = mountWith(LinkTestPlanView, {
      plan: p,
      spec: { durationS: 5, direction: 'both', streams: 1, limitMbps: null },
    })
    await flushPromises()
    const text = wrapper.text()
    expect(text).toContain('Peça a um administrador para instalar «OpenWrt — teste de enlace»')
    expect(text).toContain('Peça a um administrador para cadastrar o acesso SSH')
    expect(text).toContain('Peça a um administrador para instalar o iperf3 em AP-terraço')
    expect(wrapper.findAll('button').some((b) => b.text().includes('Instalar'))).toBe(false)
  })
})

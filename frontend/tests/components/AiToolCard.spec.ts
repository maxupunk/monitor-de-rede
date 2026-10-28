import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { CredentialView } from '@/bindings/CredentialView'
import type { DeviceAccessView } from '@/bindings/DeviceAccessView'
import AiToolCard from '@/components/ai/AiToolCard.vue'
import DeviceAccessHost from '@/components/devices/access/DeviceAccessHost.vue'
import { settleAccess } from '@/composables/useDeviceAccess'
import { apiService } from '@/services/apiService'
import { useAiStore, type AiToolCallState } from '@/stores/ai'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom, type } from './domHarness'

let pinia: Pinia

const FOLLOW_UP = 'Cadastrei o acesso SSH de AP-terraço; pode tentar de novo.'

function access(): DeviceAccessView {
  return { deviceId: 7, deviceName: 'AP-terraço', credentials: [], agents: [] }
}

function saved(): CredentialView {
  return {
    kind: 'ssh',
    username: 'root',
    storage: 'ask',
    port: 22,
    https: false,
    viaProbeId: null,
    hasStoredSecret: false,
    sessionActive: true,
  }
}

function failedTool(patch: Partial<AiToolCallState> = {}): AiToolCallState {
  return {
    id: 'c1',
    name: 'device_plugin_run',
    arguments: { device_id: 7, action: 'radios' },
    status: 'error',
    result: {
      error: 'Cadastre a credencial SSH deste equipamento para usar o plugin.',
      missingAccess: { deviceId: 7, transport: 'ssh' },
    },
    ...patch,
  }
}

function sseResponse(events: unknown[]): Response {
  const body = events.map((event) => `data: ${JSON.stringify(event)}\n\n`).join('')
  return new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(body))
        controller.close()
      },
    })
  )
}

/** A conversa com a ferramenta que falhou por falta do acesso, e o cartão dela. */
function mountCard(tool = failedTool()) {
  const store = useAiStore()
  store.messages = [
    { id: 'user-1', role: 'user', content: 'Quais canais o AP-terraço usa?' },
    { id: 'asst-1', role: 'assistant', content: '', toolCalls: [tool] },
  ]
  mountWith(DeviceAccessHost, {}, pinia)
  const card = mountWith(
    AiToolCard,
    { tool: store.messages[1]!.toolCalls![0], messageId: 'asst-1' },
    pinia
  )
  return { store, card }
}

async function saveAccessFromCard() {
  button('Cadastrar o acesso (SSH)')?.click()
  await flushPromises()
  const password = document.body.querySelector<HTMLInputElement>('input[type="password"]')
  expect(password).not.toBeNull()
  type(password as HTMLInputElement, 'segredo')
  await flushPromises()
  button('Salvar acesso')?.click()
  await flushPromises()
}

describe('IA: o acesso que faltava, salvo no cartão da ferramenta', () => {
  beforeEach(() => {
    pinia = setupDom()
    vi.spyOn(apiService, 'get').mockResolvedValue(access())
    vi.spyOn(apiService, 'put').mockResolvedValue(saved())
    // A conversa é gravada a cada resposta.
    vi.spyOn(apiService, 'post').mockResolvedValue({
      id: 1,
      title: 'Canais',
      updatedAt: '2026-10-17T10:00:00Z',
      messageCount: 3,
    })
  })

  afterEach(() => {
    settleAccess(null)
    teardownDom()
  })

  it('salvo o acesso, a conversa continua sozinha com uma mensagem do usuário', async () => {
    setRole('admin')
    const stream = vi
      .spyOn(apiService, 'postStream')
      .mockResolvedValue(
        sseResponse([{ type: 'textDelta', content: 'Tentando.' }, { type: 'done' }])
      )
    // Confirmada pelo usuário (o caminho de sempre do acesso da IA ao equipamento).
    const { store, card } = mountCard(
      failedTool({ summary: 'Ler os rádios de AP-terraço (só leitura)' })
    )
    await flushPromises()
    expect(bodyText()).toContain('Cadastre a credencial SSH')

    await saveAccessFromCard()

    expect(apiService.put).toHaveBeenCalledWith(
      '/devices/7/credentials',
      expect.objectContaining({ kind: 'ssh', secret: 'segredo' })
    )
    // Uma mensagem comum do usuário, na mesma conversa.
    const user = store.messages.filter((message) => message.role === 'user')
    expect(user.map((message) => message.content)).toEqual([
      'Quais canais o AP-terraço usa?',
      FOLLOW_UP,
    ])
    expect(stream).toHaveBeenCalledTimes(1)
    const [path, body] = stream.mock.calls[0]!
    expect(path).toBe('/ai/chat/stream')
    expect(body).toEqual(
      expect.objectContaining({
        conversationKey: store.conversationKey,
        messages: expect.arrayContaining([{ role: 'user', content: FOLLOW_UP }]),
      })
    )
    // Só a mensagem: nenhuma ferramenta é executada direto daqui (a próxima
    // chamada da IA passa pela confirmação de sempre).
    expect(apiService.post).not.toHaveBeenCalledWith(
      '/ai/tools/execute',
      expect.anything(),
      expect.anything()
    )
    expect(card.text()).toContain('Pedi ao assistente para tentar de novo')
    expect(button('Cadastrar o acesso')).toBeUndefined()
  })

  it('rodada pelo modo automático também oferece o acesso; "Agora não" não manda nada', async () => {
    setRole('admin')
    const stream = vi.spyOn(apiService, 'postStream')
    const { store } = mountCard(failedTool({ autoApproved: true }))
    await flushPromises()
    expect(bodyText()).toContain('Cadastre a credencial SSH')
    button('Cadastrar o acesso (SSH)')?.click()
    await flushPromises()
    button('Agora não')?.click()
    await flushPromises()
    expect(stream).not.toHaveBeenCalled()
    expect(store.messages).toHaveLength(2)
    expect(button('Cadastrar o acesso (SSH)')).toBeDefined()
  })
})

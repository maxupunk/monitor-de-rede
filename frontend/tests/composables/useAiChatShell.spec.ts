import { describe, expect, it } from 'vitest'
import { aiChatPlaceholder } from '@/composables/useAiChatShell'
import { activeTestsMode, activeTestsPresentation } from '@/components/ai/aiActiveTests'

describe('aiChatPlaceholder', () => {
  it('é curta no celular e cita o @ no resto', () => {
    expect(aiChatPlaceholder(true)).toBe('Pergunte sobre a rede…')
    expect(aiChatPlaceholder(false)).toContain('@')
  })
})

describe('activeTestsMode', () => {
  it('fica desligado sem configurações ou sem testes ativos', () => {
    expect(activeTestsMode(null)).toBe('off')
    expect(activeTestsMode({ allowActiveTools: false, requireToolConfirmation: true })).toBe('off')
  })

  it('separa pedir permissão de automático', () => {
    expect(activeTestsMode({ allowActiveTools: true, requireToolConfirmation: true })).toBe(
      'confirm'
    )
    expect(activeTestsMode({ allowActiveTools: true, requireToolConfirmation: false })).toBe('auto')
  })

  it('usa cor semântica em todos os estados — nunca cinza', () => {
    const colors = [
      activeTestsPresentation(null),
      activeTestsPresentation({ allowActiveTools: true, requireToolConfirmation: true }),
      activeTestsPresentation({ allowActiveTools: true, requireToolConfirmation: false }),
    ].map((presentation) => presentation.color)
    expect(colors).toEqual(['warning', 'info', 'success'])
  })
})

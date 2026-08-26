import { describe, expect, it } from 'vitest'
import { AUTO_ACCESS_MODE, accessModeOptions } from '@/utils/accessMode'

describe('apresentação da forma de acesso automática', () => {
  it('mostra a conclusão efetiva sem transformar auto em declaração', () => {
    const automatic = accessModeOptions({
      mode: 'vpn',
      reason: 'o IP está dentro da faixa do túnel',
    })[0]

    expect(automatic.value).toBe(AUTO_ACCESS_MODE)
    expect(automatic.title).toBe('Túnel VPN')
    expect(automatic.subtitle).toBe(
      'Detectado automaticamente — o IP está dentro da faixa do túnel'
    )
  })
})

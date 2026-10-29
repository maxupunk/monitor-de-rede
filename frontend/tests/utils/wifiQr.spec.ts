import { describe, expect, it } from 'vitest'
import { qrPath, wifiNeedsPassword, wifiQrPayload, wifiQrSecurity } from '@/utils/wifiQr'

describe('QR code do Wi-Fi', () => {
  it('traduz a segurança do roteador para a do QR', () => {
    expect(wifiQrSecurity('psk2')).toBe('WPA')
    expect(wifiQrSecurity('sae-mixed')).toBe('WPA')
    expect(wifiQrSecurity('psk2+ccmp')).toBe('WPA')
    expect(wifiQrSecurity('wep-open')).toBe('WEP')
    expect(wifiQrSecurity('owe')).toBe('nopass')
    expect(wifiQrSecurity('none')).toBe('nopass')
    expect(wifiQrSecurity('')).toBe('nopass')
    expect(wifiNeedsPassword('sae')).toBe(true)
    expect(wifiNeedsPassword('owe')).toBe(false)
  })

  it('monta o texto com a senha e escapa os caracteres do formato', () => {
    expect(wifiQrPayload({ ssid: 'Casa', encryption: 'psk2', password: 'segredo123' })).toBe(
      'WIFI:T:WPA;S:Casa;P:segredo123;;'
    )
    expect(
      wifiQrPayload({ ssid: 'Loja;1', encryption: 'sae', password: 'a:b,c\\d"e', hidden: true })
    ).toBe('WIFI:T:WPA;S:Loja\\;1;P:a\\:b\\,c\\\\d\\"e;H:true;;')
  })

  it('rede sem senha não leva o campo da senha', () => {
    expect(wifiQrPayload({ ssid: 'Visitantes', encryption: 'owe', password: 'ignorada' })).toBe(
      'WIFI:T:nopass;S:Visitantes;;'
    )
  })

  it('desenha um quadrado por módulo escuro', () => {
    expect(
      qrPath([
        [true, false],
        [false, true],
      ])
    ).toBe('M0 0h1v1h-1zM1 1h1v1h-1z')
  })
})

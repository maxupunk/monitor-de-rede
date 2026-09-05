import { describe, expect, it } from 'vitest'
import { discoveryIdentity } from '@/stores/discovery'

describe('discoveryIdentity', () => {
  it('lê a identidade do snapshot SSE', () => {
    expect(
      discoveryIdentity({ data: { identity: { operatingSystem: 'openwrt', label: 'OpenWrt' } } })
    ).toMatchObject({ operatingSystem: 'openwrt', label: 'OpenWrt' })
  })

  it('lê a identidade persistida nos detalhes da descoberta', () => {
    expect(
      discoveryIdentity({
        data: { details: { identity: { operatingSystem: 'other', label: 'Outro sistema' } } },
      })
    ).toMatchObject({ operatingSystem: 'other', label: 'Outro sistema' })
  })
})

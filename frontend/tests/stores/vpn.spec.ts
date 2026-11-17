import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import {
  useVpnStore,
  vpnProfileIcon,
  vpnProfileLabel,
  vpnProfileSupportsQrCode,
  type VpnArtifact,
  type VpnProfileOption,
} from '@/stores/vpn'

const profiles: VpnProfileOption[] = [
  {
    profile: 'future-device',
    label: 'Dispositivo Futuro',
    icon: 'mdi-chip',
    supportsQrCode: true,
  },
]

describe('apresentação dos adapters VPN', () => {
  it('usa o catálogo recebido do backend sem lista fixa no frontend', () => {
    expect(vpnProfileLabel('future-device', profiles)).toBe('Dispositivo Futuro')
    expect(vpnProfileIcon('future-device', profiles)).toBe('mdi-chip')
    expect(vpnProfileSupportsQrCode('future-device', profiles)).toBe(true)
  })

  it('mantém fallback legível para snapshot antigo ou adapter removido', () => {
    expect(vpnProfileLabel('legacy', profiles)).toBe('legacy')
    expect(vpnProfileIcon('legacy', profiles)).toBe('mdi-devices')
    expect(vpnProfileSupportsQrCode('legacy', profiles)).toBe(false)
  })
})

describe('vpn store — política do agente no artefato', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('atualiza AGENT_ALLOW na variante Bash sem perder o .conf com a chave privada', async () => {
    const store = useVpnStore()
    const initial: VpnArtifact = {
      peerId: 12,
      qrSvg: null,
      profile: 'linux',
      label: 'Linux',
      delivery: 'download',
      fileName: 'netmonitor-srv.conf',
      language: 'ini',
      content: '[Interface]\nPrivateKey = CHAVE-SECRETA\n',
      instructions: [],
      supportsQrCode: false,
      summary: [],
      variants: [
        {
          id: 'bash',
          label: 'Script Bash',
          hint: '',
          icon: 'mdi-console',
          fileName: 'netmonitor-vpn.sh',
          language: 'shell',
          content: "AGENT_ALLOW='read,lifecycle'",
          instructions: [],
        },
      ],
      agentInstall: {
        enrollCode: 'nma_' + 'a'.repeat(64),
        policy: { allow: ['read', 'lifecycle'], databaseTargets: '' },
      },
    }
    store.lastArtifact = initial

    const post = vi.spyOn(apiService, 'post').mockResolvedValue({
      agentInstall: {
        enrollCode: initial.agentInstall!.enrollCode,
        policy: { allow: ['read', 'compose'], databaseTargets: '' },
      },
      variants: [
        {
          ...initial.variants[0],
          content: "AGENT_ALLOW='read,compose'",
        },
      ],
    })

    const updated = await store.updatePeerAgentPolicy(
      12,
      initial.agentInstall!.enrollCode,
      { allow: ['read', 'compose'], databaseTargets: '' },
      initial.content
    )

    expect(post).toHaveBeenCalledWith('/vpn/peers/12/agent-policy', {
      enrollCode: initial.agentInstall!.enrollCode,
      policy: { allow: ['read', 'compose'], databaseTargets: '' },
      confContent: initial.content,
    })
    expect(updated?.agentInstall.policy.allow).toEqual(['read', 'compose'])
    expect(store.lastArtifact?.content).toContain('PrivateKey = CHAVE-SECRETA')
    expect(store.lastArtifact?.variants[0]?.content).toBe("AGENT_ALLOW='read,compose'")
  })
})

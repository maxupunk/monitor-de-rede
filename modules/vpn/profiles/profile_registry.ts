import type { VpnDeviceProfile } from '#models/vpn_peer'
import type { VpnProfileGenerator } from './profile_contract.js'
import { MikrotikProfileGenerator } from './mikrotik.js'
import { OpenWrtProfileGenerator } from './openwrt.js'
import { createLinuxGenerator, createMobileGenerator, createWindowsGenerator } from './wg_conf.js'

/**
 * Resolve o gerador correto para cada perfil de equipamento.
 * Quem consome depende apenas da abstração `VpnProfileGenerator` (DIP).
 */
export class ProfileRegistry {
  private generators = new Map<VpnDeviceProfile, VpnProfileGenerator>()

  constructor(generators?: VpnProfileGenerator[]) {
    const defaults = generators ?? [
      new MikrotikProfileGenerator(),
      new OpenWrtProfileGenerator(),
      createLinuxGenerator(),
      createWindowsGenerator(),
      createMobileGenerator(),
    ]

    for (const generator of defaults) {
      this.register(generator)
    }
  }

  register(generator: VpnProfileGenerator): void {
    this.generators.set(generator.profile, generator)
  }

  has(profile: string): profile is VpnDeviceProfile {
    return this.generators.has(profile as VpnDeviceProfile)
  }

  resolve(profile: VpnDeviceProfile): VpnProfileGenerator {
    const generator = this.generators.get(profile)
    if (!generator) {
      throw new Error(`Perfil de equipamento não suportado: ${profile}`)
    }
    return generator
  }

  /** Catálogo exibido nos cards do wizard. */
  list(): Array<{
    profile: VpnDeviceProfile
    label: string
    icon: string
    supportsQrCode: boolean
  }> {
    return [...this.generators.values()].map((generator) => ({
      profile: generator.profile,
      label: generator.label,
      icon: generator.icon,
      supportsQrCode: generator.supportsQrCode,
    }))
  }
}

export const profileRegistry = new ProfileRegistry()

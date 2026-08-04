import crypto from 'node:crypto'
import { DateTime } from 'luxon'
import Probe from '#models/probe'
import { VPN_PROBE_NAME } from './monitor_provisioner.js'

/**
 * Registro idempotente do `vpn-probe` — o agente que compartilha o namespace de
 * rede do container WireGuard e executa ICMP/SNMP dentro do túnel.
 *
 * O token vem de `VPN_PROBE_TOKEN` (o mesmo valor usado pelo container), de modo
 * que a inicialização do servidor já deixa o probe pronto para o heartbeat.
 */
export interface VpnProbeRegistration {
  probe: Probe
  created: boolean
  /** Preenchido apenas quando o token foi gerado aqui. */
  token?: string
}

export class VpnProbeRegistrar {
  static hashToken(rawToken: string): string {
    return crypto.createHash('sha256').update(rawToken).digest('hex')
  }

  /**
   * Cria ou atualiza o probe dedicado.
   * Retorna `null` quando não há token configurado — nesse caso o operador
   * registra manualmente com `node ace vpn:probe-register`.
   */
  async register(rawToken?: string | null): Promise<VpnProbeRegistration | null> {
    const token = rawToken || process.env.VPN_PROBE_TOKEN || null
    if (!token) {
      // Sem token configurado: o operador registra com `node ace vpn:probe-register`.
      return null
    }

    const tokenHash = VpnProbeRegistrar.hashToken(token)
    const existing = await Probe.query().where('name', VPN_PROBE_NAME).first()

    if (existing) {
      existing.tokenHash = tokenHash
      if (existing.status === 'revoked') existing.status = 'pending'
      await existing.save()
      return { probe: existing, created: false }
    }

    const probe = await Probe.create({
      name: VPN_PROBE_NAME,
      tokenHash,
      status: 'pending',
      registeredAt: DateTime.now(),
      configuration: { role: 'vpn', network: 'wireguard' },
    })

    return { probe, created: true }
  }

  /** Versão para CLI: gera o token quando não houver um configurado no ambiente. */
  async registerWithGeneratedToken(): Promise<VpnProbeRegistration> {
    const envToken = process.env.VPN_PROBE_TOKEN || null
    const token = envToken ?? crypto.randomBytes(32).toString('hex')
    const registration = (await this.register(token))!

    return { ...registration, token: envToken ? undefined : token }
  }
}

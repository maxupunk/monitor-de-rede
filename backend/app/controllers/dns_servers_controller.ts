import type { HttpContext } from '@adonisjs/core/http'
import vine from '@vinejs/vine'
import DnsServer from '#models/dns_server'
import { DnsServerRegistry } from '#modules/network_tools/dns/dns_server_registry'

const IPV4_RE = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$/
const HOSTNAME_RE =
  /^(?=.{1,253}$)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/

/**
 * Valida o endereço conforme o transporte: DoH exige um endpoint https, os
 * demais exigem IP/hostname com porta opcional.
 */
export function validateDnsAddress(address: string, protocol: string): string | null {
  const value = address.trim()
  if (!value) return 'Informe o endereço do servidor DNS'

  if (protocol === 'doh') {
    if (!/^https:\/\//i.test(value)) return 'O endpoint DoH precisa começar com https://'
    try {
      // eslint-disable-next-line no-new
      new URL(value)
      return null
    } catch {
      return 'Endpoint DoH inválido'
    }
  }

  const bracketMatch = value.match(/^\[(.+)\](?::(\d{1,5}))?$/)
  const host = bracketMatch
    ? bracketMatch[1]!
    : value.split(':').length === 2
      ? value.split(':')[0]!
      : value
  const port = bracketMatch?.[2] ?? (value.split(':').length === 2 ? value.split(':')[1] : null)

  if (port && (Number(port) < 1 || Number(port) > 65535)) {
    return 'Porta inválida — use um valor entre 1 e 65535'
  }
  if (!IPV4_RE.test(host) && !HOSTNAME_RE.test(host) && !host.includes(':')) {
    return 'Endereço inválido. Use um IP (ex: 1.1.1.1) ou hostname'
  }

  return null
}

export default class DnsServersController {
  private registry = new DnsServerRegistry()

  async index({ response }: HttpContext) {
    const servers = await this.registry.list()
    return response.ok(servers.map((server) => server.serialize()))
  }

  async store({ request, response }: HttpContext) {
    const schema = vine.object({
      name: vine.string().trim().minLength(1).maxLength(80),
      address: vine.string().trim().minLength(1).maxLength(255),
      protocol: vine.enum(['udp', 'tcp', 'doh']).optional(),
      isDefault: vine.boolean().optional(),
      description: vine.string().trim().maxLength(255).nullable().optional(),
    })

    const payload = await vine.validate({ schema, data: request.all() })
    const protocol = payload.protocol ?? 'udp'

    const addressError = validateDnsAddress(payload.address, protocol)
    if (addressError) {
      return response.unprocessableEntity({ message: addressError })
    }

    const duplicate = await DnsServer.query()
      .where('address', payload.address)
      .where('protocol', protocol)
      .first()

    if (duplicate) {
      return response.conflict({
        message: `O servidor ${payload.address} (${protocol.toUpperCase()}) já está cadastrado`,
      })
    }

    const server = await DnsServer.create({
      name: payload.name,
      address: payload.address,
      protocol,
      isDefault: payload.isDefault ?? true,
      description: payload.description ?? null,
    })

    return response.created(server.serialize())
  }

  async update({ params, request, response }: HttpContext) {
    const server = await DnsServer.findOrFail(params.id)

    const schema = vine.object({
      name: vine.string().trim().minLength(1).maxLength(80).optional(),
      address: vine.string().trim().minLength(1).maxLength(255).optional(),
      protocol: vine.enum(['udp', 'tcp', 'doh']).optional(),
      isDefault: vine.boolean().optional(),
      description: vine.string().trim().maxLength(255).nullable().optional(),
    })

    const payload = await vine.validate({ schema, data: request.all() })
    const protocol = payload.protocol ?? server.protocol
    const address = payload.address ?? server.address

    const addressError = validateDnsAddress(address, protocol)
    if (addressError) {
      return response.unprocessableEntity({ message: addressError })
    }

    const duplicate = await DnsServer.query()
      .where('address', address)
      .where('protocol', protocol)
      .whereNot('id', server.id)
      .first()

    if (duplicate) {
      return response.conflict({
        message: `O servidor ${address} (${protocol.toUpperCase()}) já está cadastrado`,
      })
    }

    server.merge({
      name: payload.name ?? server.name,
      address,
      protocol,
      isDefault: payload.isDefault ?? server.isDefault,
      description: payload.description === undefined ? server.description : payload.description,
    })
    await server.save()

    return response.ok(server.serialize())
  }

  async destroy({ params, response }: HttpContext) {
    const server = await DnsServer.findOrFail(params.id)
    await server.delete()
    return response.noContent()
  }
}

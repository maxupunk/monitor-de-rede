import db from '@adonisjs/lucid/services/db'
import type { TransactionClientContract } from '@adonisjs/lucid/types/database'
import { iterateUsableAddresses, firstUsableAddress, isIpInCidr } from './cidr.js'

/**
 * Alocação de IPs (IPAM) para a rede da VPN.
 *
 * A unicidade real é garantida pelo índice `unique(network_id, ip_address)` em
 * `devices`. Este alocador apenas *sugere* o próximo livre e reexecuta a
 * operação quando duas requisições concorrentes escolhem o mesmo endereço.
 */
export class IpAllocator {
  /** Nº máximo de tentativas antes de desistir por concorrência. */
  static readonly MAX_ATTEMPTS = 10

  /**
   * Identifica violação de unicidade em PostgreSQL (23505) e SQLite
   * (SQLITE_CONSTRAINT_UNIQUE), sem acoplar o serviço ao driver.
   */
  static isUniqueViolation(error: unknown): boolean {
    if (!error || typeof error !== 'object') return false
    const candidate = error as { code?: string; message?: string }
    if (candidate.code === '23505' || candidate.code === 'SQLITE_CONSTRAINT_UNIQUE') return true
    return /unique constraint|UNIQUE constraint failed/i.test(candidate.message ?? '')
  }

  /** IPs já ocupados por dispositivos da rede. */
  private async usedAddresses(
    networkId: number,
    trx?: TransactionClientContract
  ): Promise<Set<string>> {
    const query = (trx ?? db).from('devices').select('ip_address').where('network_id', networkId)
    const rows = await query
    return new Set(
      rows
        .map((row: { ip_address: string | null }) => row.ip_address)
        .filter((ip: string | null): ip is string => Boolean(ip))
    )
  }

  /**
   * Próximo IP livre do CIDR, pulando o endereço do servidor (primeiro utilizável)
   * e os endereços explicitamente reservados.
   */
  async findNextFree(
    networkId: number,
    cidr: string,
    reserved: string[] = [],
    trx?: TransactionClientContract
  ): Promise<string> {
    const used = await this.usedAddresses(networkId, trx)
    const serverAddress = firstUsableAddress(cidr)
    const blocked = new Set<string>([serverAddress, ...reserved, ...used])

    for (const candidate of iterateUsableAddresses(cidr)) {
      if (!blocked.has(candidate)) {
        return candidate
      }
    }

    throw new Error(`Não há endereços livres disponíveis na faixa ${cidr}`)
  }

  /**
   * Executa `operation` com um IP livre, repetindo com o próximo endereço quando
   * outra transação vence a corrida pelo mesmo IP.
   */
  async allocate<T>(
    networkId: number,
    cidr: string,
    operation: (ipAddress: string) => Promise<T>,
    reserved: string[] = []
  ): Promise<T> {
    const attempted: string[] = []

    for (let attempt = 0; attempt < IpAllocator.MAX_ATTEMPTS; attempt++) {
      const ipAddress = await this.findNextFree(networkId, cidr, [...reserved, ...attempted])

      try {
        return await operation(ipAddress)
      } catch (error: unknown) {
        if (!IpAllocator.isUniqueViolation(error)) {
          throw error
        }
        attempted.push(ipAddress)
      }
    }

    throw new Error(
      `Não foi possível alocar um IP em ${cidr} após ${IpAllocator.MAX_ATTEMPTS} tentativas (concorrência excessiva)`
    )
  }

  /** Valida um IP informado manualmente: precisa pertencer ao CIDR e estar livre. */
  async assertAvailable(networkId: number, cidr: string, ipAddress: string): Promise<void> {
    if (!isIpInCidr(ipAddress, cidr)) {
      throw new Error(`O endereço ${ipAddress} não pertence à faixa ${cidr}`)
    }
    if (ipAddress === firstUsableAddress(cidr)) {
      throw new Error(`O endereço ${ipAddress} é reservado para o servidor VPN`)
    }

    const used = await this.usedAddresses(networkId)
    if (used.has(ipAddress)) {
      throw new Error(`O endereço ${ipAddress} já está em uso nesta rede`)
    }
  }
}

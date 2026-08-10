import type { CheckResult } from './contracts/check_result.js'
import { PingChecker, type PingConfig } from './checkers/ping_checker.js'
import { HttpChecker, type HttpConfig } from './checkers/http_checker.js'
import { TcpChecker, type TcpConfig } from './checkers/tcp_checker.js'
import { DnsChecker, type DnsConfig } from './checkers/dns_checker.js'
import { SnmpChecker, type SnmpCheckerConfig } from './checkers/snmp_checker.js'

export interface RunMonitorOptions {
  /** Timeout do monitor (`timeoutSeconds`) aplicado quando o config não traz o seu */
  timeoutMs?: number
}

/**
 * Os checkers leem o timeout de dentro do `configuration`, então sem esta
 * mesclagem o `timeoutSeconds` do monitor seria ignorado e todo checker usaria
 * o seu default embutido. Um `timeoutMs` explícito no configuration continua
 * tendo prioridade sobre o campo do monitor.
 */
export function mergeTimeout<T extends object>(config: unknown, timeoutMs?: number): T {
  const base = (config ?? {}) as Record<string, unknown>
  if (!timeoutMs || timeoutMs <= 0 || base.timeoutMs !== undefined) return base as T
  return { ...base, timeoutMs } as T
}

export class MonitorRunner {
  private pingChecker = new PingChecker()
  private httpChecker = new HttpChecker()
  private tcpChecker = new TcpChecker()
  private dnsChecker = new DnsChecker()
  private snmpChecker = new SnmpChecker()

  private withTimeout<T extends object>(config: unknown, options?: RunMonitorOptions): T {
    return mergeTimeout<T>(config, options?.timeoutMs)
  }

  async runMonitor(
    type: string,
    config: unknown,
    options?: RunMonitorOptions
  ): Promise<CheckResult> {
    const normType = type.toLowerCase()

    switch (normType) {
      case 'ping':
        return this.pingChecker.execute(this.withTimeout<PingConfig>(config, options))
      case 'http':
      case 'https':
        return this.httpChecker.execute(this.withTimeout<HttpConfig>(config, options))
      case 'tcp':
        return this.tcpChecker.execute(this.withTimeout<TcpConfig>(config, options))
      case 'dns':
        return this.dnsChecker.execute(this.withTimeout<DnsConfig>(config, options))
      case 'snmp':
        return this.snmpChecker.execute(this.withTimeout<SnmpCheckerConfig>(config, options))
      default:
        throw new Error(`Tipo de monitor desconhecido ou não suportado: ${type}`)
    }
  }
}

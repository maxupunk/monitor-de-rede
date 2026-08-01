import type { CheckResult } from './contracts/check_result.js'
import { PingChecker, type PingConfig } from './checkers/ping_checker.js'
import { HttpChecker, type HttpConfig } from './checkers/http_checker.js'
import { TcpChecker, type TcpConfig } from './checkers/tcp_checker.js'
import { DnsChecker, type DnsConfig } from './checkers/dns_checker.js'

export class MonitorRunner {
  private pingChecker = new PingChecker()
  private httpChecker = new HttpChecker()
  private tcpChecker = new TcpChecker()
  private dnsChecker = new DnsChecker()

  async runMonitor(type: string, config: unknown): Promise<CheckResult> {
    const normType = type.toLowerCase()

    switch (normType) {
      case 'ping':
        return this.pingChecker.execute(config as PingConfig)
      case 'http':
      case 'https':
        return this.httpChecker.execute(config as HttpConfig)
      case 'tcp':
        return this.tcpChecker.execute(config as TcpConfig)
      case 'dns':
        return this.dnsChecker.execute(config as DnsConfig)
      default:
        throw new Error(`Tipo de monitor desconhecido ou não suportado: ${type}`)
    }
  }
}

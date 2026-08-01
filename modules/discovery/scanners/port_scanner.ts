import net from 'node:net'
import type { DiscoveredHost } from './icmp_scanner.js'

export class PortScanner {
  private commonPorts = [80, 443, 22, 445, 8080, 8000, 3389, 161]

  async scanHosts(hosts: DiscoveredHost[]): Promise<DiscoveredHost[]> {
    const updatedHosts: DiscoveredHost[] = []

    for (const host of hosts) {
      const openPorts = await this.scanPortsForIp(host.ipAddress)
      updatedHosts.push({
        ...host,
        openPorts,
        confidence: openPorts.length > 0 ? Math.min(100, host.confidence + 20) : host.confidence,
      })
    }

    return updatedHosts
  }

  private async scanPortsForIp(ip: string): Promise<number[]> {
    const openPorts: number[] = []

    const portChecks = this.commonPorts.map(
      (port) =>
        new Promise<number | null>((resolve) => {
          const socket = new net.Socket()
          socket.setTimeout(800)

          socket.on('connect', () => {
            socket.destroy()
            resolve(port)
          })

          socket.on('timeout', () => {
            socket.destroy()
            resolve(null)
          })

          socket.on('error', () => {
            socket.destroy()
            resolve(null)
          })

          socket.connect(port, ip)
        })
    )

    const results = await Promise.all(portChecks)
    for (const res of results) {
      if (res !== null) {
        openPorts.push(res)
      }
    }

    return openPorts
  }
}

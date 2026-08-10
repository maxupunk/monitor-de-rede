import type { MonitorStatus } from './contracts/check_result.js'

export class StatusCalculator {
  calculateDeviceStatus(monitorStatuses: MonitorStatus[]): MonitorStatus {
    if (monitorStatuses.includes('down')) return 'down'
    if (monitorStatuses.includes('warning')) return 'warning'
    if (monitorStatuses.includes('up')) return 'up'
    return 'unknown'
  }
}

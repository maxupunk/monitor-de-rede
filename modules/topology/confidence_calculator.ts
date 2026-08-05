export class ConfidenceCalculator {
  calculateConfidence(method: string): number {
    switch (method) {
      case 'manual':
        return 100
      case 'lldp':
        return 95
      case 'cdp':
        return 90
      case 'snmp':
        return 80
      case 'traceroute':
        return 60
      default:
        return 50
    }
  }
}

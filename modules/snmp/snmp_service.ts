import { SnmpSessionFactory } from './snmp_session_factory.js'
import type { SnmpConfig } from './clients/snmp_client.js'

export class SnmpService {
  private factory = new SnmpSessionFactory()

  async pollDevice(config: SnmpConfig) {
    const client = this.factory.createSession(config)
    return { client }
  }
}

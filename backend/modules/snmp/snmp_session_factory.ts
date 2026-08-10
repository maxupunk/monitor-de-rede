import { SnmpClient, type SnmpConfig } from './clients/snmp_client.js'

export class SnmpSessionFactory {
  createSession(config: SnmpConfig): SnmpClient {
    return new SnmpClient(config)
  }
}

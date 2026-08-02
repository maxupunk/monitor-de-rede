import type { SnmpClient } from '../clients/snmp_client.js'

export interface SnmpSystemInfo {
  sysName?: string
  sysDescr?: string
  sysObjectID?: string
  sysUpTime?: number
  sysContact?: string
  sysLocation?: string
}

export class SystemCollector {
  public static readonly OID_SYS_DESCR = '1.3.6.1.2.1.1.1.0'
  public static readonly OID_SYS_OBJECT_ID = '1.3.6.1.2.1.1.2.0'
  public static readonly OID_SYS_UPTIME = '1.3.6.1.2.1.1.3.0'
  public static readonly OID_SYS_CONTACT = '1.3.6.1.2.1.1.4.0'
  public static readonly OID_SYS_NAME = '1.3.6.1.2.1.1.5.0'
  public static readonly OID_SYS_LOCATION = '1.3.6.1.2.1.1.6.0'

  async collect(client: SnmpClient): Promise<SnmpSystemInfo> {
    const oids = [
      SystemCollector.OID_SYS_DESCR,
      SystemCollector.OID_SYS_OBJECT_ID,
      SystemCollector.OID_SYS_UPTIME,
      SystemCollector.OID_SYS_CONTACT,
      SystemCollector.OID_SYS_NAME,
      SystemCollector.OID_SYS_LOCATION,
    ]

    const response = await client.get(oids)

    return {
      sysDescr: response[SystemCollector.OID_SYS_DESCR] ? String(response[SystemCollector.OID_SYS_DESCR]) : undefined,
      sysObjectID: response[SystemCollector.OID_SYS_OBJECT_ID] ? String(response[SystemCollector.OID_SYS_OBJECT_ID]) : undefined,
      sysUpTime: response[SystemCollector.OID_SYS_UPTIME] !== undefined && response[SystemCollector.OID_SYS_UPTIME] !== null ? Number(response[SystemCollector.OID_SYS_UPTIME]) : undefined,
      sysContact: response[SystemCollector.OID_SYS_CONTACT] ? String(response[SystemCollector.OID_SYS_CONTACT]) : undefined,
      sysName: response[SystemCollector.OID_SYS_NAME] ? String(response[SystemCollector.OID_SYS_NAME]) : undefined,
      sysLocation: response[SystemCollector.OID_SYS_LOCATION] ? String(response[SystemCollector.OID_SYS_LOCATION]) : undefined,
    }
  }
}

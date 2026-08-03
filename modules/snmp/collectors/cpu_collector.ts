import type { SnmpClient } from '../clients/snmp_client.js'

export interface CpuCoreLoad {
  coreIndex: number
  loadPercent: number
}

export interface SnmpCpuInfo {
  usagePercent?: number
  userPercent?: number
  systemPercent?: number
  idlePercent?: number
  load1min?: number
  load5min?: number
  load15min?: number
  coresCount?: number
  cores?: CpuCoreLoad[]
}

export class CpuCollector {
  public static readonly BASE_HR_PROCESSOR_LOAD = '1.3.6.1.2.1.25.3.3.1.2'
  public static readonly OID_SS_CPU_USER = '1.3.6.1.4.1.2021.11.9.0'
  public static readonly OID_SS_CPU_SYSTEM = '1.3.6.1.4.1.2021.11.10.0'
  public static readonly OID_SS_CPU_IDLE = '1.3.6.1.4.1.2021.11.11.0'
  public static readonly OID_LA_LOAD_1 = '1.3.6.1.4.1.2021.10.1.3.1'
  public static readonly OID_LA_LOAD_5 = '1.3.6.1.4.1.2021.10.1.3.2'
  public static readonly OID_LA_LOAD_15 = '1.3.6.1.4.1.2021.10.1.3.3'

  async collect(client: SnmpClient): Promise<SnmpCpuInfo> {
    const result: SnmpCpuInfo = {}

    // 1. Host Resources MIB - Cores load
    const processorEntries = await client.walk(CpuCollector.BASE_HR_PROCESSOR_LOAD)
    if (processorEntries.length > 0) {
      const cores: CpuCoreLoad[] = []
      let totalLoad = 0
      processorEntries.forEach((entry, idx) => {
        const val = Number(entry.value)
        // Validar porcentagem entre 0 e 100 por núcleo
        if (!isNaN(val) && val >= 0 && val <= 100) {
          cores.push({ coreIndex: idx + 1, loadPercent: val })
          totalLoad += val
        }
      })
      if (cores.length > 0) {
        result.coresCount = cores.length
        result.cores = cores
        result.usagePercent = Math.round(totalLoad / cores.length)
      }
    }

    // 2. UCD-SNMP-MIB CPU & Load averages
    const ucdResponse = await client.get([
      CpuCollector.OID_SS_CPU_USER,
      CpuCollector.OID_SS_CPU_SYSTEM,
      CpuCollector.OID_SS_CPU_IDLE,
      CpuCollector.OID_LA_LOAD_1,
      CpuCollector.OID_LA_LOAD_5,
      CpuCollector.OID_LA_LOAD_15,
    ])

    const user =
      ucdResponse[CpuCollector.OID_SS_CPU_USER] !== null &&
      ucdResponse[CpuCollector.OID_SS_CPU_USER] !== undefined
        ? Number(ucdResponse[CpuCollector.OID_SS_CPU_USER])
        : undefined
    const sys =
      ucdResponse[CpuCollector.OID_SS_CPU_SYSTEM] !== null &&
      ucdResponse[CpuCollector.OID_SS_CPU_SYSTEM] !== undefined
        ? Number(ucdResponse[CpuCollector.OID_SS_CPU_SYSTEM])
        : undefined
    const idle =
      ucdResponse[CpuCollector.OID_SS_CPU_IDLE] !== null &&
      ucdResponse[CpuCollector.OID_SS_CPU_IDLE] !== undefined
        ? Number(ucdResponse[CpuCollector.OID_SS_CPU_IDLE])
        : undefined

    if (user !== undefined && !isNaN(user)) result.userPercent = user
    if (sys !== undefined && !isNaN(sys)) result.systemPercent = sys
    if (idle !== undefined && !isNaN(idle)) {
      result.idlePercent = idle
      if (result.usagePercent === undefined) {
        result.usagePercent = Math.max(0, Math.min(100, 100 - idle))
      }
    }

    const parseLoad = (val: unknown) => {
      if (!val) return undefined
      const num = parseFloat(String(val))
      return isNaN(num) ? undefined : num
    }

    result.load1min = parseLoad(ucdResponse[CpuCollector.OID_LA_LOAD_1])
    result.load5min = parseLoad(ucdResponse[CpuCollector.OID_LA_LOAD_5])
    result.load15min = parseLoad(ucdResponse[CpuCollector.OID_LA_LOAD_15])

    return result
  }
}

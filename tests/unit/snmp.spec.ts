import { test } from '@japa/runner'
import { SnmpClient } from '#modules/snmp/clients/snmp_client'
import { SystemCollector } from '#modules/snmp/collectors/system_collector'
import { InterfaceCollector } from '#modules/snmp/collectors/interface_collector'
import { TrafficCollector } from '#modules/snmp/collectors/traffic_collector'
import { LldpCollector } from '#modules/snmp/collectors/lldp_collector'
import { CpuCollector } from '#modules/snmp/collectors/cpu_collector'
import { MemoryCollector } from '#modules/snmp/collectors/memory_collector'

test.group('SNMP Collectors - Unit Tests', () => {
  test('SystemCollector deve extrair informações de sistema MIB-II', async ({ assert }) => {
    const client = new SnmpClient({ host: '127.0.0.1', version: 'v2c' })
    client.setMockGet({
      [SystemCollector.OID_SYS_NAME]: 'Core-Switch-01',
      [SystemCollector.OID_SYS_DESCR]: 'Cisco IOS Software, C3750 Software',
      [SystemCollector.OID_SYS_UPTIME]: 1234567,
    })

    const collector = new SystemCollector()
    const info = await collector.collect(client)

    assert.equal(info.sysName, 'Core-Switch-01')
    assert.equal(info.sysDescr, 'Cisco IOS Software, C3750 Software')
    assert.equal(info.sysUpTime, 1234567)
  })

  test('InterfaceCollector deve listar interfaces e mapear atributos', async ({ assert }) => {
    const client = new SnmpClient({ host: '127.0.0.1', version: 'v2c' })
    client.setMockWalk(InterfaceCollector.BASE_IF_TABLE, [
      {
        oid: `${InterfaceCollector.BASE_IF_TABLE}.${InterfaceCollector.COL_IF_DESCR}.1`,
        value: 'GigabitEthernet1/0/1',
      },
      {
        oid: `${InterfaceCollector.BASE_IF_TABLE}.${InterfaceCollector.COL_IF_SPEED}.1`,
        value: 1000000000,
      },
      {
        oid: `${InterfaceCollector.BASE_IF_TABLE}.${InterfaceCollector.COL_IF_ADMIN_STATUS}.1`,
        value: 1,
      },
      {
        oid: `${InterfaceCollector.BASE_IF_TABLE}.${InterfaceCollector.COL_IF_OPER_STATUS}.1`,
        value: 1,
      },
    ])

    const collector = new InterfaceCollector()
    const ifaces = await collector.collect(client)

    assert.equal(ifaces.length, 1)
    assert.equal(ifaces[0].ifIndex, 1)
    assert.equal(ifaces[0].ifDescr, 'GigabitEthernet1/0/1')
    assert.equal(ifaces[0].ifSpeed, 1000000000)
    assert.equal(ifaces[0].ifAdminStatus, 1)
    assert.equal(ifaces[0].ifOperStatus, 1)
  })

  test('TrafficCollector deve coletar contadores de tráfego e calcular BPS', async ({ assert }) => {
    const client = new SnmpClient({ host: '127.0.0.1', version: 'v2c' })
    client.setMockWalk(TrafficCollector.BASE_IF_TABLE, [
      {
        oid: `${TrafficCollector.BASE_IF_TABLE}.${TrafficCollector.COL_IF_IN_OCTETS}.1`,
        value: 100000,
      },
      {
        oid: `${TrafficCollector.BASE_IF_TABLE}.${TrafficCollector.COL_IF_OUT_OCTETS}.1`,
        value: 50000,
      },
    ])

    const collector = new TrafficCollector()
    const traffic = await collector.collect(client)

    assert.equal(traffic.length, 1)
    assert.equal(traffic[0].inOctets, 100000)
    assert.equal(traffic[0].outOctets, 50000)

    const now = Date.now()
    traffic[0].recordedAt = new Date(now)
    const prevDate = new Date(now - 10000) // exatamente 10 segundos atrás
    const rates = collector.calculateRates(
      {
        ifIndex: 1,
        inOctets: 50000,
        outOctets: 25000,
        inErrors: 0,
        outErrors: 0,
        recordedAt: prevDate,
      },
      traffic[0]
    )

    // (100000 - 50000) * 8 / 10s = 40000 bps
    assert.equal(rates.inBps, 40000)
    // (50000 - 25000) * 8 / 10s = 20000 bps
    assert.equal(rates.outBps, 20000)
  })

  test('LldpCollector deve extrair vizinhos LLDP', async ({ assert }) => {
    const client = new SnmpClient({ host: '127.0.0.1', version: 'v2c' })
    client.setMockWalk(LldpCollector.BASE_LLDP_REM_TABLE, [
      {
        oid: `${LldpCollector.BASE_LLDP_REM_TABLE}.${LldpCollector.COL_LLDP_PORT_ID}.100.1.1`,
        value: 'Gi0/1',
      },
      {
        oid: `${LldpCollector.BASE_LLDP_REM_TABLE}.${LldpCollector.COL_LLDP_SYS_NAME}.100.1.1`,
        value: 'Access-SW-02',
      },
    ])

    const collector = new LldpCollector()
    const neighbors = await collector.collect(client)

    assert.equal(neighbors.length, 1)
    assert.equal(neighbors[0].localPort, '1')
    assert.equal(neighbors[0].remotePort, 'Gi0/1')
    assert.equal(neighbors[0].remoteSysName, 'Access-SW-02')
    assert.equal(neighbors[0].protocol, 'lldp')
  })

  test('CpuCollector deve coletar dados de carga de CPU por núcleo e load average', async ({
    assert,
  }) => {
    const client = new SnmpClient({ host: '127.0.0.1', version: 'v2c' })
    client.setMockWalk(CpuCollector.BASE_HR_PROCESSOR_LOAD, [
      { oid: `${CpuCollector.BASE_HR_PROCESSOR_LOAD}.1`, value: 10 },
      { oid: `${CpuCollector.BASE_HR_PROCESSOR_LOAD}.2`, value: 20 },
      { oid: `${CpuCollector.BASE_HR_PROCESSOR_LOAD}.3`, value: 30 },
      { oid: `${CpuCollector.BASE_HR_PROCESSOR_LOAD}.4`, value: 40 },
    ])
    client.setMockGet({
      [CpuCollector.OID_SS_CPU_USER]: 15,
      [CpuCollector.OID_SS_CPU_SYSTEM]: 10,
      [CpuCollector.OID_SS_CPU_IDLE]: 75,
      [CpuCollector.OID_LA_LOAD_1]: '0.35',
      [CpuCollector.OID_LA_LOAD_5]: '0.20',
      [CpuCollector.OID_LA_LOAD_15]: '0.15',
    })

    const collector = new CpuCollector()
    const cpuInfo = await collector.collect(client)

    assert.equal(cpuInfo.coresCount, 4)
    assert.equal(cpuInfo.usagePercent, 25) // (10+20+30+40)/4
    assert.equal(cpuInfo.userPercent, 15)
    assert.equal(cpuInfo.systemPercent, 10)
    assert.equal(cpuInfo.idlePercent, 75)
    assert.equal(cpuInfo.load1min, 0.35)
  })

  test('MemoryCollector deve coletar uso de memória RAM', async ({ assert }) => {
    const client = new SnmpClient({ host: '127.0.0.1', version: 'v2c' })
    client.setMockGet({
      [MemoryCollector.OID_MEM_TOTAL_REAL]: 2048000,
      [MemoryCollector.OID_MEM_AVAIL_REAL]: 1024000,
      [MemoryCollector.OID_MEM_TOTAL_FREE]: 512000,
    })

    const collector = new MemoryCollector()
    const memInfo = await collector.collect(client)

    assert.equal(memInfo.totalKb, 2048000)
    assert.equal(memInfo.availKb, 1024000)
    assert.equal(memInfo.usedKb, 1024000)
    assert.equal(memInfo.usedPercent, 50)
  })
})

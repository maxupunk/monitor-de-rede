import type Device from '#models/device'
import Monitor from '#models/monitor'

export const ZABBIX_TEMPLATE_MONITOR_NAME = 'Coleta de Template Zabbix'

/**
 * Métricas de um Template Zabbix só são coletadas como efeito colateral de um monitor
 * SNMP em execução (ver SnmpChecker.execute → SnmpService.pollDevice). Sem isso, um
 * dispositivo com apenas o template vinculado (sem monitor de CPU, Memória ou
 * Interface) nunca teria seus itens agendados pelo scheduler. Esta função garante a
 * existência de um monitor "de sincronização" — mesmo padrão já usado para CPU/Memória
 * em DevicesController — e é chamada tanto ao salvar o dispositivo quanto ao rodar um
 * poll/scan manual, para se autocorrigir mesmo em dispositivos configurados antes desta
 * mudança existir.
 */
export async function syncZabbixTemplateMonitor(device: Device): Promise<void> {
  const existingMonitor = await Monitor.query()
    .where('deviceId', device.id)
    .where('name', ZABBIX_TEMPLATE_MONITOR_NAME)
    .first()

  if (device.zabbixTemplateId) {
    const targetHost = device.ipAddress || device.name
    if (existingMonitor) {
      existingMonitor.enabled = true
      existingMonitor.configuration = { host: targetHost }
      await existingMonitor.save()
    } else {
      await Monitor.create({
        deviceId: device.id,
        name: ZABBIX_TEMPLATE_MONITOR_NAME,
        type: 'snmp',
        configuration: { host: targetHost },
        intervalSeconds: 60,
        timeoutSeconds: 5,
        retryCount: 3,
        enabled: true,
        status: 'unknown',
      })
    }
  } else if (existingMonitor && existingMonitor.enabled) {
    existingMonitor.enabled = false
    await existingMonitor.save()
  }
}

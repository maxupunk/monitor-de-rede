import { DateTime } from 'luxon'
import type { SnmpClient } from '#modules/snmp/clients/snmp_client'
import type Device from '#models/device'
import ZabbixTemplate from '#models/zabbix_template'
import type ZabbixTemplateItem from '#models/zabbix_template_item'
import Metric from '#models/metric'

export interface ZabbixTemplateItemReading {
  id: number
  name: string
  key: string
  units: string | null
  /** null quando o dispositivo não respondeu a esse OID específico. */
  value: number | null
}

// Muitos agentes SNMP embarcados (firmwares de dispositivos de baixo custo, como
// controladores solares) limitam o número de varbinds aceitos em um único GET ou o
// tamanho do datagrama UDP — pedir todos os OIDs do template de uma vez pode fazer o
// dispositivo simplesmente não responder ao pacote inteiro. Consultar em lotes menores
// é mais lento, mas muito mais compatível.
const OID_BATCH_SIZE = 6

/**
 * Coleta, via SNMP, os itens de um Template Zabbix importado e vinculado ao
 * dispositivo — generaliza o que antes era um coletor específico por fabricante
 * (ver PR de remoção do VoltMpptCollector). Os OIDs do template são buscados em
 * lotes (ver OID_BATCH_SIZE); o multiplicador de cada item (preprocessing MULTIPLIER
 * do Zabbix) é aplicado antes de gravar o Metric, usando a key_ do Zabbix como nome.
 */
export class ZabbixTemplateCollector {
  async collect(device: Device, client: SnmpClient): Promise<number> {
    const readings = await this.preview(device, client)
    const now = DateTime.now()
    let count = 0

    for (const reading of readings) {
      if (reading.value === null) continue
      await Metric.create({
        deviceId: device.id,
        name: reading.key,
        value: reading.value,
        unit: reading.units || '',
        recordedAt: now,
      })
      count++
    }

    return count
  }

  /** Lê os itens do template sem gravar Metric — usado pela varredura/preview interativa. */
  async preview(device: Device, client: SnmpClient): Promise<ZabbixTemplateItemReading[]> {
    if (!device.zabbixTemplateId) return []

    const template = await ZabbixTemplate.query()
      .where('id', device.zabbixTemplateId)
      .preload('items')
      .first()

    if (!template || template.items.length === 0) return []

    return this.readItems(template.items, client)
  }

  private async readItems(
    items: ZabbixTemplateItem[],
    client: SnmpClient
  ): Promise<ZabbixTemplateItemReading[]> {
    // Apenas itens numéricos viram métricas de série temporal (TEXT/CHAR/LOG ficam de fora por ora).
    const numericItems = items.filter(
      (item) => item.valueType !== 'TEXT' && item.valueType !== 'CHAR' && item.valueType !== 'LOG'
    )
    if (numericItems.length === 0) return []

    const oids = numericItems.map((item) => item.snmpOid)
    const response: Record<string, unknown> = {}
    for (let i = 0; i < oids.length; i += OID_BATCH_SIZE) {
      const batch = oids.slice(i, i + OID_BATCH_SIZE)
      Object.assign(response, await client.get(batch))
    }

    return numericItems.map((item) => {
      const raw = response[item.snmpOid]
      let value: number | null = null

      if (raw !== null && raw !== undefined) {
        const num = Number(raw)
        if (!Number.isNaN(num)) {
          value = item.multiplier !== null ? num * item.multiplier : num
        }
      }

      return { id: item.id, name: item.name, key: item.key, units: item.units, value }
    })
  }
}

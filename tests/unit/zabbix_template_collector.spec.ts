import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import { SnmpClient } from '#modules/snmp/clients/snmp_client'
import { ZabbixTemplateCollector } from '#modules/zabbix/zabbix_template_collector'
import Device from '#models/device'
import ZabbixTemplate from '#models/zabbix_template'
import ZabbixTemplateItem from '#models/zabbix_template_item'
import Metric from '#models/metric'
import { DateTime } from 'luxon'

test.group('ZabbixTemplateCollector - Unit Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('deve coletar os itens do template vinculado, aplicar o multiplicador e gravar Metrics', async ({
    assert,
  }) => {
    const device = await Device.create({
      name: 'Controlador Solar 01',
      ipAddress: '192.168.10.50',
      type: 'other',
      status: 'online',
    })

    const template = await ZabbixTemplate.create({
      name: 'Controlador de Carga MPPT 20-30-40A - Linha Mpower - VoltOs 4.1.2',
      zabbixVersion: '7.0',
      rawExport: { zabbix_export: { version: '7.0' } },
      importedAt: DateTime.now(),
    })

    await ZabbixTemplateItem.create({
      templateId: template.id,
      name: 'Tensão de Bateria',
      key: 'V.bat',
      snmpOid: '.1.3.6.1.4.1.57072.1.3.8.0',
      valueType: 'FLOAT',
      units: 'V',
      multiplier: 0.1,
    })

    await ZabbixTemplateItem.create({
      templateId: template.id,
      name: 'Status Bateria',
      key: 'sts.bat',
      snmpOid: '.1.3.6.1.4.1.57072.1.3.10.0',
      valueType: 'UNSIGNED',
      units: 'sts',
      multiplier: null,
    })

    device.zabbixTemplateId = template.id
    await device.save()

    const client = new SnmpClient({ host: device.ipAddress!, version: 'v2c' })
    client.setMockGet({
      '.1.3.6.1.4.1.57072.1.3.8.0': 517, // 51.7V já nos décimos de volt do Zabbix
      '.1.3.6.1.4.1.57072.1.3.10.0': 3, // "carregada"
    })

    const collector = new ZabbixTemplateCollector()
    const count = await collector.collect(device, client)

    assert.equal(count, 2)

    const voltageMetric = await Metric.query()
      .where('deviceId', device.id)
      .where('name', 'V.bat')
      .first()
    assert.exists(voltageMetric)
    assert.approximately(voltageMetric!.value, 51.7, 0.001)
    assert.equal(voltageMetric!.unit, 'V')

    const statusMetric = await Metric.query()
      .where('deviceId', device.id)
      .where('name', 'sts.bat')
      .first()
    assert.exists(statusMetric)
    assert.equal(statusMetric!.value, 3)
  })

  test('não deve coletar nada quando o dispositivo não tem template vinculado', async ({
    assert,
  }) => {
    const device = await Device.create({
      name: 'Dispositivo Sem Template',
      ipAddress: '192.168.10.51',
      type: 'other',
      status: 'online',
    })

    const client = new SnmpClient({ host: device.ipAddress!, version: 'v2c' })
    const collector = new ZabbixTemplateCollector()
    const count = await collector.collect(device, client)

    assert.equal(count, 0)
  })

  test('preview() deve retornar valor null para itens sem resposta, sem gravar Metric (usado na varredura/scan)', async ({
    assert,
  }) => {
    const device = await Device.create({
      name: 'Controlador Solar 02',
      ipAddress: '192.168.10.52',
      type: 'other',
      status: 'online',
    })

    const template = await ZabbixTemplate.create({
      name: 'Template Parcialmente Respondendo',
      zabbixVersion: '7.0',
      rawExport: { zabbix_export: { version: '7.0' } },
      importedAt: DateTime.now(),
    })

    await ZabbixTemplateItem.create({
      templateId: template.id,
      name: 'Tensão de Bateria',
      key: 'V.bat',
      snmpOid: '.1.3.6.1.4.1.57072.1.3.8.0',
      valueType: 'FLOAT',
      units: 'V',
      multiplier: 0.1,
    })
    await ZabbixTemplateItem.create({
      templateId: template.id,
      name: 'Corrente Painel',
      key: 'A.painel',
      snmpOid: '.1.3.6.1.4.1.57072.1.3.5.0',
      valueType: 'FLOAT',
      units: 'A',
      multiplier: 0.1,
    })

    device.zabbixTemplateId = template.id
    await device.save()

    const client = new SnmpClient({ host: device.ipAddress!, version: 'v2c' })
    // Só um dos dois OIDs responde — simula um dispositivo real que respondeu parcialmente.
    client.setMockGet({ '.1.3.6.1.4.1.57072.1.3.8.0': 517 })

    const collector = new ZabbixTemplateCollector()
    const readings = await collector.preview(device, client)

    assert.lengthOf(readings, 2)
    const voltage = readings.find((r) => r.key === 'V.bat')!
    assert.approximately(voltage.value!, 51.7, 0.001)
    const current = readings.find((r) => r.key === 'A.painel')!
    assert.isNull(current.value)

    const metricCount = await Metric.query().where('deviceId', device.id).count('* as total')
    assert.equal(
      Number((metricCount[0] as unknown as { $extras: { total: number } }).$extras.total),
      0
    )
  })
})

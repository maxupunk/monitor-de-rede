import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import { DateTime } from 'luxon'
import ZabbixTemplate from '#models/zabbix_template'
import Device from '#models/device'
import Monitor from '#models/monitor'
import { syncZabbixTemplateMonitor } from '#modules/zabbix/zabbix_template_monitor_sync'

// Amostra fiel (reduzida) do export oficial da Volt Tecnologia para o Controlador
// de Carga MPPT 20-30-40A (VoltOs 4.1.2).
function buildVoltMpptExport() {
  return {
    zabbix_export: {
      version: '7.0',
      templates: [
        {
          uuid: 'f070634b50f844a3b753c6f64429acd5',
          template: 'Controlador de Carga MPPT 20-30-40A - Linha Mpower - VoltOs 4.1.2',
          name: 'Controlador de Carga MPPT 20-30-40A - Linha Mpower - VoltOs 4.1.2',
          description: 'VoltOs 4.1.2',
          items: [
            {
              uuid: 'e4de9493e0c1486ab6a9ca7abfa14cc4',
              name: 'Tensão de Bateria',
              type: 'SNMP_AGENT',
              snmp_oid: '.1.3.6.1.4.1.57072.1.3.8.0',
              key: 'V.bat',
              value_type: 'FLOAT',
              units: 'V',
              preprocessing: [{ type: 'MULTIPLIER', parameters: ['0.1'] }],
            },
            {
              uuid: '37bb0d8aa8db42f2944c9d01a360fa9d',
              name: 'Corrente Painel',
              type: 'SNMP_AGENT',
              snmp_oid: '.1.3.6.1.4.1.57072.1.3.5.0',
              key: 'A.painel',
              value_type: 'FLOAT',
              units: 'A',
              preprocessing: [{ type: 'MULTIPLIER', parameters: ['0.1'] }],
            },
          ],
        },
      ],
    },
  }
}

test.group('Zabbix Templates API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('POST /api/zabbix-templates deve importar um template Zabbix válido', async ({
    client,
    assert,
  }) => {
    const response = await client
      .visit('zabbix_templates.store')
      .json({ content: JSON.stringify(buildVoltMpptExport()) })

    response.assertStatus(201)
    const body = response.body()
    assert.lengthOf(body.templates, 1)
    assert.equal(body.templates[0].itemCount, 2)
    assert.lengthOf(body.templates[0].skippedItems, 0)

    const stored = await ZabbixTemplate.query().preload('items').first()
    assert.exists(stored)
    assert.lengthOf(stored!.items, 2)
    assert.equal(stored!.zabbixUuid, 'f070634b50f844a3b753c6f64429acd5')
  })

  test('POST /api/zabbix-templates deve rejeitar conteúdo que não seja um export Zabbix válido', async ({
    client,
  }) => {
    const response = await client
      .visit('zabbix_templates.store')
      .json({ content: '{"foo": "bar"}' })
    response.assertStatus(422)
  })

  test('GET /api/zabbix-templates deve listar templates com contagem de itens e dispositivos', async ({
    client,
    assert,
  }) => {
    await client
      .visit('zabbix_templates.store')
      .json({ content: JSON.stringify(buildVoltMpptExport()) })
    const template = await ZabbixTemplate.firstOrFail()
    await Device.create({
      name: 'Controlador Solar',
      type: 'other',
      status: 'online',
      zabbixTemplateId: template.id,
    })

    const response = await client.visit('zabbix_templates.index')

    response.assertStatus(200)
    const [item] = response.body()
    assert.equal(item.deviceCount, 1)
    assert.lengthOf(item.items, 2)
  })

  test('reimportar um template com o mesmo uuid deve substituir os itens em vez de duplicar', async ({
    client,
    assert,
  }) => {
    await client
      .visit('zabbix_templates.store')
      .json({ content: JSON.stringify(buildVoltMpptExport()) })

    const exportData = buildVoltMpptExport()
    // Remove um item na reimportação, simulando uma nova versão do template
    exportData.zabbix_export.templates[0].items.pop()
    await client.visit('zabbix_templates.store').json({ content: JSON.stringify(exportData) })

    const templates = await ZabbixTemplate.query().preload('items')
    assert.lengthOf(templates, 1, 'não deve duplicar o template ao reimportar o mesmo uuid')
    assert.lengthOf(templates[0].items, 1)
  })

  test('DELETE /api/zabbix-templates/:id deve remover o template e desvincular dispositivos', async ({
    client,
    assert,
  }) => {
    await client
      .visit('zabbix_templates.store')
      .json({ content: JSON.stringify(buildVoltMpptExport()) })
    const template = await ZabbixTemplate.firstOrFail()
    const device = await Device.create({
      name: 'Controlador Solar',
      type: 'other',
      status: 'online',
      zabbixTemplateId: template.id,
    })

    const response = await client.visit('zabbix_templates.destroy', { id: template.id })
    response.assertStatus(204)

    assert.isNull(await ZabbixTemplate.find(template.id))
    await device.refresh()
    assert.isNull(device.zabbixTemplateId)
  })

  test('vincular um template a um dispositivo via PUT /api/devices/:id deve criar o monitor de coleta periódica', async ({
    client,
    assert,
  }) => {
    await client
      .visit('zabbix_templates.store')
      .json({ content: JSON.stringify(buildVoltMpptExport()) })
    const template = await ZabbixTemplate.firstOrFail()

    const device = await Device.create({
      name: 'Controlador Solar',
      ipAddress: '10.0.0.34',
      type: 'other',
      status: 'online',
    })

    // Sem template: nenhum monitor de coleta deve existir ainda.
    let monitor = await Monitor.query()
      .where('deviceId', device.id)
      .where('name', 'Coleta de Template Zabbix')
      .first()
    assert.isNull(monitor)

    const response = await client.visit('devices.update', { id: device.id }).json({
      name: device.name,
      ipAddress: device.ipAddress,
      type: device.type,
      zabbixTemplateId: template.id,
    })
    response.assertStatus(200)

    monitor = await Monitor.query()
      .where('deviceId', device.id)
      .where('name', 'Coleta de Template Zabbix')
      .first()
    assert.exists(monitor, 'deveria existir um monitor SNMP agendando a coleta do template')
    // SQLite/Lucid retorna 0/1 (não um boolean estrito) para colunas booleanas aqui.
    assert.isTrue(Boolean(monitor!.enabled))
    assert.equal(monitor!.type, 'snmp')
    assert.equal((monitor!.configuration as { host?: string }).host, '10.0.0.34')

    // Desvincular o template deve desabilitar (não apagar) o monitor de coleta.
    await client.visit('devices.update', { id: device.id }).json({
      name: device.name,
      ipAddress: device.ipAddress,
      type: device.type,
      zabbixTemplateId: null,
    })

    await monitor!.refresh()
    assert.isFalse(Boolean(monitor!.enabled))
  })

  test('syncZabbixTemplateMonitor deve autocorrigir dispositivos configurados antes desta correção existir', async ({
    assert,
  }) => {
    const template = await ZabbixTemplate.create({
      name: 'Template Legado',
      zabbixVersion: '7.0',
      rawExport: { zabbix_export: { version: '7.0' } },
      importedAt: DateTime.now(),
    })

    // Simula um dispositivo configurado antes desta correção existir: template
    // vinculado diretamente no banco, sem nunca ter passado pelo controller.
    const device = await Device.create({
      name: 'Controlador Solar Legado',
      ipAddress: '10.0.0.99',
      type: 'other',
      status: 'online',
      zabbixTemplateId: template.id,
    })

    // poll()/scan() chamam essa mesma função antes de tentar o SNMP real — chamamos
    // direto aqui para não depender de rede em teste (a chamada real é coberta pela
    // leitura de código dos controllers, que só delegam para esta função pura).
    await syncZabbixTemplateMonitor(device)

    const monitor = await Monitor.query()
      .where('deviceId', device.id)
      .where('name', 'Coleta de Template Zabbix')
      .first()
    assert.exists(monitor, 'deveria autocorrigir e criar o monitor de coleta ausente')
    assert.isTrue(Boolean(monitor!.enabled))
    assert.equal(monitor!.type, 'snmp')
  })
})

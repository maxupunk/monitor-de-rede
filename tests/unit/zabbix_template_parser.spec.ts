import { test } from '@japa/runner'
import {
  parseZabbixTemplateExport,
  ZabbixTemplateParseError,
} from '#modules/zabbix/zabbix_template_parser'

// Amostra fiel (reduzida) do export oficial da Volt Tecnologia para o Controlador
// de Carga MPPT 20-30-40A (VoltOs 4.1.2) — inclui um item SNMP_AGENT com
// preprocessing MULTIPLIER, um sem "value_type" (default UNSIGNED) e um item
// de outro tipo (TRAPPER) para validar que é ignorado sem quebrar o import.
const VOLT_MPPT_EXPORT_SAMPLE = {
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
            uuid: '687926d1207d48aeb4df1cf4783d3688',
            name: 'Status Bateria',
            type: 'SNMP_AGENT',
            snmp_oid: '.1.3.6.1.4.1.57072.1.3.10.0',
            key: 'sts.bat',
            units: 'sts',
          },
          {
            uuid: 'aaaa0000aaaa0000aaaa0000aaaa0000',
            name: 'Alerta via Trapper (não suportado)',
            type: 'TRAPPER',
            key: 'trap.alert',
          },
        ],
      },
    ],
  },
}

test.group('ZabbixTemplateParser - Unit Tests', () => {
  test('deve extrair itens SNMP_AGENT com multiplicador e ignorar os demais tipos', ({
    assert,
  }) => {
    const [parsed] = parseZabbixTemplateExport(VOLT_MPPT_EXPORT_SAMPLE)

    assert.equal(parsed.uuid, 'f070634b50f844a3b753c6f64429acd5')
    assert.equal(parsed.name, 'Controlador de Carga MPPT 20-30-40A - Linha Mpower - VoltOs 4.1.2')
    assert.equal(parsed.zabbixVersion, '7.0')
    assert.lengthOf(parsed.items, 2)
    assert.lengthOf(parsed.skippedItems, 1)
    assert.equal(parsed.skippedItems[0].type, 'TRAPPER')

    const voltage = parsed.items.find((i) => i.key === 'V.bat')!
    assert.equal(voltage.snmpOid, '.1.3.6.1.4.1.57072.1.3.8.0')
    assert.equal(voltage.valueType, 'FLOAT')
    assert.equal(voltage.units, 'V')
    assert.equal(voltage.multiplier, 0.1)

    const status = parsed.items.find((i) => i.key === 'sts.bat')!
    assert.equal(
      status.valueType,
      'UNSIGNED',
      'value_type ausente deve assumir o padrão UNSIGNED do Zabbix'
    )
    assert.isNull(status.multiplier)
  })

  test('deve aceitar o conteúdo como string JSON', ({ assert }) => {
    const [parsed] = parseZabbixTemplateExport(JSON.stringify(VOLT_MPPT_EXPORT_SAMPLE))
    assert.lengthOf(parsed.items, 2)
  })

  test('deve rejeitar JSON malformado', ({ assert }) => {
    assert.throws(() => parseZabbixTemplateExport('{ isso não é json'), ZabbixTemplateParseError)
  })

  test('deve rejeitar um objeto sem "zabbix_export"', ({ assert }) => {
    assert.throws(() => parseZabbixTemplateExport({ foo: 'bar' }), ZabbixTemplateParseError)
  })

  test('deve rejeitar um export sem templates', ({ assert }) => {
    assert.throws(
      () => parseZabbixTemplateExport({ zabbix_export: { version: '7.0', templates: [] } }),
      ZabbixTemplateParseError
    )
  })

  test('deve rejeitar um template sem nenhum item SNMP_AGENT válido', ({ assert }) => {
    assert.throws(
      () =>
        parseZabbixTemplateExport({
          zabbix_export: {
            version: '7.0',
            templates: [
              { name: 'Template Vazio', items: [{ type: 'TRAPPER', name: 'x', key: 'x' }] },
            ],
          },
        }),
      ZabbixTemplateParseError
    )
  })
})

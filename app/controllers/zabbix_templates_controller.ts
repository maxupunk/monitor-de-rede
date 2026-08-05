import type { HttpContext } from '@adonisjs/core/http'
import { DateTime } from 'luxon'
import db from '@adonisjs/lucid/services/db'
import ZabbixTemplate from '#models/zabbix_template'
import ZabbixTemplateItem from '#models/zabbix_template_item'
import Device from '#models/device'
import {
  parseZabbixTemplateExport,
  ZabbixTemplateParseError,
} from '#modules/zabbix/zabbix_template_parser'
import { ResourceCleanupService } from '#services/resource_cleanup_service'

export default class ZabbixTemplatesController {
  private cleanupService = new ResourceCleanupService()

  async index({ response }: HttpContext) {
    const templates = await ZabbixTemplate.query().preload('items').orderBy('name', 'asc')

    const formatted = await Promise.all(
      templates.map(async (template) => {
        const deviceCount = await Device.query()
          .where('zabbixTemplateId', template.id)
          .count('* as total')
        return {
          id: template.id,
          zabbixUuid: template.zabbixUuid,
          name: template.name,
          description: template.description,
          zabbixVersion: template.zabbixVersion,
          importedAt: template.importedAt,
          deviceCount: Number(
            (deviceCount[0] as unknown as { $extras: { total: number } }).$extras.total
          ),
          items: template.items.map((item) => ({
            id: item.id,
            name: item.name,
            key: item.key,
            snmpOid: item.snmpOid,
            valueType: item.valueType,
            units: item.units,
            multiplier: item.multiplier,
          })),
        }
      })
    )

    return response.ok(formatted)
  }

  async show({ params, response }: HttpContext) {
    const template = await ZabbixTemplate.query().where('id', params.id).preload('items').first()
    if (!template) {
      return response.notFound({ message: 'Template Zabbix não encontrado' })
    }
    return response.ok(template)
  }

  /**
   * Importa um export de template do Zabbix (JSON, formato oficial —
   * https://www.zabbix.com/documentation/current/en/manual/xml_export_import/templates).
   * Um mesmo arquivo pode conter mais de um template; todos são importados.
   * Reimportar um template com o mesmo uuid substitui seus itens (mantendo o id
   * e, portanto, os dispositivos já vinculados a ele).
   */
  async store({ request, response }: HttpContext) {
    const content = request.input('content')
    if (!content || typeof content !== 'string') {
      return response.badRequest({
        message: 'Envie o conteúdo do arquivo de export do Zabbix no campo "content".',
      })
    }

    let rawData: unknown
    try {
      rawData = JSON.parse(content)
    } catch {
      return response.badRequest({ message: 'O conteúdo enviado não é um JSON válido.' })
    }

    let parsedTemplates
    try {
      parsedTemplates = parseZabbixTemplateExport(rawData)
    } catch (error) {
      if (error instanceof ZabbixTemplateParseError) {
        return response.unprocessableEntity({ message: error.message })
      }
      throw error
    }

    const imported = []

    for (const parsed of parsedTemplates) {
      const result = await db.transaction(async (trx) => {
        let template = parsed.uuid
          ? await ZabbixTemplate.query()
              .useTransaction(trx)
              .where('zabbixUuid', parsed.uuid)
              .first()
          : null

        if (template) {
          template.useTransaction(trx)
          await ZabbixTemplateItem.query()
            .useTransaction(trx)
            .where('templateId', template.id)
            .delete()
        } else {
          template = new ZabbixTemplate()
          template.useTransaction(trx)
          template.zabbixUuid = parsed.uuid
        }

        template.name = parsed.name
        template.description = parsed.description
        template.zabbixVersion = parsed.zabbixVersion
        template.rawExport = rawData as Record<string, unknown>
        template.importedAt = DateTime.now()
        await template.save()

        for (const parsedItem of parsed.items) {
          const item = new ZabbixTemplateItem()
          item.useTransaction(trx)
          item.templateId = template.id
          item.zabbixUuid = parsedItem.uuid
          item.name = parsedItem.name
          item.key = parsedItem.key
          item.snmpOid = parsedItem.snmpOid
          item.valueType = parsedItem.valueType
          item.units = parsedItem.units
          item.multiplier = parsedItem.multiplier
          await item.save()
        }

        return template
      })

      imported.push({
        id: result.id,
        name: result.name,
        itemCount: parsed.items.length,
        skippedItems: parsed.skippedItems,
      })
    }

    return response.created({ templates: imported })
  }

  async destroy({ params, response }: HttpContext) {
    const template = await ZabbixTemplate.findOrFail(params.id)
    await this.cleanupService.deleteZabbixTemplate(template.id)
    return response.noContent()
  }
}

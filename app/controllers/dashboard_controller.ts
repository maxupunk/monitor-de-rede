import type { HttpContext } from '@adonisjs/core/http'
import vine from '@vinejs/vine'
import SystemSetting from '#models/system_setting'
import { EventBus } from '#modules/events/event_bus'

export default class DashboardController {
  async getLayout({ response }: HttpContext) {
    const setting = await SystemSetting.findBy('key', 'dashboard_layout')
    if (!setting || !setting.value) {
      return response.ok({ layout: null, updatedAt: null })
    }

    try {
      const layout = JSON.parse(setting.value)
      return response.ok({
        layout,
        updatedAt: setting.updatedAt ? setting.updatedAt.toISO() : null,
      })
    } catch {
      return response.ok({ layout: null, updatedAt: null })
    }
  }

  async saveLayout({ request, response }: HttpContext) {
    const schema = vine.object({
      layout: vine.array(vine.any()),
      clientId: vine.string().optional(),
    })

    const payload = await vine.validate({ schema, data: request.all() })
    const serializedValue = JSON.stringify(payload.layout)

    let setting = await SystemSetting.findBy('key', 'dashboard_layout')
    if (setting) {
      setting.value = serializedValue
      await setting.save()
    } else {
      setting = await SystemSetting.create({
        key: 'dashboard_layout',
        value: serializedValue,
      })
    }

    const updatedAt = setting.updatedAt ? setting.updatedAt.toISO() ?? new Date().toISOString() : new Date().toISOString()

    // Emite evento SSE para todos os clientes conectados sincronizarem o layout do dashboard
    EventBus.getInstance().emit('dashboard:layout_updated', {
      layout: payload.layout,
      updatedAt,
      clientId: payload.clientId ?? null,
    })

    return response.ok({
      success: true,
      updatedAt,
    })
  }
}

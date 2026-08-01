import type { HttpContext } from '@adonisjs/core/http'

export default class DiscoveryController {
  async runs({ response }: HttpContext) {
    return response.ok([])
  }

  async runDetails({ params, response }: HttpContext) {
    return response.ok({ id: params.id, status: 'completed' })
  }

  async results({ response }: HttpContext) {
    return response.ok([])
  }

  async accept({ params, response }: HttpContext) {
    return response.ok({ message: `Resultado de descoberta ID ${params.id} aceito` })
  }

  async ignore({ params, response }: HttpContext) {
    return response.ok({ message: `Resultado de descoberta ID ${params.id} ignorado` })
  }

  async merge({ params, response }: HttpContext) {
    return response.ok({ message: `Resultado de descoberta ID ${params.id} mesclado` })
  }
}

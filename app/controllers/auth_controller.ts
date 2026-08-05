import type { HttpContext } from '@adonisjs/core/http'

export default class AuthController {
  async login({ request, response }: HttpContext) {
    const credentials = request.only(['email', 'password'])
    return response.ok({
      message: 'Autenticado com sucesso',
      user: credentials.email,
      token: 'sample-token',
    })
  }

  async logout({ response }: HttpContext) {
    return response.ok({ message: 'Sessão encerrada com sucesso' })
  }

  async me({ response }: HttpContext) {
    return response.ok({ user: null })
  }
}

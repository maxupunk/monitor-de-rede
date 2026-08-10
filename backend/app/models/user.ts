import { DateTime } from 'luxon'
import { compose } from '@adonisjs/core/helpers'
import hash from '@adonisjs/core/services/hash'
import { BaseModel, column } from '@adonisjs/lucid/orm'
import { withAuthFinder } from '@adonisjs/auth/mixins/lucid'
import { DbAccessTokensProvider } from '@adonisjs/auth/access_tokens'

/**
 * O mixin registra o hook `beforeSave` que aplica o hash na senha e expõe
 * `verifyCredentials`, que compara a senha em tempo constante — sem ele, cada
 * ponto que autentica precisaria lembrar de fazer o hash na mão.
 */
const AuthFinder = withAuthFinder(() => hash.use('scrypt'), {
  uids: ['email'],
  passwordColumnName: 'password',
})

export default class User extends compose(BaseModel, AuthFinder) {
  @column({ isPrimary: true })
  declare id: number

  @column()
  declare name: string

  @column()
  declare email: string

  @column({ serializeAs: null })
  declare password: string

  @column()
  declare active: boolean

  @column.dateTime({ autoCreate: true })
  declare createdAt: DateTime

  @column.dateTime({ autoCreate: true, autoUpdate: true })
  declare updatedAt: DateTime

  /**
   * Emissor dos tokens de API guardados em `auth_access_tokens`. O guard padrão
   * é o de sessão (`config/auth.ts`); os tokens atendem os consumidores que não
   * carregam cookie — probes e integrações.
   */
  static accessTokens = DbAccessTokensProvider.forModel(User)
}

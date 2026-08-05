import AlertRule from '#models/alert_rule'
import { AlertRuleRepository } from '../alert_rule_repository.js'
import {
  ALERT_RULE_TEMPLATES,
  type AlertRuleTemplate,
  type AlertRuleCategory,
} from './alert_rule_templates.js'

/** Template acrescido do que já existe no banco — o que a tela precisa saber. */
export interface AlertRuleTemplateView extends AlertRuleTemplate {
  /** Já existe uma regra equivalente: não será criada de novo */
  applied: boolean
  /** Regra existente correspondente, quando houver */
  ruleId: number | null
}

export type SkipReason = 'already_exists' | 'unknown_template'

export interface CatalogApplicationResult {
  created: AlertRule[]
  skipped: Array<{ key: string; reason: SkipReason }>
}

/**
 * Aplica regras do catálogo de forma idempotente.
 *
 * "Já existe" cobre dois casos: a regra veio do mesmo template (`templateKey`)
 * ou o usuário já criou à mão uma regra com condição e escopo idênticos. Em
 * ambos, o template é ignorado — nunca duplicamos.
 */
export class AlertRuleCatalogService {
  constructor(
    private readonly templates: readonly AlertRuleTemplate[] = ALERT_RULE_TEMPLATES,
    private readonly rules: AlertRuleRepository = new AlertRuleRepository()
  ) {}

  /** Assinatura que identifica uma regra por condição + escopo. */
  private signature(rule: {
    condition: Record<string, unknown>
    siteId?: number | null
    deviceId?: number | null
    monitorId?: number | null
  }): string {
    const { field, operator, value } = rule.condition ?? {}
    return [
      String(field ?? ''),
      String(operator ?? ''),
      String(value ?? ''),
      rule.siteId ?? '',
      rule.deviceId ?? '',
      rule.monitorId ?? '',
    ].join('|')
  }

  private templateSignature(template: AlertRuleTemplate): string {
    return this.signature({
      condition: template.condition as unknown as Record<string, unknown>,
      siteId: null,
      deviceId: null,
      monitorId: null,
    })
  }

  /** Índices de tudo que já existe, para decidir sem N consultas. */
  private async loadExisting(): Promise<{
    byTemplateKey: Map<string, AlertRule>
    bySignature: Map<string, AlertRule>
  }> {
    const existing = await this.rules.findAll()
    const byTemplateKey = new Map<string, AlertRule>()
    const bySignature = new Map<string, AlertRule>()

    for (const rule of existing) {
      if (rule.templateKey && !byTemplateKey.has(rule.templateKey)) {
        byTemplateKey.set(rule.templateKey, rule)
      }
      const signature = this.signature(rule)
      if (!bySignature.has(signature)) bySignature.set(signature, rule)
    }

    return { byTemplateKey, bySignature }
  }

  private match(
    template: AlertRuleTemplate,
    indexes: { byTemplateKey: Map<string, AlertRule>; bySignature: Map<string, AlertRule> }
  ): AlertRule | undefined {
    return (
      indexes.byTemplateKey.get(template.key) ??
      indexes.bySignature.get(this.templateSignature(template))
    )
  }

  /** Catálogo completo com a marcação do que já está configurado. */
  async describe(): Promise<AlertRuleTemplateView[]> {
    const indexes = await this.loadExisting()

    return this.templates.map((template) => {
      const existing = this.match(template, indexes)
      return { ...template, applied: Boolean(existing), ruleId: existing?.id ?? null }
    })
  }

  /** Cria as regras das chaves informadas, pulando as que já existem. */
  async apply(keys: string[]): Promise<CatalogApplicationResult> {
    const result: CatalogApplicationResult = { created: [], skipped: [] }
    const indexes = await this.loadExisting()
    const requested = [...new Set(keys)]

    for (const key of requested) {
      const template = this.templates.find((item) => item.key === key)
      if (!template) {
        result.skipped.push({ key, reason: 'unknown_template' })
        continue
      }

      if (this.match(template, indexes)) {
        result.skipped.push({ key, reason: 'already_exists' })
        continue
      }

      const rule = await AlertRule.create({
        name: template.name,
        type: template.type,
        templateKey: template.key,
        condition: template.condition as unknown as Record<string, unknown>,
        severity: template.severity,
        durationSeconds: template.durationSeconds,
        enabled: true,
      })

      // Mantém os índices coerentes dentro do próprio lote (evita duplicar
      // quando duas chaves resolvem para a mesma condição).
      indexes.byTemplateKey.set(template.key, rule)
      indexes.bySignature.set(this.templateSignature(template), rule)
      result.created.push(rule)
    }

    return result
  }

  /**
   * Provisiona o conjunto básico de regras em instalações novas.
   *
   * Só age quando não existe regra alguma: quem já opera o sistema decide o que
   * manter, e uma regra apagada de propósito não pode ressuscitar no restart.
   */
  async ensureDefaults(): Promise<CatalogApplicationResult> {
    if ((await this.rules.count()) > 0) {
      return { created: [], skipped: [] }
    }

    return this.apply(
      this.templates.filter((template) => template.recommended).map((template) => template.key)
    )
  }

  categories(): AlertRuleCategory[] {
    return [...new Set(this.templates.map((template) => template.category))]
  }
}

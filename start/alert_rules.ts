/*
|--------------------------------------------------------------------------
| Regras básicas de alerta
|--------------------------------------------------------------------------
|
| Executado na inicialização do servidor HTTP. Em instalações novas — banco sem
| nenhuma regra — aplica o conjunto básico do catálogo, para que a Central de
| Alertas já nasça monitorando indisponibilidade, latência, perda de pacotes,
| erro HTTP e quedas/downgrades de interface.
|
| Instalações que já possuem regras não são tocadas: uma regra removida de
| propósito não pode voltar sozinha no próximo restart.
|
*/

import logger from '@adonisjs/core/services/logger'
import { AlertRuleCatalogService } from '#modules/alerts/catalog/alert_rule_catalog_service'

try {
  const { created } = await new AlertRuleCatalogService().ensureDefaults()

  if (created.length > 0) {
    logger.info(`[Alertas] ${created.length} regras básicas aplicadas a partir do catálogo`)
  }
} catch (error: unknown) {
  // Falha aqui não pode impedir a API de subir (ex.: banco ainda migrando).
  const message = error instanceof Error ? error.message : String(error)
  logger.warn(`[Alertas] não foi possível provisionar as regras básicas: ${message}`)
}

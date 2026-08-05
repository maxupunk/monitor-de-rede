import app from '@adonisjs/core/services/app'
import { EventBus } from '#modules/events/event_bus'

/**
 * O EventBus é um singleton em memória, mas o monitoramento roda em processos
 * separados do servidor HTTP (`scheduler:run`, `queue:work`, `probe:run`).
 * Ligar a publicação na caixa de saída faz os eventos desses processos
 * chegarem às conexões SSE mantidas pelo servidor — sem isso, a interface só
 * atualizaria com recarregamento manual.
 */
const eventBus = EventBus.getInstance()
eventBus.enableCrossProcessPublishing()

/**
 * Drena as publicações pendentes enquanto o pool de conexões ainda existe.
 * Em comandos de vida curta (`monitor:test`, `snmp:poll`) o encerramento
 * chegava antes da escrita e o último evento era abortado.
 */
app.terminating(async () => {
  await eventBus.flush()
})

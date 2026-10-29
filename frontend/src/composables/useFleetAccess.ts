import type { BatchDevice } from '@/bindings/BatchDevice'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import { confirm } from '@/composables/useConfirm'
import { requestAccess } from '@/composables/useDeviceAccess'
import { useAuthStore } from '@/stores/auth'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { accessNeededReason } from '@/utils/deviceCredentials'
import { fieldsOf } from '@/utils/pluginSettings'

/**
 * O equipamento acabou de entrar no aplicativo (adicionado ou cadastrado ali
 * mesmo) e ainda não tem o acesso que o plugin usa: pede agora, com "Agora
 * não" à mão. Salvo, a página relê a frota (o cartão deixa de dizer "Sem
 * acesso").
 */
export async function askMemberAccess(pluginId: number, deviceId: number): Promise<boolean> {
  const store = usePluginAppsStore()
  const member = store.fleets[pluginId]?.members.find((item) => item.deviceId === deviceId)
  if (!member?.missingAccess || !useAuthStore().canManagePlugins) return false
  const saved = await requestAccess({
    deviceId,
    transport: member.missingAccess,
    reason: accessNeededReason(member.name, member.missingAccess),
  })
  if (!saved) return false
  await store.loadFleet(pluginId)
  return true
}

/**
 * Nenhum equipamento do aplicativo tem o acesso e o operador pediu uma ação:
 * em vez de mandá-lo à aba Equipamentos, a janela do acesso abre ali mesmo,
 * um equipamento por vez ("2 de 3"); "Agora não" encerra a fila. Salvo algum,
 * a frota é relida. `true` quando ao menos um ficou pronto.
 */
export async function askFleetAccess(pluginId: number): Promise<boolean> {
  const store = usePluginAppsStore()
  if (!useAuthStore().canManagePlugins) return false
  const missing = (store.fleets[pluginId]?.members ?? []).filter(
    (member) => !member.credentialsReady && member.missingAccess
  )
  let saved = 0
  for (const [index, member] of missing.entries()) {
    const transport = member.missingAccess ?? 'ssh'
    const position = missing.length > 1 ? ` (${index + 1} de ${missing.length})` : ''
    const credential = await requestAccess({
      deviceId: member.deviceId,
      transport,
      reason: accessNeededReason(member.name, transport) + position,
    })
    if (!credential) break
    saved += 1
  }
  if (saved === 0) return false
  await store.loadFleet(pluginId)
  return (store.fleets[pluginId]?.members ?? []).some((member) => member.credentialsReady)
}

/**
 * Depois de salvar o acesso que faltava a um equipamento do lote:
 *
 * * `started` — a ação rodou de novo só nele (direto numa leitura, depois de
 *   confirmar numa escrita), com os mesmos valores do lote;
 * * `declined` — escrita, e o operador preferiu não repetir;
 * * `needsForm` — a ação tem parâmetros e esta aba não guardou os do lote
 *   (lançado em outro lugar, ou a página foi recarregada): só o formulário
 *   pode rodá-la de novo.
 */
export type RerunOutcome = 'started' | 'declined' | 'needsForm'

export async function rerunAfterAccess(
  batch: PluginBatchView,
  device: BatchDevice,
  action: PluginAction
): Promise<RerunOutcome> {
  const store = usePluginAppsStore()
  const kept = store.canRerun(batch.id, device.deviceId)
  if (!kept && fieldsOf(action.params).length > 0) return 'needsForm'
  const write = action.effect !== 'read'
  if (write) {
    const ok = await confirm({
      title: batch.title,
      message: `Acesso salvo. Rodar «${batch.title}» de novo em ${device.deviceName}? Isso altera o equipamento.`,
      confirmText: 'Rodar',
      confirmColor: 'warning',
    })
    if (!ok) return 'declined'
  }
  if (kept) await store.rerunDevice(batch.id, device.deviceId, write)
  else await store.runAction(batch.pluginId, batch.action, [device.deviceId], write)
  return 'started'
}

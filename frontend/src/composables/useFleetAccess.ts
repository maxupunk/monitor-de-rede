import { requestAccess } from '@/composables/useDeviceAccess'
import { useAuthStore } from '@/stores/auth'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { accessNeededReason } from '@/utils/deviceCredentials'

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

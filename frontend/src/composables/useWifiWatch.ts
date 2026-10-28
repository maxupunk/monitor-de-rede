import { confirm, prompt } from '@/composables/useConfirm'
import type { WifiWatchedClient } from '@/bindings/WifiWatchedClient'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { stationName, type WifiStationRow } from '@/utils/wifiPresentation'

const NAME_RULES = [
  (value: string) => value.trim().length > 0 || 'Dê um nome ao aparelho.',
  (value: string) => value.trim().length <= 120 || 'Até 120 caracteres.',
]

const PRIVATE_MAC_WARNING =
  ' Atenção: este aparelho usa MAC privado, que pode mudar — desative o MAC privado nele para o alerta não disparar à toa.'

/**
 * Vigiar, renomear e deixar de vigiar um aparelho — os diálogos num lugar só,
 * para a tabela e o painel dos vigiados falarem a mesma língua.
 */
export function useWifiWatch() {
  const store = useWifiClientsStore()

  async function startWatching(row: WifiStationRow): Promise<void> {
    const name = await prompt({
      title: 'Vigiar este aparelho',
      message:
        'Se ele sumir de todos os roteadores por 10 minutos, ou ficar com sinal ruim, vira alerta. Use para câmeras, IoT e o que deveria estar sempre conectado.' +
        (row.randomizedMac ? PRIVATE_MAC_WARNING : ''),
      inputLabel: 'Nome do aparelho',
      placeholder: 'Câmera da portaria',
      defaultValue: stationName(row),
      confirmText: 'Vigiar',
      icon: 'mdi-eye-plus-outline',
      rules: NAME_RULES,
    })
    if (name) await store.watch(row.mac, name.trim())
  }

  async function rename(client: WifiWatchedClient): Promise<void> {
    const name = await prompt({
      title: 'Renomear aparelho vigiado',
      message: `MAC ${client.mac}. O nome aparece nos alertas.`,
      inputLabel: 'Nome do aparelho',
      defaultValue: client.name,
      confirmText: 'Salvar',
      icon: 'mdi-pencil',
      rules: NAME_RULES,
    })
    if (name && name.trim() !== client.name) await store.renameWatched(client.id, name.trim())
  }

  async function stopWatching(client: WifiWatchedClient): Promise<void> {
    const ok = await confirm({
      title: 'Deixar de vigiar?',
      message: `${client.name} deixa de gerar alerta de Wi-Fi, e o alerta aberto dele é encerrado.`,
      confirmText: 'Deixar de vigiar',
      confirmColor: 'warning',
      icon: 'mdi-eye-off-outline',
    })
    if (ok) await store.unwatch(client.id)
  }

  return { startWatching, rename, stopWatching }
}

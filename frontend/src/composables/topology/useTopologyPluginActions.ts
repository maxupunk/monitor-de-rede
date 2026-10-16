import { usePluginsStore } from '@/stores/plugins'
import { useTopologyStore } from '@/stores/topology'

/**
 * Carrega as ações de plugin do mapa pela consulta única do store de plugins
 * (`fetchTopologyActions`). Falha (rota ausente, sem permissão) = nenhuma ação.
 */
export function useTopologyPluginActions() {
  const topology = useTopologyStore()
  const plugins = usePluginsStore()

  async function load(): Promise<void> {
    try {
      topology.setPluginActions(await plugins.fetchTopologyActions())
    } catch {
      topology.setPluginActions({})
    }
  }

  return { load }
}

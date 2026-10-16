import { usePluginsStore } from '@/stores/plugins'
import { parsePluginActionMap, useTopologyStore } from '@/stores/topology'

/**
 * Carrega as ações de plugin do mapa. Quando o store de plugins já oferece
 * `fetchTopologyActions`, usa ele (fonte única); senão, o método enxuto do
 * store da topologia faz a mesma consulta.
 */
export function useTopologyPluginActions() {
  const topology = useTopologyStore()
  const plugins = usePluginsStore()

  async function load(): Promise<void> {
    if ('fetchTopologyActions' in plugins && typeof plugins.fetchTopologyActions === 'function') {
      try {
        const raw: unknown = await plugins.fetchTopologyActions()
        const parsed = parsePluginActionMap(raw)
        if (Object.keys(parsed).length > 0) {
          topology.setPluginActions(parsed)
          return
        }
      } catch {
        // Cai na consulta própria abaixo.
      }
    }
    await topology.fetchPluginActions()
  }

  return { load }
}

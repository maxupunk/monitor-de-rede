import type { Area } from '@/bindings/Area'
import type { AreaModule, ServerConnectPathSpec } from './defineArea'

/**
 * Os caminhos para conectar um servidor remoto, do que as áreas declaram
 * (ADR 017). Só entra o que é de área visível, na ordem declarada.
 */
export function buildServerConnectPaths(
  modules: readonly AreaModule[],
  isVisible: (area: Area) => boolean
): ServerConnectPathSpec[] {
  return modules
    .filter((module) => isVisible(module.id))
    .flatMap((module) => module.serverConnectPaths)
    .sort((a, b) => a.order - b.order)
}

/** O caminho que já vem marcado: o recomendado e, sem ele, o primeiro. */
export function defaultServerConnectPath(
  paths: readonly ServerConnectPathSpec[]
): ServerConnectPathSpec | null {
  return paths.find((path) => path.recommended) ?? paths[0] ?? null
}

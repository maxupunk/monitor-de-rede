import { watch } from 'vue'
import type { Area } from '@/bindings/Area'
import { useAreasStore } from '@/stores/areas'
import { useEventsStore } from '@/stores/events'
import { AREAS, type AreaModule } from './index'

/**
 * Liga o barramento SSE às áreas: as rotas de cada área ficam registradas
 * enquanto ela está visível (ligada e com acesso, ADR 019) e saem quando
 * deixa de estar. O núcleo fica sempre — é
 * ele que recebe o `areas:updated`, então a mudança chega mesmo com tudo
 * desligado.
 *
 * Retorna a função que desfaz tudo (logout, desmontagem do layout).
 */
export function installAreaRealtime(modules: readonly AreaModule[] = AREAS): () => void {
  const areas = useAreasStore()
  const events = useEventsStore()
  const installed = new Map<Area, () => void>()

  function sync() {
    for (const module of modules) {
      const on = areas.isVisible(module.id)
      const current = installed.get(module.id)
      if (on && !current) installed.set(module.id, events.registerRoutes(module.sse))
      if (!on && current) {
        current()
        installed.delete(module.id)
      }
    }
  }

  // `sync`: o evento seguinte ao `areas:updated` já encontra as rotas certas.
  const stop = watch(() => areas.visibleAreas, sync, { immediate: true, flush: 'sync' })

  return () => {
    stop()
    for (const remove of installed.values()) remove()
    installed.clear()
  }
}

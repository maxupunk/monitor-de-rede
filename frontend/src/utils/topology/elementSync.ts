/**
 * Aplica os elementos (`buildElements`) num Cytoscape por diferença: dado e
 * classe que mudaram, elemento novo, elemento que saiu — sem recriar o grafo
 * (o desenho não pisca e a posição arrastada fica). Usado pelo mapa
 * (`TopologyCanvas`) e pelo mini mapa do teste (`LinkTestPathMap`).
 */
import type { CollectionReturnValue, Core, NodeSingular } from 'cytoscape'
import type { TopologyElement } from './elements'

/** Onde o elemento guarda as classes que vieram do estado (as outras são do canvas). */
const BASE_CLASSES = 'baseClasses'

/**
 * Sincroniza `elements` em `core`, num `batch` só. `onAdded` recebe os
 * recém-criados antes da remoção dos que saíram (o canvas aplica ali o nível
 * de detalhe e o traçado). Elementos com id `__…` são do canvas e ficam.
 */
export function syncElements(
  core: Core,
  elements: ReadonlyArray<TopologyElement>,
  onAdded?: (added: CollectionReturnValue) => void
): void {
  core.batch(() => {
    const nextIds = new Set(elements.map((el) => el.data.id))
    const toAdd: TopologyElement[] = []

    for (const el of elements) {
      const existing = core.getElementById(el.data.id)
      if (existing.empty()) {
        toAdd.push(el)
        continue
      }
      const current = existing.data() as Record<string, unknown>
      const patch: Record<string, unknown> = {}
      let changed = false
      for (const [key, value] of Object.entries(el.data)) {
        if (key === 'parent' || key === 'source' || key === 'target') continue
        if (current[key] !== value) {
          patch[key] = value
          changed = true
        }
      }
      if (changed) existing.data(patch)
      if (existing.isNode()) {
        const wanted = (el.data.parent as string | undefined) ?? null
        const now = (existing.data('parent') as string | undefined) ?? null
        if (wanted !== now) (existing as NodeSingular).move({ parent: wanted })
      }
      const previous = (existing.scratch(BASE_CLASSES) as string | undefined) ?? ''
      if (previous !== el.classes) {
        existing.removeClass(previous)
        existing.addClass(el.classes)
        existing.scratch(BASE_CLASSES, el.classes)
      }
    }

    if (toAdd.length > 0) {
      const added = core.add(toAdd.map((el) => ({ ...el, data: { ...el.data } })))
      added.forEach((ele) => {
        const def = toAdd.find((item) => item.data.id === ele.id())
        ele.scratch(BASE_CLASSES, def?.classes ?? '')
      })
      onAdded?.(added)
    }

    // Remoção por último: um site (nó composto) só sai depois que os filhos
    // já foram movidos para fora dele.
    core
      .elements()
      .filter((ele) => !nextIds.has(ele.id()) && !ele.id().startsWith('__'))
      .remove()
  })
}

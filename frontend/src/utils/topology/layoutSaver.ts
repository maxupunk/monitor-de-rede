/**
 * Junta as posições movidas e grava num único `PATCH` depois de um intervalo
 * sem novos movimentos. Arrastar dez nós em sequência vira uma requisição só,
 * com os dez. Se a gravação falha, as posições continuam pendentes e voltam
 * no próximo envio (ou no "Tentar de novo" da tela).
 */
import type { TopologyLayoutNode, TopologyPosition } from '@/stores/topology'

export interface LayoutSaverOptions {
  delayMs?: number
  save: (nodes: TopologyLayoutNode[]) => Promise<unknown>
  onError?: (error: unknown) => void
  onSaved?: () => void
  /** Injetável nos testes. */
  setTimer?: (fn: () => void, ms: number) => unknown
  clearTimer?: (handle: unknown) => void
}

export interface LayoutSaver {
  queue: (positions: Record<number, TopologyPosition>) => void
  flush: () => Promise<boolean>
  cancel: () => void
  readonly pendingCount: number
  readonly saving: boolean
}

export function createLayoutSaver(options: LayoutSaverOptions): LayoutSaver {
  const delay = options.delayMs ?? 800
  const setTimer = options.setTimer ?? ((fn, ms) => setTimeout(fn, ms))
  const clearTimer =
    options.clearTimer ?? ((handle) => clearTimeout(handle as ReturnType<typeof setTimeout>))
  const pending = new Map<number, TopologyPosition>()
  let timer: unknown = null
  let saving = false
  let again = false

  function schedule() {
    if (timer !== null) clearTimer(timer)
    timer = setTimer(() => {
      timer = null
      void flush()
    }, delay)
  }

  async function flush(): Promise<boolean> {
    if (timer !== null) {
      clearTimer(timer)
      timer = null
    }
    if (pending.size === 0) return true
    if (saving) {
      again = true
      return false
    }
    const batch = [...pending.entries()].map(([deviceId, pos]) => ({
      deviceId,
      x: Math.round(pos.x),
      y: Math.round(pos.y),
    }))
    pending.clear()
    saving = true
    try {
      await options.save(batch)
      options.onSaved?.()
      return true
    } catch (error: unknown) {
      // Devolve o lote sem atropelar o que foi movido durante o envio.
      for (const item of batch) {
        if (!pending.has(item.deviceId)) pending.set(item.deviceId, { x: item.x, y: item.y })
      }
      options.onError?.(error)
      return false
    } finally {
      saving = false
      if (again) {
        again = false
        schedule()
      }
    }
  }

  return {
    queue(positions) {
      for (const [id, pos] of Object.entries(positions)) pending.set(Number(id), pos)
      if (pending.size > 0) schedule()
    },
    flush,
    cancel() {
      if (timer !== null) clearTimer(timer)
      timer = null
      pending.clear()
    },
    get pendingCount() {
      return pending.size
    },
    get saving() {
      return saving
    },
  }
}

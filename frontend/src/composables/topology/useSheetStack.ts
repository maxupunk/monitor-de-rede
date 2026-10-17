import { computed, inject, provide, ref, type InjectionKey } from 'vue'

/** As folhas inferiores do mapa no celular. */
export type SheetId = 'inspector' | 'link-test' | 'options'

/**
 * Política única das folhas no celular: uma visível por vez — a última aberta
 * vence; fechar (X, arrastar para baixo ou o gesto de voltar, que o Vuetify
 * converte em fechar a folha ativa) devolve a anterior. Ex.: inspetor →
 * "Ver último teste" abre o teste por cima; voltar reabre o inspetor.
 */
export interface SheetStack {
  readonly top: SheetId | null
  open(id: SheetId): void
  close(id: SheetId): void
  isTop(id: SheetId): boolean
}

/** Puras: a pilha nunca repete uma folha. */
export function pushSheet(stack: readonly SheetId[], id: SheetId): SheetId[] {
  return [...stack.filter((item) => item !== id), id]
}

export function removeSheet(stack: readonly SheetId[], id: SheetId): SheetId[] {
  return stack.filter((item) => item !== id)
}

export function createSheetStack(): SheetStack {
  const items = ref<SheetId[]>([])
  const top = computed(() => items.value[items.value.length - 1] ?? null)
  return {
    get top() {
      return top.value
    },
    open(id) {
      if (top.value !== id) items.value = pushSheet(items.value, id)
    },
    close(id) {
      if (items.value.includes(id)) items.value = removeSheet(items.value, id)
    },
    isTop(id) {
      return top.value === id
    },
  }
}

const KEY: InjectionKey<SheetStack> = Symbol('topologySheets')

/** A página cria a pilha; os filhos (camada do teste) usam a mesma. */
export function provideSheetStack(): SheetStack {
  const stack = createSheetStack()
  provide(KEY, stack)
  return stack
}

export function useSheetStack(): SheetStack {
  return inject(KEY, null) ?? createSheetStack()
}

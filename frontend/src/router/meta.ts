import type { Area } from '@/bindings/Area'

declare module 'vue-router' {
  interface RouteMeta {
    /** Área do produto (ADR 017); carimbada pelo `defineArea`. */
    area?: Area
    /** Acessível sem sessão (login, cadastro inicial). */
    public?: boolean
    requiresAdmin?: boolean
    /** A página ocupa a tela inteira, sem o contêiner do layout. */
    fullBleed?: boolean
  }
}

export {}

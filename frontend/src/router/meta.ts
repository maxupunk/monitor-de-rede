import type { Area } from '@/bindings/Area'
import type { AiScreenMeta } from '@/utils/aiScreen'

declare module 'vue-router' {
  interface RouteMeta {
    /** Área do produto (ADR 017); carimbada pelo `defineArea`. */
    area?: Area
    /** Acessível sem sessão (login, cadastro inicial). */
    public?: boolean
    requiresAdmin?: boolean
    /** A página ocupa a tela inteira, sem o contêiner do layout. */
    fullBleed?: boolean
    /** Como a tela se apresenta ao chat da IA (o chip "Tela" do composer). */
    aiScreen?: AiScreenMeta
  }
}

export {}

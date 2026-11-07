import type { RouteLocationNormalized, RouteLocationRaw } from 'vue-router'
import type { Area } from '@/bindings/Area'
import './meta'

/** A área (fora o núcleo) da rota: a do registro mais interno que declara uma. */
export function areaOfRoute(to: Pick<RouteLocationNormalized, 'matched'>): Area | null {
  const area = [...to.matched].reverse().find((record) => record.meta.area)?.meta.area
  return area && area !== 'core' ? area : null
}

/**
 * Rota de área desligada — ou de área a que o usuário não tem acesso (ADR
 * 019) — vai para a `AreaOffPage`, que explica o motivo, levando o endereço
 * pedido em `from` para voltar a ele quando a área ligar ou o acesso chegar.
 * Link profundo nunca vira 404.
 */
export function areaOffRedirect(
  to: Pick<RouteLocationNormalized, 'matched' | 'fullPath'>,
  isEnabled: (area: Area) => boolean,
  hasAccess: (area: Area) => boolean = () => true
): RouteLocationRaw | null {
  const area = areaOfRoute(to)
  if (!area) return null
  if (!isEnabled(area)) return { name: 'area-off', params: { area }, query: { from: to.fullPath } }
  if (!hasAccess(area)) {
    return { name: 'area-off', params: { area }, query: { from: to.fullPath, motivo: 'acesso' } }
  }
  return null
}

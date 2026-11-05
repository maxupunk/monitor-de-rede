import type { RouteLocationNormalized, RouteLocationRaw } from 'vue-router'
import type { Area } from '@/bindings/Area'
import './meta'

/** A área (fora o núcleo) da rota: a do registro mais interno que declara uma. */
export function areaOfRoute(to: Pick<RouteLocationNormalized, 'matched'>): Area | null {
  const area = [...to.matched].reverse().find((record) => record.meta.area)?.meta.area
  return area && area !== 'core' ? area : null
}

/**
 * Rota de área desligada vai para a `AreaOffPage`, levando o endereço pedido
 * em `from` para voltar a ele quando a área ligar. Link profundo nunca vira 404.
 */
export function areaOffRedirect(
  to: Pick<RouteLocationNormalized, 'matched' | 'fullPath'>,
  isEnabled: (area: Area) => boolean
): RouteLocationRaw | null {
  const area = areaOfRoute(to)
  if (!area || isEnabled(area)) return null
  return { name: 'area-off', params: { area }, query: { from: to.fullPath } }
}

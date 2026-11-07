import type { Area } from '@/bindings/Area'
import type { AreaModule, SettingsCardSpec, SettingsTabSpec } from './defineArea'

/** Uma aba pronta para desenhar: os cartões dela, em ordem. */
export interface SettingsTabView extends SettingsTabSpec {
  cards: SettingsCardSpec[]
}

/**
 * Monta as abas de Configurações a partir do que as áreas declaram (ADR 017).
 * Só entra o que é de área visível; aba sem cartão visível não aparece.
 */
export function buildSettingsTabs(
  modules: readonly AreaModule[],
  isVisible: (area: Area) => boolean
): SettingsTabView[] {
  const visible = modules.filter((module) => isVisible(module.id))
  const cards = visible.flatMap((module) => module.settingsCards)
  return visible
    .flatMap((module) => module.settingsTabs)
    .sort((a, b) => a.order - b.order)
    .map((tab) => ({
      ...tab,
      cards: cards.filter((card) => card.tab === tab.value).sort((a, b) => a.order - b.order),
    }))
    .filter((tab) => tab.cards.length > 0)
}

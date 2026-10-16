import { computed } from 'vue'
import { useTheme } from 'vuetify'
import { paletteFromTheme, type TopologyPalette } from '@/utils/topology/style'

/** Paleta do mapa a partir do tema Vuetify ativo — muda junto com o tema. */
export function useTopologyPalette() {
  const theme = useTheme()
  return computed<TopologyPalette>(() =>
    paletteFromTheme({ dark: theme.current.value.dark, colors: theme.current.value.colors })
  )
}

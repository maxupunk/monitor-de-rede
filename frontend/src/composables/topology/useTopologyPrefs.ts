import { reactive, watch } from 'vue'
import { loadPrefs, savePrefs, type TopologyPrefs } from '@/utils/topology/prefs'

/** Preferências do visitante, reativas e gravadas a cada mudança. */
export function useTopologyPrefs() {
  const prefs = reactive<TopologyPrefs>(loadPrefs())
  // A legenda abre sozinha só nesta primeira visita.
  prefs.legendSeen = true
  watch(prefs, (value) => savePrefs({ ...value, filters: { ...value.filters } }), {
    deep: true,
    immediate: true,
  })
  return prefs
}

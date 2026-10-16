import { reactive, watch } from 'vue'
import { loadPrefs, savePrefs, type TopologyPrefs } from '@/utils/topology/prefs'

/** Preferências do visitante, reativas e gravadas a cada mudança. */
export function useTopologyPrefs() {
  const prefs = reactive<TopologyPrefs>(loadPrefs())
  watch(prefs, (value) => savePrefs({ ...value, filters: { ...value.filters } }), { deep: true })
  return prefs
}

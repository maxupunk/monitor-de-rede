<template>
  <div class="tests-tab">
    <div v-if="activeTest" class="px-4 pt-4">
      <div class="section-title">Em andamento agora</div>
      <LinkTestMiniCard :test="activeTest" @open="store.focus(activeTest.id)" />
    </div>

    <TopologyActionList :actions="testActions" dense class="mt-2" />
    <p class="hint text-body-small px-4 mb-4">
      O teste de velocidade mostra o plano antes de rodar e satura o enlace enquanto mede (5 s por
      sentido, por padrão). O diagnóstico por ping é rápido e não satura, mas é só uma estimativa.
    </p>

    <div class="px-4 pb-4">
      <div class="section-title">Testes recentes</div>
      <div v-if="loading && recent.length === 0" class="d-flex align-center ga-2 py-2">
        <v-progress-circular
          indeterminate
          size="18"
          width="2"
          color="primary"
        ></v-progress-circular>
        <span class="text-body-small">Carregando…</span>
      </div>
      <v-alert v-else-if="error" type="error" variant="tonal" density="compact">
        {{ error }}
      </v-alert>
      <LinkTestRecentList
        v-else-if="recent.length > 0"
        :tests="recent"
        :edge-id="edgeId"
        :focused-id="store.focusedId"
        @open="store.focus"
      />
      <div v-else class="empty">
        <v-icon color="info" size="32">mdi-speedometer-slow</v-icon>
        <div class="text-body-medium">
          Nenhum teste {{ target.kind === 'edge' ? 'neste enlace' : 'com este equipamento' }} ainda.
        </div>
      </div>
      <v-btn
        v-if="recent.length > 0"
        :to="historyLink"
        color="primary"
        variant="text"
        size="small"
        append-icon="mdi-arrow-right"
        class="mt-2"
      >
        Ver todo o histórico
      </v-btn>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Aba "Testes" do inspetor (nó e enlace): o teste em andamento com o alvo, as
 * ações de teste — as mesmas do menu de ações (`resolveActions`, grupo
 * "Testes"): testar daqui até…, do servidor até aqui, diagnóstico rápido por
 * ping, repetir o último — e os testes recentes. Clicar num teste abre a tela
 * do teste.
 */
import { computed, ref, watch } from 'vue'
import { useLinkTestsStore } from '@/stores/linkTests'
import { isActiveStatus } from '@/utils/linkTests/presentation'
import {
  resolveActions,
  type TopologyHost,
  type TopologyTarget,
} from '@/utils/topology/inspectorRegistry'
import LinkTestMiniCard from '@/components/linkTests/LinkTestMiniCard.vue'
import LinkTestRecentList from '@/components/linkTests/LinkTestRecentList.vue'
import TopologyActionList from '../TopologyActionList.vue'

const props = defineProps<{ target: TopologyTarget; host: TopologyHost }>()

/** "Ver último teste" é a própria lista abaixo. */
const HIDDEN_ACTIONS = new Set(['test.view-last'])

const store = useLinkTestsStore()
const loading = ref(false)
const error = ref<string | null>(null)

const edgeId = computed(() => (props.target.kind === 'edge' ? props.target.edge.id : null))
const deviceId = computed(() => (props.target.kind === 'node' ? props.target.node.id : null))

const testActions = computed(() =>
  resolveActions(props.target, props.host).filter(
    (action) => action.group === 'test' && !HIDDEN_ACTIONS.has(action.id)
  )
)

const related = computed(() => {
  if (edgeId.value !== null) return store.testsForEdge(edgeId.value)
  if (deviceId.value !== null) return store.testsForDevice(deviceId.value)
  return []
})
const activeTest = computed(() => related.value.find((test) => isActiveStatus(test.status)) ?? null)
const recent = computed(() =>
  related.value.filter((test) => test.id !== activeTest.value?.id).slice(0, 5)
)

const historyLink = computed(() =>
  deviceId.value !== null ? `/link-tests?deviceId=${deviceId.value}` : '/link-tests'
)

/** Abrir a aba é a ação do usuário que busca o histórico; o resto chega pelo SSE. */
async function load() {
  loading.value = true
  error.value = null
  try {
    if (edgeId.value !== null) await store.fetchRecent({ linkId: edgeId.value })
    else if (deviceId.value !== null) await store.fetchRecent({ deviceId: deviceId.value })
  } catch (err: unknown) {
    error.value =
      err instanceof Error && err.message ? err.message : 'Não foi possível carregar os testes.'
  } finally {
    loading.value = false
  }
}

watch([edgeId, deviceId], () => void load(), { immediate: true })
</script>

<style scoped>
.section-title {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 8px;
  color: rgb(var(--v-theme-on-surface));
}
.hint {
  color: rgb(var(--v-theme-on-surface));
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  text-align: center;
  padding: 16px 8px;
  border: 1px dashed rgba(var(--v-theme-on-surface), 0.3);
  border-radius: 10px;
  color: rgb(var(--v-theme-on-surface));
}
</style>

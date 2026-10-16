<template>
  <div class="pa-4">
    <template v-if="activeTest">
      <div class="section-title">Em andamento agora</div>
      <div class="active-card mb-4">
        <LinkTestRunPanel :test="activeTest" compact>
          <template #actions>
            <v-btn
              color="primary"
              variant="flat"
              size="small"
              prepend-icon="mdi-map-marker-path"
              @click="store.focus(activeTest.id)"
            >
              Acompanhar no mapa
            </v-btn>
          </template>
        </LinkTestRunPanel>
      </div>
    </template>

    <v-tooltip :text="READ_ONLY_REASON" :disabled="host.canWrite">
      <template #activator="{ props: tip }">
        <div v-bind="tip">
          <v-btn
            block
            color="primary"
            variant="flat"
            prepend-icon="mdi-speedometer"
            :disabled="!host.canWrite || !testRequest"
            @click="testNow"
          >
            {{ testLabel }}
          </v-btn>
        </div>
      </template>
    </v-tooltip>
    <p class="hint text-body-small mt-2 mb-4">
      Mostra o plano antes de rodar. O enlace fica saturado durante a medição (5 s por sentido, por
      padrão).
    </p>

    <div class="section-title">Testes recentes</div>
    <div v-if="loading && recent.length === 0" class="d-flex align-center ga-2 py-2">
      <v-progress-circular indeterminate size="18" width="2" color="primary"></v-progress-circular>
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
</template>

<script setup lang="ts">
/**
 * Aba "Testes" do inspetor (nó e enlace): o teste em andamento que envolve o
 * alvo, o "Testar agora" e os testes recentes. Clicar num teste o abre no
 * painel flutuante do mapa, com a sobreposição.
 */
import { computed, ref, watch } from 'vue'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTestRequest } from '@/stores/linkTestRequests'
import { isActiveStatus } from '@/utils/linkTests/presentation'
import {
  READ_ONLY_REASON,
  type TopologyHost,
  type TopologyTarget,
} from '@/utils/topology/inspectorRegistry'
import { edgeEndpoints } from '@/utils/topology/presentation'
import LinkTestRecentList from '@/components/linkTests/LinkTestRecentList.vue'
import LinkTestRunPanel from '@/components/linkTests/LinkTestRunPanel.vue'

const props = defineProps<{ target: TopologyTarget; host: TopologyHost }>()

const store = useLinkTestsStore()
const loading = ref(false)
const error = ref<string | null>(null)

const edgeId = computed(() => (props.target.kind === 'edge' ? props.target.edge.id : null))
const deviceId = computed(() => (props.target.kind === 'node' ? props.target.node.id : null))

const related = computed(() => {
  if (edgeId.value !== null) return store.testsForEdge(edgeId.value)
  if (deviceId.value !== null) return store.testsForDevice(deviceId.value)
  return []
})
const activeTest = computed(() => related.value.find((test) => isActiveStatus(test.status)) ?? null)
const recent = computed(() =>
  related.value.filter((test) => test.id !== activeTest.value?.id).slice(0, 5)
)

const testRequest = computed<LinkTestRequest | null>(() => {
  const target = props.target
  if (target.kind === 'edge') {
    const { source, target: to } = edgeEndpoints(target.edge)
    return { kind: 'link', linkId: target.edge.id, sourceDeviceId: source, targetDeviceId: to }
  }
  if (target.kind !== 'node') return null
  const system = props.host.systemNodeId
  if (system !== null && system !== target.node.id) {
    return { kind: 'server', targetDeviceId: target.node.id }
  }
  return { kind: 'path', targetDeviceId: target.node.id }
})

const testLabel = computed(() => {
  if (props.target.kind === 'edge') return 'Testar este enlace agora'
  return testRequest.value?.kind === 'server'
    ? 'Testar do servidor até aqui'
    : 'Testar caminho até aqui'
})

const historyLink = computed(() =>
  deviceId.value !== null ? `/link-tests?deviceId=${deviceId.value}` : '/link-tests'
)

function testNow() {
  if (testRequest.value) props.host.requestLinkTest(testRequest.value)
}

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
.active-card {
  border: 1px solid rgb(var(--v-theme-primary));
  border-radius: 12px;
  overflow: hidden;
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

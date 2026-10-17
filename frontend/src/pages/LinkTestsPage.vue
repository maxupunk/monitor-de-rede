<template>
  <div>
    <PageHeader
      title="Testes de velocidade"
      subtitle="Quanto passa em cada enlace da rede: entre dois equipamentos, ao longo de um caminho ou do servidor até um roteador"
    >
      <template #actions>
        <v-btn color="primary" variant="flat" prepend-icon="mdi-plus" @click="openWizard">
          Novo teste
        </v-btn>
      </template>
    </PageHeader>

    <section v-if="store.active.length > 0" class="mb-4" aria-label="Testes em andamento">
      <div class="section-title mb-2">Em andamento</div>
      <div class="active-grid">
        <button
          v-for="test in store.active"
          :key="test.id"
          type="button"
          class="active-card"
          :aria-label="`Abrir ${testTitle(test)}`"
          @click="openTest(test.id)"
        >
          <span class="font-weight-bold text-truncate w-100">{{ testTitle(test) }}</span>
          <span class="d-flex align-center flex-wrap ga-2">
            <LinkTestStatusChip :status="test.status" size="x-small" class="flex-shrink-0" />
            <span class="text-body-small">{{ activeLegText(test) }}</span>
          </span>
          <span class="live">{{ activeLive(test) }}</span>
        </button>
      </div>
    </section>

    <v-card rounded="lg" border>
      <v-tabs v-model="tab" color="primary">
        <v-tab value="history" prepend-icon="mdi-history">Histórico</v-tab>
        <v-tab value="schedules" prepend-icon="mdi-calendar-clock">Agendamentos</v-tab>
      </v-tabs>
      <v-divider></v-divider>
      <v-window v-model="tab">
        <v-window-item value="history">
          <LinkTestHistoryPanel
            :initial-device-id="queryDeviceId"
            @open="openTest"
            @new="openWizard"
          />
        </v-window-item>
        <v-window-item value="schedules">
          <LinkTestSchedulesPanel v-if="tab === 'schedules'" @started="openTest" />
        </v-window-item>
      </v-window>
    </v-card>

    <v-dialog
      :model-value="detailId !== null"
      :fullscreen="smAndDown"
      max-width="780"
      scrollable
      @update:model-value="(open) => !open && closeTest()"
    >
      <v-card :rounded="smAndDown ? 0 : 'lg'">
        <div v-if="detailLoading && !detail" class="d-flex align-center ga-3 pa-8 justify-center">
          <v-progress-circular indeterminate color="primary"></v-progress-circular>
          <span>Carregando o teste…</span>
        </div>
        <div v-else-if="detailError && !detail" class="pa-4">
          <v-alert type="error" variant="tonal" class="mb-3">{{ detailError }}</v-alert>
          <div class="d-flex justify-end">
            <v-btn color="primary" variant="flat" @click="closeTest">Fechar</v-btn>
          </div>
        </div>
        <v-card-text v-else-if="detail" class="pa-0">
          <LinkTestRunPanel :test="detail" closable @close="closeTest">
            <template #actions>
              <v-btn
                color="secondary"
                variant="flat"
                size="small"
                prepend-icon="mdi-sitemap"
                @click="showOnMap(detail.id)"
              >
                Ver no mapa
              </v-btn>
            </template>
          </LinkTestRunPanel>
        </v-card-text>
      </v-card>
    </v-dialog>

    <LinkTestWizard v-model="wizardOpen" @started="openTest" />
  </div>
</template>

<script setup lang="ts">
/**
 * Testes de velocidade entre equipamentos (ADR 015, "teste de enlace"): histórico filtrável, agendamentos e o "Novo
 * teste". Testes em andamento chegam pelo SSE (snapshot ao conectar); o
 * detalhe abre por `?test=<id>` (link direto).
 */
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useDisplay } from 'vuetify'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTest } from '@/types/linkTests'
import { flowPairLabel, legTitle, testTitle } from '@/utils/linkTests/presentation'
import { currentLeg, legsDone } from '@/utils/linkTests/progress'
import PageHeader from '@/components/PageHeader.vue'
import LinkTestHistoryPanel from '@/components/linkTests/LinkTestHistoryPanel.vue'
import LinkTestRunPanel from '@/components/linkTests/LinkTestRunPanel.vue'
import LinkTestSchedulesPanel from '@/components/linkTests/LinkTestSchedulesPanel.vue'
import LinkTestStatusChip from '@/components/linkTests/LinkTestStatusChip.vue'
import LinkTestWizard from '@/components/linkTests/LinkTestWizard.vue'

const route = useRoute()
const router = useRouter()
const { smAndDown } = useDisplay()
const store = useLinkTestsStore()

const wizardOpen = ref(false)
const detailLoading = ref(false)
const detailError = ref<string | null>(null)

function queryNumber(value: unknown): number | null {
  const n = typeof value === 'string' ? Number(value) : NaN
  return Number.isInteger(n) && n > 0 ? n : null
}

const tab = computed<string>({
  get: () => (route.query.tab === 'schedules' ? 'schedules' : 'history'),
  set: (value) => {
    void router.replace({ query: { ...route.query, tab: value === 'history' ? undefined : value } })
  },
})
const detailId = computed(() => queryNumber(route.query.test))
const queryDeviceId = computed(() => queryNumber(route.query.deviceId))
const detail = computed(() => store.byId(detailId.value))

function openWizard() {
  wizardOpen.value = true
}

function openTest(id: number) {
  void router.push({ query: { ...route.query, test: String(id) } })
}

function closeTest() {
  void router.replace({ query: { ...route.query, test: undefined } })
}

function showOnMap(id: number) {
  store.focus(id)
  void router.push('/topology')
}

function activeLegText(test: LinkTest): string {
  const leg = currentLeg(test)
  if (!leg) return test.status === 'queued' ? 'Na fila' : 'Preparando…'
  return `Perna ${legsDone(test) + 1} de ${test.legs.length} · ${legTitle(leg)}`
}

function activeLive(test: LinkTest): string {
  const leg = currentLeg(test)
  if (!leg) return ''
  return (
    flowPairLabel(
      store.liveBps(test.id, leg.seq, 'forward') ?? leg.forwardBps,
      store.liveBps(test.id, leg.seq, 'reverse') ?? leg.reverseBps
    ) ?? ''
  )
}

/** Abrir o detalhe (clique ou link direto) busca o teste com as amostras gravadas. */
watch(
  detailId,
  async (id) => {
    if (id === null) return
    detailLoading.value = true
    detailError.value = null
    try {
      await store.fetchDetail(id)
    } catch (err: unknown) {
      detailError.value =
        err instanceof Error && err.message ? err.message : 'Teste não encontrado.'
    } finally {
      detailLoading.value = false
    }
  },
  { immediate: true }
)
</script>

<style scoped>
.section-title {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: rgb(var(--v-theme-on-surface));
}
.active-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 12px;
}
.active-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: left;
  padding: 12px;
  border-radius: 12px;
  border: 1px solid rgb(var(--v-theme-primary));
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  cursor: pointer;
}
.active-card:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: 2px;
}
.live {
  font-size: 1.1rem;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
}
</style>

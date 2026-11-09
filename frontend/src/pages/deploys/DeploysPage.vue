<template>
  <div>
    <PageHeader
      title="Deploys"
      subtitle="O GitHub compila e publica as imagens; aqui você decide quando e onde elas entram no ar — com aprovação, janela de manutenção, backup e rollback."
    >
      <template v-if="canAdmin" #actions>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-key-chain"
          @click="credentialsOpen = true"
        >
          Credenciais
        </v-btn>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-rocket-launch"
          @click="openSetup(null)"
        >
          Automatizar deploy
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="deploys.error" type="error" variant="tonal" class="mb-4">
      {{ deploys.error }}
    </v-alert>

    <v-alert
      v-if="deploys.awaitingApproval.length"
      type="warning"
      variant="tonal"
      class="mb-4"
      icon="mdi-account-clock"
      data-test="approval-banner"
    >
      <div class="font-weight-bold mb-1">
        {{ deploys.awaitingApproval.length }} versão(ões) esperando aprovação
      </div>
      <div class="d-flex flex-wrap ga-2">
        <v-btn
          v-for="run in deploys.awaitingApproval.slice(0, 4)"
          :key="run.id"
          color="warning"
          variant="flat"
          size="small"
          prepend-icon="mdi-eye-check"
          @click="openRun(run.id)"
        >
          {{ run.applicationName }} → {{ run.targetName }} ({{ run.shortSha }})
        </v-btn>
      </div>
    </v-alert>

    <v-row v-if="counts" dense class="mb-2">
      <v-col v-for="tile in tiles" :key="tile.label" cols="6" md="3">
        <v-card elevation="2" class="pa-4 h-100">
          <div class="d-flex align-center ga-3">
            <v-avatar :color="tile.color" variant="tonal" rounded="lg">
              <v-icon>{{ tile.icon }}</v-icon>
            </v-avatar>
            <div>
              <div class="text-h5 font-weight-bold">{{ tile.value }}</div>
              <div class="text-body-2 text-high-emphasis">{{ tile.label }}</div>
            </div>
          </div>
        </v-card>
      </v-col>
    </v-row>

    <v-skeleton-loader
      v-if="deploys.loading && !deploys.overview"
      type="card@2"
    ></v-skeleton-loader>

    <v-card
      v-else-if="deploys.overview && !deploys.applications.length"
      elevation="2"
      class="pa-6 mb-4"
      data-test="empty-state"
    >
      <div class="text-h6 font-weight-bold mb-1">Publique sem abrir SSH no servidor</div>
      <div class="text-body-1 text-high-emphasis mb-4">
        O servidor não compila nada: o agente que já está lá baixa as imagens prontas do ghcr.io,
        sobe o compose esperando os healthchecks e volta a versão anterior se algo falhar.
      </div>
      <v-row dense class="mb-4">
        <v-col v-for="(item, index) in ONBOARDING" :key="item.title" cols="12" md="4">
          <v-card border flat class="pa-4 h-100">
            <div class="d-flex align-center ga-2 mb-1">
              <v-avatar color="primary" size="26" class="font-weight-bold">{{
                index + 1
              }}</v-avatar>
              <span class="font-weight-bold">{{ item.title }}</span>
            </div>
            <div class="text-body-2 text-high-emphasis">{{ item.text }}</div>
          </v-card>
        </v-col>
      </v-row>
      <div class="d-flex flex-wrap ga-2">
        <v-btn
          v-if="canAdmin"
          color="primary"
          variant="flat"
          prepend-icon="mdi-rocket-launch"
          @click="openSetup(null)"
        >
          Automatizar o primeiro deploy
        </v-btn>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-docker"
          :to="{ name: 'docker-compose' }"
        >
          Ver os projetos compose dos servidores
        </v-btn>
      </div>
    </v-card>

    <v-row v-else dense>
      <v-col v-for="application in deploys.applications" :key="application.id" cols="12" xl="6">
        <DeployApplicationCard
          :application="application"
          :can-admin="canAdmin"
          @open-run="openRun"
          @publish="(target) => openTargetDialog('publish', target)"
          @env="(target) => openTargetDialog('env', target)"
          @history="(target) => openTargetDialog('history', target)"
          @edit-target="editTarget"
          @remove-target="removeTarget"
          @add-target="addTarget"
          @workflow="openWorkflow"
          @edit="editApplication"
          @remove="removeApplication"
        />
      </v-col>
    </v-row>

    <v-card v-if="deploys.recentRuns.length" elevation="2" class="mt-4">
      <v-card-title class="font-weight-bold d-flex align-center ga-2">
        <v-icon color="primary">mdi-timeline-clock</v-icon> Deploys recentes
      </v-card-title>
      <v-card-text>
        <DeployRunsList :runs="deploys.recentRuns" @open="openRun" />
      </v-card-text>
    </v-card>

    <DeploySetupDialog v-model="setupOpen" :preset="setupPreset" @done="onSetupDone" />
    <DeployCredentialsDialog v-model="credentialsOpen" />
    <DeployApplicationDialog
      v-model="applicationOpen"
      :application="selectedApplication"
      @saved="notify('Aplicação salva.')"
    />
    <DeployTargetDialog
      v-model="targetOpen"
      :application="selectedApplication"
      :target="editingTarget"
      @saved="notify('Alvo salvo.')"
    />
    <DeployWorkflowDialog v-model="workflowOpen" :application="selectedApplication" />
    <DeployEnvDialog v-model="envOpen" :target="selectedTarget" @saved="onEnvSaved" />
    <DeployPublishDialog
      v-model="publishOpen"
      :target="selectedTarget"
      @started="(run) => openRun(run.id)"
    />
    <DeployHistoryDialog v-model="historyOpen" :target="selectedTarget" @open-run="openRun" />
    <DeployRunDialog v-model="runOpen" :run-id="runId" @open-run="openRun" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="5000">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import PageHeader from '@/components/PageHeader.vue'
import DeployApplicationCard from '@/components/deploys/DeployApplicationCard.vue'
import DeployApplicationDialog from '@/components/deploys/DeployApplicationDialog.vue'
import DeployCredentialsDialog from '@/components/deploys/DeployCredentialsDialog.vue'
import DeployEnvDialog from '@/components/deploys/DeployEnvDialog.vue'
import DeployHistoryDialog from '@/components/deploys/DeployHistoryDialog.vue'
import DeployPublishDialog from '@/components/deploys/DeployPublishDialog.vue'
import DeployRunDialog from '@/components/deploys/DeployRunDialog.vue'
import DeployRunsList from '@/components/deploys/DeployRunsList.vue'
import DeploySetupDialog from '@/components/deploys/DeploySetupDialog.vue'
import DeployTargetDialog from '@/components/deploys/DeployTargetDialog.vue'
import DeployWorkflowDialog from '@/components/deploys/DeployWorkflowDialog.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployRunView } from '@/bindings/DeployRunView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import { useAuthStore } from '@/stores/auth'
import { useDeploysStore } from '@/stores/deploys'
import type { SetupPreset } from '@/utils/deployForms'

const deploys = useDeploysStore()
const auth = useAuthStore()
const route = useRoute()
const router = useRouter()
const { confirm } = useConfirm()

const canAdmin = computed(() => auth.roleIn('servers') === 'admin')
const counts = computed(() => deploys.overview?.counts ?? null)

const ONBOARDING = [
  {
    title: 'Libere o deploy no agente',
    text: 'No servidor, acrescente deploy ao AGENT_ALLOW. A raiz das stacks já vem montada nas instalações novas.',
  },
  {
    title: 'Automatize o projeto',
    text: 'Diga o repositório, o servidor e a política (aprovação, janela, backup). A checagem de prontidão aponta o que falta.',
  },
  {
    title: 'Ligue o GitHub',
    text: 'Copie o workflow gerado: ele compila, publica e avisa a central. Nenhum segredo vai para o GitHub.',
  },
]

const tiles = computed(() => {
  const value = counts.value
  if (!value) return []
  return [
    {
      label: `aplicação(ões) em ${value.targets} alvo(s)`,
      value: value.applications,
      icon: 'mdi-github',
      color: 'primary',
    },
    {
      label: 'publicados nas últimas 24 h',
      value: value.succeeded24h,
      icon: 'mdi-check-circle',
      color: 'success',
    },
    {
      label: 'falhas ou rollbacks em 24 h',
      value: value.failed24h,
      icon: 'mdi-alert-circle',
      color: value.failed24h > 0 ? 'error' : 'success',
    },
    {
      label: 'aguardando aprovação',
      value: value.awaitingApproval,
      icon: 'mdi-account-clock',
      color: value.awaitingApproval > 0 ? 'warning' : 'info',
    },
  ]
})

const feedback = reactive({ visible: false, message: '', color: 'success' })
function notify(message: string, color = 'success') {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}

// ── Diálogos ─────────────────────────────────────────────────────────────

const setupOpen = ref(false)
const setupPreset = ref<SetupPreset | null>(null)
const credentialsOpen = ref(false)
const applicationOpen = ref(false)
const targetOpen = ref(false)
const workflowOpen = ref(false)
const envOpen = ref(false)
const publishOpen = ref(false)
const historyOpen = ref(false)
const runOpen = ref(false)
const runId = ref<number | null>(null)
const selectedApplication = ref<DeployApplicationView | null>(null)
const selectedTarget = ref<DeployTargetView | null>(null)
const editingTarget = ref<DeployTargetView | null>(null)

function openSetup(preset: SetupPreset | null) {
  setupPreset.value = preset
  setupOpen.value = true
}

function onSetupDone(applicationId: number) {
  notify('Deploy automático configurado. Falta ligar o GitHub.')
  const application = deploys.applications.find((item) => item.id === applicationId) ?? null
  if (application) openWorkflow(application)
}

function openRun(id: number) {
  runId.value = id
  runOpen.value = true
}

function openTargetDialog(kind: 'publish' | 'env' | 'history', target: DeployTargetView) {
  selectedTarget.value = target
  if (kind === 'publish') publishOpen.value = true
  if (kind === 'env') envOpen.value = true
  if (kind === 'history') historyOpen.value = true
}

function onEnvSaved(message: string, run: DeployRunView | null) {
  notify(message)
  if (run) openRun(run.id)
}

function openWorkflow(application: DeployApplicationView) {
  selectedApplication.value = application
  workflowOpen.value = true
}

function editApplication(application: DeployApplicationView) {
  selectedApplication.value = application
  applicationOpen.value = true
}

function addTarget(application: DeployApplicationView) {
  selectedApplication.value = application
  editingTarget.value = null
  targetOpen.value = true
}

function editTarget(application: DeployApplicationView, target: DeployTargetView) {
  selectedApplication.value = application
  editingTarget.value = target
  targetOpen.value = true
}

async function removeApplication(application: DeployApplicationView) {
  const ok = await confirm({
    title: `Remover a aplicação ${application.name}?`,
    message:
      'O deploy automático, os alvos, as versões do .env e o histórico saem da central. Os containers continuam no ar.',
    confirmText: 'Remover',
    confirmColor: 'error',
    icon: 'mdi-delete',
  })
  if (!ok) return
  try {
    await deploys.removeApplication(application.id)
    notify('Aplicação removida.')
  } catch (err) {
    notify(err instanceof Error ? err.message : 'Não foi possível remover', 'error')
  }
}

async function removeTarget(target: DeployTargetView) {
  const ok = await confirm({
    title: `Remover o alvo ${target.name}?`,
    message:
      'Este servidor deixa de receber deploys desta aplicação. Os containers e o diretório da stack continuam lá.',
    confirmText: 'Remover',
    confirmColor: 'error',
    icon: 'mdi-delete',
  })
  if (!ok) return
  try {
    await deploys.removeTarget(target.id)
    notify('Alvo removido.')
  } catch (err) {
    notify(err instanceof Error ? err.message : 'Não foi possível remover', 'error')
  }
}

// ── Links diretos: a notificação (`?run=`) e a aba Compose (`?setup=`) ─────

function applyQuery() {
  const run = Number(route.query.run)
  if (Number.isInteger(run) && run > 0) openRun(run)
  if (route.query.setup && canAdmin.value) {
    const probe = Number(route.query.probe)
    openSetup({
      probeId: Number.isInteger(probe) && probe > 0 ? probe : undefined,
      project: typeof route.query.project === 'string' ? route.query.project : undefined,
    })
  }
  if (route.query.run || route.query.setup) {
    void router.replace({ name: 'deploys', query: {} })
  }
}

watch(() => route.query, applyQuery)

onMounted(async () => {
  await deploys.fetchOverview()
  applyQuery()
})
</script>

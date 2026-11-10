<template>
  <div>
    <PageHeader
      title="Deploys"
      subtitle="Dê push e o servidor compila, guarda a imagem no registry e sobe — com aprovação, janela de manutenção, backup e rollback."
    >
      <template v-if="canAdmin" #actions>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-github"
          @click="openConnections(false)"
        >
          Conexões
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
      v-if="githubProgress"
      type="info"
      variant="tonal"
      class="mb-4"
      icon="mdi-github"
      data-test="github-progress"
    >
      <div class="d-flex align-center ga-3">
        <v-progress-circular indeterminate size="20" width="2" color="info"></v-progress-circular>
        {{ githubProgress }}
      </div>
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
        {{ countNoun(deploys.awaitingApproval.length, 'versão', 'versões') }} esperando aprovação
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
          :latest-build="builds.latestOf(application.id)"
          @build="openBuildStart"
          @open-build="openBuild"
          @secrets="openSecrets"
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

    <v-card v-if="builds.recent.length" elevation="2" class="mt-4" data-test="recent-builds">
      <v-card-title class="font-weight-bold d-flex align-center ga-2">
        <v-icon color="primary">mdi-hammer-wrench</v-icon> Compilações recentes
      </v-card-title>
      <v-card-text>
        <v-list density="compact" class="bg-transparent pa-0">
          <v-list-item
            v-for="build in builds.recent.slice(0, 10)"
            :key="build.id"
            :title="`${build.application} · ${build.shortSha}`"
            :subtitle="buildSubtitle(build)"
            class="px-0"
            @click="openBuild(build.id)"
          >
            <template #prepend>
              <StatusMetaChip :meta="BUILD_STATUS[build.status]" class="mr-3" />
            </template>
            <template #append>
              <v-icon color="primary">mdi-chevron-right</v-icon>
            </template>
          </v-list-item>
        </v-list>
      </v-card-text>
    </v-card>

    <v-card v-if="deploys.recentRuns.length" elevation="2" class="mt-4">
      <v-card-title class="font-weight-bold d-flex align-center ga-2">
        <v-icon color="primary">mdi-timeline-clock</v-icon> Deploys recentes
      </v-card-title>
      <v-card-text>
        <DeployRunsList :runs="deploys.recentRuns" @open="openRun" />
      </v-card-text>
    </v-card>

    <DeploySetupDialog v-model="setupOpen" :preset="setupPreset" @done="onSetupDone" />
    <DeployConnectionsDialog v-model="connectionsOpen" :start-connecting="connectNow" />
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
    <DeployBuildStartDialog
      v-model="buildStartOpen"
      :application="selectedApplication"
      @started="openBuild"
    />
    <DeployBuildDialog
      v-model="buildOpen"
      :build-id="buildId"
      :can-admin="canAdmin"
      @opened="openBuild"
    />
    <DeployBuildSecretsDialog v-model="secretsOpen" :application="selectedApplication" />

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
import DeployConnectionsDialog from '@/components/deploys/DeployConnectionsDialog.vue'
import DeployEnvDialog from '@/components/deploys/DeployEnvDialog.vue'
import DeployHistoryDialog from '@/components/deploys/DeployHistoryDialog.vue'
import DeployPublishDialog from '@/components/deploys/DeployPublishDialog.vue'
import DeployRunDialog from '@/components/deploys/DeployRunDialog.vue'
import DeployBuildDialog from '@/components/deploys/DeployBuildDialog.vue'
import DeployBuildSecretsDialog from '@/components/deploys/DeployBuildSecretsDialog.vue'
import DeployBuildStartDialog from '@/components/deploys/DeployBuildStartDialog.vue'
import StatusMetaChip from '@/components/deploys/StatusMetaChip.vue'
import type { DeployServerBuildView } from '@/bindings/DeployServerBuildView'
import { useDeployBuildsStore } from '@/stores/deployBuilds'
import { BUILD_STATUS, refName } from '@/utils/deployPresentation'
import { countNoun, formatRelativeTime } from '@/utils/formatters'
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
import {
  GITHUB_CALLBACK_KEYS,
  parseGithubCallback,
  takeGithubReturn,
  type GithubCallback,
} from '@/utils/githubConnect'

const deploys = useDeploysStore()
const auth = useAuthStore()
const route = useRoute()
const router = useRouter()
const { confirm } = useConfirm()

const canAdmin = computed(() => auth.roleIn('servers') === 'admin')
const builds = useDeployBuildsStore()
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
    title: 'Dê push',
    text: 'Com o GitHub conectado, cada push na branch do alvo chega à central: o servidor compila, guarda a imagem no registry, confere as vulnerabilidades e sobe. Sem workflow no repositório.',
  },
]

const tiles = computed(() => {
  const value = counts.value
  if (!value) return []
  return [
    {
      label: `${value.applications === 1 ? 'aplicação' : 'aplicações'} em ${countNoun(value.targets, 'alvo')}`,
      value: value.applications,
      icon: 'mdi-github',
      color: 'primary',
    },
    {
      label: 'publicados nas últimas 24\u00a0h',
      value: value.succeeded24h,
      icon: 'mdi-check-circle',
      color: 'success',
    },
    {
      label: 'falhas ou rollbacks em 24\u00a0h',
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
const connectionsOpen = ref(false)
const connectNow = ref(false)
const githubProgress = ref<string | null>(null)
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

function openConnections(connect: boolean) {
  connectNow.value = connect
  connectionsOpen.value = true
}

function openSetup(preset: SetupPreset | null) {
  setupPreset.value = preset
  setupOpen.value = true
}

function onSetupDone(applicationId: number) {
  const application = deploys.applications.find((item) => item.id === applicationId) ?? null
  if (application?.builder === 'netmonitor') {
    // O servidor compila (ADR 023): nada a ligar no GitHub — o próximo push já
    // compila. "Compilar agora" mostra o caminho inteiro sem esperar um push.
    notify('Pronto: o próximo push compila no servidor e sobe. Quer ver agora?')
    openBuildStart(application)
    return
  }
  notify('Deploy automático configurado. Falta ligar o GitHub.')
  if (application) openWorkflow(application)
}

const buildStartOpen = ref(false)
const buildOpen = ref(false)
const buildId = ref<number | null>(null)
const secretsOpen = ref(false)

function openBuildStart(application: DeployApplicationView) {
  selectedApplication.value = application
  buildStartOpen.value = true
}

function openBuild(id: number) {
  buildId.value = id
  buildOpen.value = true
}

function openSecrets(application: DeployApplicationView) {
  selectedApplication.value = application
  secretsOpen.value = true
}

function buildSubtitle(build: DeployServerBuildView): string {
  const reference = refName(build.reference)
  return [reference, build.agent && `em ${build.agent}`, formatRelativeTime(build.createdAt)]
    .filter(Boolean)
    .join(' · ')
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

// ── A volta do GitHub (criar e instalar o app) ─────────────────────────────

/** Retoma onde a pessoa estava antes de ir ao GitHub. */
function resume() {
  const target = takeGithubReturn()
  if (target === 'setup') openSetup(null)
  if (target === 'connections') openConnections(false)
}

async function finishGithub(callback: GithubCallback) {
  try {
    if (callback.kind === 'created') {
      githubProgress.value = 'App criado no GitHub. Abrindo a instalação…'
      const connection = await deploys.connectGithub(callback.code, callback.state)
      // Sem instalar o app não enxerga nada: segue direto para lá (a volta
      // cai aqui de novo, com o `installation_id`).
      window.location.assign(connection.installUrl)
      return
    }
    githubProgress.value = 'Conferindo a instalação no GitHub…'
    const connection = await deploys.claimGithubInstallation(callback.installationId)
    githubProgress.value = null
    notify(`GitHub conectado: ${connection.account}.`)
    resume()
  } catch (err) {
    githubProgress.value = null
    notify(err instanceof Error ? err.message : 'Não foi possível concluir a conexão', 'error')
    resume()
  }
}

// ── Links diretos: as notificações (`?run=`, `?build=`) e a aba Compose (`?setup=`) ─

function applyQuery() {
  const run = Number(route.query.run)
  if (Number.isInteger(run) && run > 0) openRun(run)
  const build = Number(route.query.build)
  if (Number.isInteger(build) && build > 0) openBuild(build)
  if (route.query.setup && canAdmin.value) {
    const probe = Number(route.query.probe)
    openSetup({
      probeId: Number.isInteger(probe) && probe > 0 ? probe : undefined,
      project: typeof route.query.project === 'string' ? route.query.project : undefined,
    })
  }
  const callback = parseGithubCallback(route.query)
  if (callback && canAdmin.value) void finishGithub(callback)
  const handled =
    route.query.run ||
    route.query.build ||
    route.query.setup ||
    GITHUB_CALLBACK_KEYS.some((key) => key in route.query)
  if (handled) {
    void router.replace({ name: 'deploys', query: {} })
  }
}

watch(() => route.query, applyQuery)

onMounted(async () => {
  await Promise.all([
    deploys.fetchOverview(),
    builds
      .fetchRecent()
      .catch((err) =>
        notify(
          err instanceof Error ? err.message : 'Não foi possível ler as compilações recentes',
          'error'
        )
      ),
  ])
  applyQuery()
})
</script>

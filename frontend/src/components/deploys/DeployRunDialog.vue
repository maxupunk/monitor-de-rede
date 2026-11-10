<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 960"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="run" class="rounded-lg">
      <v-card-item class="pt-4 px-6">
        <template #prepend>
          <v-avatar :color="statusMeta.color" variant="tonal" rounded="lg">
            <v-icon>{{ statusMeta.icon }}</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-wrap">
          Deploy #{{ run.id }} — {{ run.applicationName }} em {{ run.targetName }}
        </v-card-title>
        <v-card-subtitle class="text-wrap text-high-emphasis">
          {{ TRIGGER_LABEL[run.trigger] }}{{ run.triggeredBy ? ` por ${run.triggeredBy}` : '' }} ·
          {{ formatRelativeTime(run.createdAt) }}
          <template v-if="run.durationMs !== null">
            · durou {{ formatElapsedMs(run.durationMs) }}
          </template>
        </v-card-subtitle>
        <template #append>
          <DeployRunStatusChip :status="run.status" size="default" />
        </template>
      </v-card-item>

      <v-card-text class="px-6">
        <v-card border flat class="pa-3 mb-4">
          <div class="d-flex flex-wrap align-center ga-2">
            <v-chip
              color="primary"
              variant="tonal"
              label
              size="small"
              prepend-icon="mdi-source-commit"
              :href="commitUrl(run.repository, run.commitSha)"
              target="_blank"
              rel="noopener"
            >
              {{ run.shortSha }}
            </v-chip>
            <span class="text-body-1 font-weight-medium text-break">
              {{ commitTitle(run.commitMessage) || 'Sem mensagem de commit' }}
            </span>
          </div>
          <div class="text-body-2 text-high-emphasis mt-1">
            Imagem <code>:{{ run.imageTag }}</code>
            <template v-if="run.reference"> · de {{ run.reference }}</template>
            <template v-if="run.approvedBy"> · aprovado por {{ run.approvedBy }}</template>
            <template v-if="run.status === 'scheduled' && run.scheduledFor">
              · roda {{ formatTimeUntil(run.scheduledFor) }} ({{
                formatShortDateTime(run.scheduledFor)
              }})
            </template>
          </div>
        </v-card>

        <!-- Aprovação: o que entra no ar -->
        <template v-if="run.status === 'awaiting_approval'">
          <div class="text-subtitle-1 font-weight-bold mb-2">O que muda</div>
          <v-skeleton-loader
            v-if="changesLoading"
            type="list-item-two-line@3"
            class="mb-4"
          ></v-skeleton-loader>
          <template v-else-if="changes">
            <v-alert
              v-if="changes.unavailableReason"
              type="info"
              variant="tonal"
              density="compact"
              class="mb-2"
            >
              {{ changes.unavailableReason }}
            </v-alert>
            <v-list v-if="changes.commits.length" density="compact" class="bg-transparent pa-0">
              <v-list-item
                v-for="commit in changes.commits"
                :key="commit.sha"
                :title="commit.message"
                :subtitle="commitSubtitle(commit)"
                :href="commit.url"
                target="_blank"
                rel="noopener"
                class="px-0"
              >
                <template #prepend>
                  <v-chip color="primary" variant="tonal" label size="x-small" class="mr-3">
                    {{ commit.shortSha }}
                  </v-chip>
                </template>
              </v-list-item>
            </v-list>
            <div
              v-if="changes.total > changes.commits.length"
              class="text-body-2 text-high-emphasis"
            >
              e mais {{ changes.total - changes.commits.length }} commit(s)
            </div>
            <v-btn
              v-if="changes.compareUrl"
              :href="changes.compareUrl"
              target="_blank"
              rel="noopener"
              variant="text"
              color="primary"
              size="small"
              prepend-icon="mdi-github"
              class="px-0 mt-1 mb-4"
            >
              Ver a comparação no GitHub
            </v-btn>
          </template>
        </template>

        <!-- Etapas -->
        <div class="text-subtitle-1 font-weight-bold mb-2">Etapas</div>
        <div class="d-flex flex-wrap ga-2 mb-4">
          <v-chip
            v-for="stage in visibleStages"
            :key="stage.value"
            :color="STAGE_COLOR[stages[stage.value]]"
            :variant="stages[stage.value] === 'todo' ? 'outlined' : 'tonal'"
            label
            size="small"
            :prepend-icon="stageIcon(stage.value, stage.icon)"
          >
            {{ stage.label }}
          </v-chip>
        </div>

        <v-alert
          v-if="run.error"
          :type="run.status === 'rolled_back' ? 'warning' : 'error'"
          variant="tonal"
          class="mb-4"
          data-test="run-error"
        >
          <div class="text-body-1">{{ run.error }}</div>
          <template v-if="backups.length && canAdmin">
            <div class="font-weight-bold mt-3 mb-1">Cópias do banco feitas antes deste deploy</div>
            <div
              v-for="copy in backups"
              :key="copy.id"
              class="d-flex align-center flex-wrap ga-2 py-1"
            >
              <v-icon size="18">mdi-database</v-icon>
              <span class="font-weight-medium">{{ copy.databaseName }}</span>
              <span class="text-body-2">{{ formatShortDateTime(copy.startedAt) }}</span>
              <v-spacer></v-spacer>
              <v-btn
                size="small"
                color="warning"
                variant="flat"
                prepend-icon="mdi-database-refresh"
                :loading="openingRestore === copy.id"
                @click="openRestore(copy)"
              >
                Restaurar este banco
              </v-btn>
            </div>
          </template>
        </v-alert>

        <!-- Console -->
        <DeployConsole
          :lines="lines"
          :live="isLive"
          :error="consoleError"
          :empty-text="
            isPendingRun ? 'O console começa quando o deploy rodar.' : 'Nenhuma linha registrada.'
          "
        />
      </v-card-text>

      <v-card-actions class="px-6 pb-4 flex-wrap ga-2">
        <template v-if="canAdmin">
          <template v-if="run.status === 'awaiting_approval'">
            <v-btn
              color="error"
              variant="tonal"
              prepend-icon="mdi-close"
              :loading="acting === 'reject'"
              @click="reject"
            >
              Recusar
            </v-btn>
            <v-spacer></v-spacer>
            <v-btn
              v-if="windowText"
              color="info"
              variant="tonal"
              prepend-icon="mdi-calendar-clock"
              :loading="acting === 'window'"
              @click="approve('window')"
            >
              Publicar na janela ({{ windowText }})
            </v-btn>
            <v-btn
              color="success"
              variant="flat"
              prepend-icon="mdi-rocket-launch"
              :loading="acting === 'now'"
              @click="approve('now')"
            >
              Publicar agora
            </v-btn>
          </template>
          <template v-else-if="isPendingRun">
            <v-btn
              color="error"
              variant="tonal"
              prepend-icon="mdi-cancel"
              :loading="acting === 'cancel'"
              @click="cancel"
            >
              Cancelar deploy
            </v-btn>
            <v-spacer></v-spacer>
          </template>
          <v-btn
            v-else-if="run.status === 'succeeded' && !isLiveVersion"
            color="warning"
            variant="tonal"
            prepend-icon="mdi-backup-restore"
            :loading="acting === 'restore'"
            @click="restore"
          >
            Restaurar esta versão
          </v-btn>
        </template>
        <v-spacer v-if="!(canAdmin && run.status === 'awaiting_approval')"></v-spacer>
        <v-btn variant="text" @click="emit('update:modelValue', false)">Fechar</v-btn>
      </v-card-actions>
    </v-card>

    <DatabaseRestoreDialog v-model="restoreOpen" :backup="restoreBackup" :source="restoreSource" />
  </v-dialog>
</template>

<script setup lang="ts">
import DeployConsole from '@/components/deploys/DeployConsole.vue'
import { computed, ref, watch } from 'vue'
import DatabaseRestoreDialog from '@/components/databases/DatabaseRestoreDialog.vue'
import DeployRunStatusChip from '@/components/deploys/DeployRunStatusChip.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { DatabaseBackupResponse } from '@/bindings/DatabaseBackupResponse'
import type { DatabaseConnectionResponse } from '@/bindings/DatabaseConnectionResponse'
import type { DeployBackupView } from '@/bindings/DeployBackupView'
import type { DeployChangesView } from '@/bindings/DeployChangesView'
import type { DeployCommitView } from '@/bindings/DeployCommitView'
import type { DeployStage } from '@/bindings/DeployStage'
import type { DeployWhen } from '@/bindings/DeployWhen'
import { useAuthStore } from '@/stores/auth'
import { useDatabasesStore } from '@/stores/databases'
import { useDeploysStore } from '@/stores/deploys'
import {
  DEPLOY_STAGES,
  TRIGGER_LABEL,
  commitTitle,
  commitUrl,
  isPending,
  runStatusMeta,
  stageStates,
  windowLabel,
  type StageState,
} from '@/utils/deployPresentation'
import {
  formatElapsedMs,
  formatRelativeTime,
  formatShortDateTime,
  formatTimeUntil,
} from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean; runId: number | null }>()
const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  /** Abrir outro deploy no lugar deste (a restauração recém-pedida). */
  'open-run': [id: number]
}>()

const deploys = useDeploysStore()
const databases = useDatabasesStore()
const auth = useAuthStore()
const { confirm, prompt } = useConfirm()

const STAGE_COLOR: Record<StageState, string> = {
  done: 'success',
  current: 'primary',
  todo: 'secondary',
  failed: 'error',
}

const run = computed(() => (props.runId === null ? null : deploys.runById(props.runId)))
const target = computed(() =>
  run.value ? (deploys.targets.find((item) => item.id === run.value?.targetId) ?? null) : null
)
const canAdmin = computed(() => auth.roleIn('servers') === 'admin')
const statusMeta = computed(() => runStatusMeta(run.value?.status ?? 'queued'))
const isPendingRun = computed(() => (run.value ? isPending(run.value.status) : false))
const isLive = computed(() => run.value?.status === 'running' || run.value?.status === 'queued')
const isLiveVersion = computed(() => target.value?.live?.id === run.value?.id)
const windowText = computed(() =>
  target.value ? windowLabel(target.value.windowStart, target.value.windowEnd) : ''
)
const stages = computed(() =>
  run.value ? stageStates(run.value.stage, run.value.status) : stageStates(null, 'queued')
)
/** O rollback só aparece quando aconteceu; o backup, quando o alvo o tem. */
const visibleStages = computed(() =>
  DEPLOY_STAGES.filter((stage) => {
    if (stage.value === 'rollback') return stages.value.rollback !== 'todo'
    if (stage.value === 'backup') return Boolean(target.value?.backupConnectionId)
    return true
  })
)

function stageIcon(stage: DeployStage, icon: string): string {
  const state = stages.value[stage]
  if (state === 'done') return 'mdi-check'
  if (state === 'failed') return 'mdi-close'
  return icon
}

const lines = computed(() => (props.runId === null ? [] : (deploys.consoles[props.runId] ?? [])))
const consoleError = ref<string | null>(null)

// ── O que muda (aprovação) e as cópias do banco ─────────────────────────

const changes = ref<DeployChangesView | null>(null)
const changesLoading = ref(false)
const backups = ref<DeployBackupView[]>([])

function commitSubtitle(commit: DeployCommitView): string {
  return [commit.author, commit.date ? formatShortDateTime(commit.date) : '']
    .filter(Boolean)
    .join(' · ')
}

async function loadChanges(runId: number) {
  changesLoading.value = true
  try {
    changes.value = await deploys.changes(runId)
  } catch {
    changes.value = null
  } finally {
    changesLoading.value = false
  }
}

async function loadBackups(runId: number) {
  try {
    backups.value = await deploys.backups(runId)
  } catch {
    backups.value = []
  }
}

async function open(runId: number) {
  consoleError.value = null
  changes.value = null
  backups.value = []
  try {
    await deploys.openConsole(runId)
  } catch (err) {
    consoleError.value = err instanceof Error ? err.message : 'Não foi possível ler o log'
  }
  if (run.value?.status === 'awaiting_approval') void loadChanges(runId)
  if (run.value?.backupId) void loadBackups(runId)
}

watch(
  () => [props.modelValue, props.runId] as const,
  ([visible, runId], previous) => {
    const previousId = previous?.[1]
    if (previousId !== undefined && previousId !== null && previousId !== runId) {
      deploys.closeConsole(previousId)
    }
    if (visible && runId !== null) void open(runId)
    if (!visible && runId !== null) deploys.closeConsole(runId)
  },
  { immediate: true }
)

/** Terminou com cópia de banco: a lista aparece sem reabrir. */
watch(
  () => run.value?.status,
  (status) => {
    if (
      props.runId !== null &&
      run.value?.backupId &&
      (status === 'rolled_back' || status === 'failed')
    ) {
      void loadBackups(props.runId)
    }
  }
)

// ── Ações ────────────────────────────────────────────────────────────────

const acting = ref<string | null>(null)

async function perform(key: string, action: () => Promise<unknown>) {
  acting.value = key
  try {
    await action()
  } catch (err) {
    consoleError.value = err instanceof Error ? err.message : 'A ação não pôde ser feita'
  } finally {
    acting.value = null
  }
}

async function approve(when: DeployWhen) {
  const current = run.value
  if (!current) return
  await perform(when, () => deploys.approve(current.id, when))
}

async function reject() {
  const current = run.value
  if (!current) return
  const reason = await prompt({
    title: 'Recusar este deploy?',
    message: 'A versão não vai para o ar. O motivo fica no histórico (opcional).',
    inputLabel: 'Motivo',
    confirmText: 'Recusar',
    confirmColor: 'error',
    icon: 'mdi-close-circle',
  })
  if (reason === null) return
  await perform('reject', () => deploys.reject(current.id, reason || undefined))
}

async function cancel() {
  const current = run.value
  if (!current) return
  const ok = await confirm({
    title: 'Cancelar este deploy?',
    message: 'Ele ainda não começou: nada muda no servidor.',
    confirmText: 'Cancelar deploy',
    confirmColor: 'error',
    icon: 'mdi-cancel',
  })
  if (ok) await perform('cancel', () => deploys.cancel(current.id))
}

async function restore() {
  const current = run.value
  if (!current) return
  const ok = await confirm({
    title: `Restaurar a versão ${current.shortSha}?`,
    message: `${current.targetName} volta para esta versão — a imagem e o .env que ela usou.`,
    details: [
      'Se versões posteriores rodaram migrations, o banco pode estar à frente desta versão.',
      'O progresso aparece no console.',
    ],
    confirmText: 'Restaurar agora',
    confirmColor: 'warning',
    icon: 'mdi-backup-restore',
  })
  if (!ok) return
  await perform('restore', async () => {
    const created = await deploys.restore(current.id, 'now')
    emit('open-run', created.id)
  })
}

// ── Restaurar uma cópia do banco (o diálogo dos Bancos de dados) ────────

const restoreOpen = ref(false)
const restoreBackup = ref<DatabaseBackupResponse | null>(null)
const restoreSource = ref<DatabaseConnectionResponse | null>(null)
const openingRestore = ref<number | null>(null)

async function openRestore(copy: DeployBackupView) {
  openingRestore.value = copy.id
  try {
    if (!databases.connections.length) await databases.fetchConnections()
    const history = await databases.history(copy.connectionId)
    restoreBackup.value = history.find((item) => item.id === copy.id) ?? null
    restoreSource.value =
      databases.connections.find((item) => item.id === copy.connectionId) ?? null
    restoreOpen.value = Boolean(restoreBackup.value)
  } catch (err) {
    consoleError.value = err instanceof Error ? err.message : 'Não foi possível abrir a cópia'
  } finally {
    openingRestore.value = null
  }
}
</script>

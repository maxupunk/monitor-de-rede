<template>
  <div>
    <PageHeader
      title="Tarefas agendadas"
      subtitle="Consultas, testes e ações que rodam sozinhos no horário que você escolher — pela tela ou pedindo ao Assistente IA"
    >
      <template #actions>
        <v-tooltip :text="READ_ONLY_REASON" :disabled="auth.canWrite">
          <template #activator="{ props: tip }">
            <span v-bind="tip">
              <v-btn
                color="primary"
                variant="flat"
                prepend-icon="mdi-calendar-plus"
                :disabled="!auth.canWrite"
                @click="openDialog(null)"
              >
                Nova tarefa
              </v-btn>
            </span>
          </template>
        </v-tooltip>
      </template>
    </PageHeader>

    <v-alert v-if="store.error" type="error" variant="tonal" density="compact" class="mb-3">
      {{ store.error }}
    </v-alert>

    <v-card
      v-if="store.loaded && store.tasks.length === 0"
      border
      flat
      rounded="lg"
      class="empty pa-8"
    >
      <v-icon size="48" color="primary">mdi-calendar-clock</v-icon>
      <div class="text-title-medium font-weight-bold">Nenhuma tarefa agendada</div>
      <p class="text-body-medium mb-2">
        Qualquer consulta, teste ou ação que o Assistente IA sabe fazer pode rodar sozinha num
        horário — e o histórico de cada execução fica guardado. Comece por um modelo:
      </p>
      <div v-if="auth.canWrite" class="d-flex flex-wrap justify-center ga-2">
        <v-btn
          v-for="template in templates"
          :key="template.tool"
          color="primary"
          variant="tonal"
          :prepend-icon="template.icon"
          @click="openDialog(null, template)"
        >
          {{ template.title }}
        </v-btn>
      </div>
      <p class="text-body-small mb-0 mt-2">
        Ou peça no chat: <em>"agende um ping no gateway a cada 15 minutos"</em>.
      </p>
    </v-card>

    <v-card v-else border flat rounded="lg">
      <ResponsiveDataTable
        :headers="headers"
        :items="store.tasks"
        :loading="store.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhuma tarefa agendada"
        :item-key="(item: ScheduledTask) => item.id"
      >
        <template #item.name="{ item }">
          <div class="font-weight-bold">{{ item.name }}</div>
          <div class="d-flex align-center flex-wrap ga-1 mt-1">
            <v-chip :color="toolLook(item).color" size="x-small" variant="tonal" label>
              <v-icon start size="14">{{ toolLook(item).icon }}</v-icon>
              {{ toolLook(item).label }}
            </v-chip>
            <v-chip
              v-if="item.origin === 'ai'"
              color="secondary"
              size="x-small"
              variant="tonal"
              label
            >
              <v-icon start size="14">mdi-robot-outline</v-icon>
              via IA
            </v-chip>
          </div>
          <div v-if="item.description" class="text-body-small mt-1">{{ item.description }}</div>
        </template>
        <template #item.schedule="{ item }">
          <div>{{ item.scheduleLabel }}</div>
          <code class="text-body-small">{{ item.cron }}</code>
        </template>
        <template #item.last="{ item }">
          <div class="d-flex align-center ga-2">
            <ScheduledTaskStatusChip
              v-if="item.lastStatus"
              :status="item.lastStatus"
              size="x-small"
            />
            <span class="text-body-small">{{ formatRelativeTime(item.lastRunAt) }}</span>
          </div>
          <v-tooltip
            v-if="item.lastMessage"
            :text="item.lastMessage"
            max-width="420"
            location="bottom"
          >
            <template #activator="{ props: tip }">
              <div v-bind="tip" class="text-body-small last-message text-truncate">
                {{ item.lastMessage }}
              </div>
            </template>
          </v-tooltip>
        </template>
        <template #item.next="{ item }">
          <span class="text-body-small">{{ nextLabel(item) }}</span>
        </template>
        <template #item.enabled="{ item }">
          <v-switch
            :model-value="item.enabled"
            color="success"
            inset
            hide-details
            density="compact"
            :disabled="!auth.canWrite || busyId === item.id"
            :aria-label="item.enabled ? `Pausar ${item.name}` : `Retomar ${item.name}`"
            @update:model-value="(value) => toggle(item, Boolean(value))"
          ></v-switch>
        </template>
        <template #item.actions="{ item }">
          <div class="d-flex ga-1 justify-end">
            <v-btn
              color="primary"
              variant="flat"
              size="small"
              prepend-icon="mdi-play"
              :disabled="!auth.canWrite || item.running"
              :loading="busyId === item.id || item.running"
              @click="runNow(item)"
            >
              Rodar agora
            </v-btn>
            <v-btn
              icon="mdi-history"
              color="primary"
              variant="text"
              size="small"
              :aria-label="`Histórico de ${item.name}`"
              @click="openRuns(item)"
            ></v-btn>
            <v-btn
              icon="mdi-pencil"
              color="primary"
              variant="text"
              size="small"
              :disabled="!auth.canWrite"
              :aria-label="`Editar ${item.name}`"
              @click="openDialog(item)"
            ></v-btn>
            <v-btn
              icon="mdi-delete"
              color="error"
              variant="text"
              size="small"
              :disabled="!auth.canWrite"
              :aria-label="`Excluir ${item.name}`"
              @click="remove(item)"
            ></v-btn>
          </div>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex flex-column ga-2">
            <div class="d-flex align-center justify-space-between ga-2">
              <span class="font-weight-bold text-truncate">{{ item.name }}</span>
              <v-switch
                :model-value="item.enabled"
                color="success"
                inset
                hide-details
                density="compact"
                class="flex-grow-0"
                :disabled="!auth.canWrite || busyId === item.id"
                :aria-label="item.enabled ? `Pausar ${item.name}` : `Retomar ${item.name}`"
                @update:model-value="(value) => toggle(item, Boolean(value))"
              ></v-switch>
            </div>
            <div class="d-flex align-center flex-wrap ga-1">
              <v-chip :color="toolLook(item).color" size="x-small" variant="tonal" label>
                <v-icon start size="14">{{ toolLook(item).icon }}</v-icon>
                {{ toolLook(item).label }}
              </v-chip>
              <v-chip
                v-if="item.origin === 'ai'"
                color="secondary"
                size="x-small"
                variant="tonal"
                label
              >
                via IA
              </v-chip>
            </div>
            <div class="text-body-small">{{ item.scheduleLabel }} · {{ nextLabel(item) }}</div>
            <div class="d-flex align-center ga-2">
              <ScheduledTaskStatusChip
                v-if="item.lastStatus"
                :status="item.lastStatus"
                size="x-small"
              />
              <span class="text-body-small">último {{ formatRelativeTime(item.lastRunAt) }}</span>
            </div>
            <div v-if="item.lastMessage" class="text-body-small last-message">
              {{ item.lastMessage }}
            </div>
            <div class="d-flex align-center justify-end ga-1 pt-2 border-t">
              <v-btn
                color="primary"
                variant="flat"
                size="small"
                prepend-icon="mdi-play"
                :disabled="!auth.canWrite || item.running"
                :loading="busyId === item.id || item.running"
                @click="runNow(item)"
              >
                Rodar agora
              </v-btn>
              <v-btn
                icon="mdi-history"
                color="primary"
                variant="text"
                size="small"
                :aria-label="`Histórico de ${item.name}`"
                @click="openRuns(item)"
              ></v-btn>
              <v-btn
                icon="mdi-pencil"
                color="primary"
                variant="text"
                size="small"
                :disabled="!auth.canWrite"
                :aria-label="`Editar ${item.name}`"
                @click="openDialog(item)"
              ></v-btn>
              <v-btn
                icon="mdi-delete"
                color="error"
                variant="text"
                size="small"
                :disabled="!auth.canWrite"
                :aria-label="`Excluir ${item.name}`"
                @click="remove(item)"
              ></v-btn>
            </div>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <ScheduledTaskDialog
      v-model="dialog.open"
      :task="dialog.task"
      :template="dialog.template"
      @saved="onSaved"
    />
    <ScheduledTaskRunsDialog v-model="runsDialog.open" :task="runsDialog.task" />
    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="4000">
      {{ feedback.text }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
/** Tarefas agendadas: lista, criar/editar, pausar, rodar agora, histórico e excluir. */
import { computed, onMounted, reactive, ref } from 'vue'
import type { ScheduledTask } from '@/bindings/ScheduledTask'
import { aiToolMeta } from '@/components/ai/aiToolMeta'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import ScheduledTaskDialog from '@/components/scheduledTasks/ScheduledTaskDialog.vue'
import ScheduledTaskRunsDialog from '@/components/scheduledTasks/ScheduledTaskRunsDialog.vue'
import ScheduledTaskStatusChip from '@/components/scheduledTasks/ScheduledTaskStatusChip.vue'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import { useScheduledTasksStore } from '@/stores/scheduledTasks'
import { formatRelativeTime, formatTimeUntil } from '@/utils/formatters'
import {
  TASK_TEMPLATES,
  TOOL_KIND,
  type ScheduledTaskTemplate,
  type StatusPresentation,
} from '@/utils/scheduledTasks/presentation'
import { READ_ONLY_REASON } from '@/utils/topology/inspectorRegistry'

const store = useScheduledTasksStore()
const auth = useAuthStore()

const headers = [
  { title: 'Tarefa', key: 'name', sortable: false },
  { title: 'Quando', key: 'schedule', sortable: false },
  { title: 'Última', key: 'last', sortable: false },
  { title: 'Próxima', key: 'next', sortable: false },
  { title: 'Ativa', key: 'enabled', sortable: false },
  { title: '', key: 'actions', sortable: false },
]

const dialog = reactive({
  open: false,
  task: null as ScheduledTask | null,
  template: null as ScheduledTaskTemplate | null,
})
const runsDialog = reactive({ open: false, task: null as ScheduledTask | null })
const feedback = reactive({ show: false, text: '', color: 'success' })
const busyId = ref<number | null>(null)

/** O modelo de container só aparece para quem pode agendá-lo. */
const templates = computed(() =>
  TASK_TEMPLATES.filter((template) => auth.isAdmin || template.tool !== 'docker_container_action')
)

function notify(text: string, color = 'success') {
  Object.assign(feedback, { show: true, text, color })
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

/** Rótulo e ícone da ferramenta; a cor vem do tipo (consulta, teste, ação). */
function toolLook(item: ScheduledTask): StatusPresentation {
  const kind = TOOL_KIND[store.toolOf(item.tool)?.kind ?? 'query']
  const meta = aiToolMeta(item.tool)
  return { label: meta.label, icon: meta.icon, color: kind.color }
}

function nextLabel(item: ScheduledTask): string {
  if (!item.enabled) return 'pausada'
  if (item.running) return 'rodando agora'
  return formatTimeUntil(item.nextRunAt)
}

function openDialog(task: ScheduledTask | null, template: ScheduledTaskTemplate | null = null) {
  dialog.task = task
  dialog.template = template
  dialog.open = true
}

function openRuns(task: ScheduledTask) {
  runsDialog.task = task
  runsDialog.open = true
}

function onSaved(saved: ScheduledTask) {
  notify(`Tarefa "${saved.name}" salva — ${saved.scheduleLabel}.`)
}

async function toggle(item: ScheduledTask, enabled: boolean) {
  busyId.value = item.id
  try {
    await store.setEnabled(item.id, enabled)
    notify(enabled ? `"${item.name}" retomada.` : `"${item.name}" pausada.`)
  } catch (err: unknown) {
    notify(describe(err, 'Não foi possível alterar a tarefa.'), 'error')
  } finally {
    busyId.value = null
  }
}

async function runNow(item: ScheduledTask) {
  const kind = store.toolOf(item.tool)?.kind
  if (kind === 'action' || kind === 'container') {
    const ok = await confirm({
      title: 'Rodar agora',
      message: `Rodar "${item.name}" agora, fora do horário? A ação acontece de verdade.`,
      confirmText: 'Rodar agora',
      confirmColor: 'primary',
    })
    if (!ok) return
  }
  busyId.value = item.id
  try {
    await store.runNow(item.id)
    notify(`"${item.name}" começou a rodar. O resultado aparece no histórico.`)
  } catch (err: unknown) {
    notify(describe(err, 'Não foi possível rodar a tarefa.'), 'error')
  } finally {
    busyId.value = null
  }
}

async function remove(item: ScheduledTask) {
  const ok = await confirm({
    title: 'Excluir tarefa',
    message: `Excluir "${item.name}"? O histórico de execuções dela vai junto. Para só parar, use o botão "Ativa".`,
    confirmText: 'Excluir',
    confirmColor: 'error',
  })
  if (!ok) return
  try {
    await store.remove(item.id)
    notify('Tarefa excluída.')
  } catch (err: unknown) {
    notify(describe(err, 'Não foi possível excluir a tarefa.'), 'error')
  }
}

onMounted(() => {
  // Abrir a página é a ação que carrega; mudanças chegam pelo SSE.
  void store.fetchTasks()
  void store.fetchCatalog()
})
</script>

<style scoped>
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 8px;
  color: rgb(var(--v-theme-on-surface));
}
.empty p {
  max-width: 560px;
}
.last-message {
  max-width: 280px;
  color: rgb(var(--v-theme-on-surface));
}
</style>

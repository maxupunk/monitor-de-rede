<template>
  <div>
    <div class="d-flex align-center flex-wrap ga-2 pa-3 pa-sm-4">
      <p class="text-body-medium intro mb-0 flex-grow-1">
        Testes que rodam sozinhos, no intervalo e na janela de horário escolhidos. Cada execução
        aparece no histórico.
      </p>
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
              Novo agendamento
            </v-btn>
          </span>
        </template>
      </v-tooltip>
    </div>

    <v-alert
      v-if="store.schedulesError"
      type="error"
      variant="tonal"
      density="compact"
      class="mx-4 mb-3"
    >
      {{ store.schedulesError }}
    </v-alert>

    <div v-if="!store.schedulesLoading && store.schedules.length === 0" class="empty pa-8">
      <v-icon size="48" color="primary">mdi-calendar-clock</v-icon>
      <div class="text-title-medium font-weight-bold">Nenhum agendamento</div>
      <p class="text-body-medium mb-2">
        Agende um teste para acompanhar a capacidade de um enlace ao longo do tempo — de madrugada,
        por exemplo, quando ninguém sente a saturação.
      </p>
      <v-btn
        v-if="auth.canWrite"
        color="primary"
        variant="flat"
        prepend-icon="mdi-calendar-plus"
        @click="openDialog(null)"
      >
        Criar o primeiro agendamento
      </v-btn>
    </div>

    <ResponsiveDataTable
      v-else
      :headers="headers"
      :items="store.schedules"
      :loading="store.schedulesLoading"
      :items-per-page="-1"
      hide-default-footer
      no-data-text="Nenhum agendamento"
      :item-key="(item: LinkTestSchedule) => item.id"
    >
      <template #item.name="{ item }">
        <div class="font-weight-bold">{{ item.name }}</div>
        <div class="text-body-small">{{ targetOf(item) }}</div>
      </template>
      <template #item.frequency="{ item }">
        <div>{{ intervalLabel(item.intervalMinutes) }}</div>
        <div v-if="windowLabel(item.windowStart, item.windowEnd)" class="text-body-small">
          {{ windowLabel(item.windowStart, item.windowEnd) }}
        </div>
      </template>
      <template #item.last="{ item }">
        <div class="d-flex align-center ga-2">
          <LinkTestStatusChip v-if="item.lastStatus" :status="item.lastStatus" size="x-small" />
          <span class="text-body-small">{{ formatRelativeTime(item.lastRunAt) }}</span>
        </div>
      </template>
      <template #item.next="{ item }">
        <span class="text-body-small">
          {{ item.enabled ? formatTimeUntil(item.nextRunAt) : 'pausado' }}
        </span>
      </template>
      <template #item.enabled="{ item }">
        <v-switch
          :model-value="item.enabled"
          color="success"
          inset
          hide-details
          density="compact"
          :disabled="!auth.canWrite || busyId === item.id"
          :aria-label="item.enabled ? 'Pausar agendamento' : 'Ativar agendamento'"
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
            :disabled="!auth.canWrite"
            :loading="busyId === item.id"
            @click="runNow(item)"
          >
            Rodar agora
          </v-btn>
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
              :aria-label="item.enabled ? 'Pausar agendamento' : 'Ativar agendamento'"
              @update:model-value="(value) => toggle(item, Boolean(value))"
            ></v-switch>
          </div>
          <div class="text-body-small">{{ targetOf(item) }}</div>
          <div class="text-body-small">
            {{ intervalLabel(item.intervalMinutes) }}
            <template v-if="windowLabel(item.windowStart, item.windowEnd)">
              · {{ windowLabel(item.windowStart, item.windowEnd) }}
            </template>
          </div>
          <div class="d-flex align-center ga-2">
            <LinkTestStatusChip v-if="item.lastStatus" :status="item.lastStatus" size="x-small" />
            <span class="text-body-small">último {{ formatRelativeTime(item.lastRunAt) }}</span>
          </div>
          <div class="d-flex align-center justify-end ga-1 pt-2 border-t">
            <v-btn
              color="primary"
              variant="flat"
              size="small"
              prepend-icon="mdi-play"
              :disabled="!auth.canWrite"
              :loading="busyId === item.id"
              @click="runNow(item)"
            >
              Rodar agora
            </v-btn>
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

    <LinkTestScheduleDialog v-model="dialog.open" :schedule="dialog.schedule" @saved="onSaved" />
    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="4000">
      {{ feedback.text }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
/** Aba "Agendamentos": lista, criar/editar, pausar, rodar agora e excluir. */
import { onMounted, reactive, ref } from 'vue'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import { useDevicesStore } from '@/stores/devices'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTestSchedule, LinkTestScheduleInput } from '@/types/linkTests'
import { formatRelativeTime, formatTimeUntil } from '@/utils/formatters'
import { TEST_KINDS, requestTargetLabel } from '@/utils/linkTests/presentation'
import {
  estimatedDurationText,
  estimateSeconds,
  intervalLabel,
  windowLabel,
} from '@/utils/linkTests/spec'
import { READ_ONLY_REASON } from '@/utils/topology/inspectorRegistry'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import LinkTestScheduleDialog from './LinkTestScheduleDialog.vue'
import LinkTestStatusChip from './LinkTestStatusChip.vue'

const emit = defineEmits<{ started: [testId: number] }>()

const store = useLinkTestsStore()
const auth = useAuthStore()
const devices = useDevicesStore()

const headers = [
  { title: 'Agendamento', key: 'name', sortable: false },
  { title: 'Frequência', key: 'frequency', sortable: false },
  { title: 'Último', key: 'last', sortable: false },
  { title: 'Próximo', key: 'next', sortable: false },
  { title: 'Ativo', key: 'enabled', sortable: false },
  { title: '', key: 'actions', sortable: false },
]

const dialog = reactive({ open: false, schedule: null as LinkTestSchedule | null })
const feedback = reactive({ show: false, text: '', color: 'success' })
const busyId = ref<number | null>(null)

function notify(text: string, color = 'success') {
  Object.assign(feedback, { show: true, text, color })
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

function deviceName(id: number): string {
  return devices.devices.find((device) => device.id === id)?.name ?? `#${id}`
}

function targetOf(item: LinkTestSchedule): string {
  return `${TEST_KINDS[item.kind].label} · ${requestTargetLabel(item, deviceName)}`
}

function openDialog(schedule: LinkTestSchedule | null) {
  dialog.schedule = schedule
  dialog.open = true
}

function onSaved(saved: LinkTestSchedule) {
  notify(`Agendamento "${saved.name}" salvo.`)
}

function inputOf(item: LinkTestSchedule, enabled: boolean): LinkTestScheduleInput {
  return {
    name: item.name,
    kind: item.kind,
    linkId: item.linkId,
    sourceDeviceId: item.sourceDeviceId,
    targetDeviceId: item.targetDeviceId,
    spec: item.spec,
    intervalMinutes: item.intervalMinutes,
    windowStart: item.windowStart,
    windowEnd: item.windowEnd,
    enabled,
  }
}

async function toggle(item: LinkTestSchedule, enabled: boolean) {
  busyId.value = item.id
  try {
    await store.updateSchedule(item.id, inputOf(item, enabled))
    notify(enabled ? `"${item.name}" ativado.` : `"${item.name}" pausado.`)
  } catch (err: unknown) {
    notify(describe(err, 'Não foi possível alterar o agendamento.'), 'error')
  } finally {
    busyId.value = null
  }
}

async function runNow(item: LinkTestSchedule) {
  const ok = await confirm({
    title: 'Rodar agora',
    message: `Rodar "${item.name}" agora? O enlace fica saturado durante a medição (${estimatedDurationText(estimateSeconds(item.spec))} por enlace).`,
    confirmText: 'Rodar agora',
    confirmColor: 'primary',
  })
  if (!ok) return
  busyId.value = item.id
  try {
    const id = await store.runScheduleNow(item.id)
    notify('Teste iniciado.')
    emit('started', id)
  } catch (err: unknown) {
    notify(describe(err, 'Não foi possível iniciar o teste.'), 'error')
  } finally {
    busyId.value = null
  }
}

async function remove(item: LinkTestSchedule) {
  const ok = await confirm({
    title: 'Excluir agendamento',
    message: `Excluir "${item.name}"? Os testes que ele já rodou continuam no histórico.`,
    confirmText: 'Excluir',
    confirmColor: 'error',
  })
  if (!ok) return
  try {
    await store.deleteSchedule(item.id)
    notify('Agendamento excluído.')
  } catch (err: unknown) {
    notify(describe(err, 'Não foi possível excluir o agendamento.'), 'error')
  }
}

onMounted(() => {
  // Abrir a aba é a ação que carrega a lista; mudanças chegam pelo SSE.
  void store.fetchSchedules()
  if (devices.devices.length === 0) void devices.fetchDevices()
})
</script>

<style scoped>
.intro {
  color: rgb(var(--v-theme-on-surface));
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 8px;
  color: rgb(var(--v-theme-on-surface));
}
.empty p {
  max-width: 520px;
}
</style>

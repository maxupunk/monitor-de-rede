<template>
  <v-dialog
    :model-value="modelValue"
    max-width="900"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="build" data-test="build-dialog">
      <v-card-item class="px-6 pt-5">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-hammer-wrench</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-wrap">
          Compilação #{{ build.id }} — {{ build.application }}
        </v-card-title>
        <v-card-subtitle class="text-high-emphasis">
          {{ subtitle }}
        </v-card-subtitle>
        <template #append>
          <StatusMetaChip :meta="BUILD_STATUS[build.status]" />
        </template>
      </v-card-item>

      <v-card-text class="px-6">
        <div v-if="build.commitMessage" class="text-body-2 mb-3">
          <v-icon size="small" color="primary" class="mr-1">mdi-source-commit</v-icon>
          {{ firstLine(build.commitMessage) }}
        </div>

        <div class="d-flex flex-wrap ga-2 mb-4" role="list" aria-label="Etapas da compilação">
          <v-chip
            v-for="stage in BUILD_STAGES"
            :key="stage.value"
            :color="STAGE_COLOR[states[stage.value]]"
            :variant="states[stage.value] === 'todo' ? 'outlined' : 'tonal'"
            :prepend-icon="stage.icon"
            size="small"
            label
            role="listitem"
          >
            {{ stage.label }}
            <template
              v-if="stage.value === 'build' && build.currentImage && states.build === 'current'"
            >
              · {{ build.currentImage }}
            </template>
          </v-chip>
        </div>

        <v-alert v-if="build.error" type="error" variant="tonal" density="compact" class="mb-4">
          {{ build.error }}
        </v-alert>

        <v-card v-if="build.images.length" border flat class="pa-3 mb-4">
          <div class="text-subtitle-2 font-weight-bold mb-2">Imagens no registry</div>
          <div
            v-for="image in build.images"
            :key="image.repository"
            class="d-flex align-center ga-2 text-body-2 mb-1"
          >
            <v-icon size="small" color="success">mdi-package-variant-closed</v-icon>
            <span class="font-weight-bold">{{ image.repository }}</span>
            <code>{{ shortDigest(image.digest) }}</code>
            <v-spacer></v-spacer>
            <span>{{ formatBinaryBytes(image.bytes) }}</span>
          </div>
        </v-card>

        <DeployConsole
          :lines="lines"
          :live="BUILD_STATUS[build.status].live"
          :error="consoleError"
          :empty-text="
            build.status === 'queued' ? 'Esperando a vez do builder…' : 'Nenhuma linha registrada.'
          "
        />
      </v-card-text>

      <v-card-actions class="px-6 pb-4 flex-wrap ga-2">
        <v-btn
          v-if="canAdmin && !finished"
          color="error"
          variant="tonal"
          prepend-icon="mdi-cancel"
          :loading="busy"
          @click="cancel"
        >
          Cancelar compilação
        </v-btn>
        <v-btn
          v-else-if="canAdmin"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-refresh"
          :loading="busy"
          @click="rebuild"
        >
          Recompilar sem cache
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" @click="emit('update:modelValue', false)"
          >Fechar</v-btn
        >
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import DeployConsole from '@/components/deploys/DeployConsole.vue'
import StatusMetaChip from '@/components/deploys/StatusMetaChip.vue'
import { useConfirm } from '@/composables/useConfirm'
import { useDeployBuildsStore } from '@/stores/deployBuilds'
import { shortDigest } from '@/utils/registryPresentation'
import {
  BUILD_STAGES,
  BUILD_STATUS,
  buildStageStates,
  refName,
  type StageState,
} from '@/utils/deployPresentation'
import { formatBinaryBytes, formatElapsedMs, formatRelativeTime } from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean; buildId: number | null; canAdmin: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; opened: [id: number] }>()

const builds = useDeployBuildsStore()
const { confirm } = useConfirm()
const busy = ref(false)
const consoleError = ref<string | null>(null)

const STAGE_COLOR: Record<StageState, string> = {
  done: 'success',
  current: 'primary',
  failed: 'error',
  todo: 'secondary',
}

const build = computed(() =>
  props.buildId === null ? null : (builds.builds[props.buildId] ?? null)
)
const lines = computed(() => (props.buildId === null ? [] : (builds.consoles[props.buildId] ?? [])))
const finished = computed(() => (build.value ? !BUILD_STATUS[build.value.status].live : true))
const states = computed(() =>
  buildStageStates(build.value?.stage ?? null, build.value?.status ?? 'queued')
)

const subtitle = computed(() => {
  const item = build.value
  if (!item) return ''
  const reference = refName(item.reference)
  const parts = [
    `commit ${item.shortSha}`,
    reference && `branch ${reference}`,
    item.agent && `em ${item.agent}`,
    item.triggeredBy && `por ${item.triggeredBy}`,
    item.durationMs != null ? `levou ${formatElapsedMs(item.durationMs)}` : '',
    formatRelativeTime(item.createdAt),
  ]
  return parts.filter(Boolean).join(' · ')
})

function firstLine(text: string): string {
  return text.split('\n')[0] ?? text
}

async function cancel() {
  if (!build.value) return
  const ok = await confirm({
    title: 'Cancelar a compilação?',
    message: 'O builder para onde estiver; nada sobe e a compilação precisa ser pedida de novo.',
    confirmText: 'Cancelar compilação',
    confirmColor: 'error',
    icon: 'mdi-cancel',
  })
  if (!ok) return
  busy.value = true
  try {
    await builds.cancel(build.value.id)
  } catch (err) {
    consoleError.value = err instanceof Error ? err.message : 'Não foi possível cancelar'
  } finally {
    busy.value = false
  }
}

async function rebuild() {
  if (!build.value) return
  busy.value = true
  try {
    const next = await builds.rebuild(build.value.id)
    emit('opened', next.id)
  } catch (err) {
    consoleError.value = err instanceof Error ? err.message : 'Não foi possível recompilar'
  } finally {
    busy.value = false
  }
}

watch(
  () => [props.modelValue, props.buildId] as const,
  async ([open, id], previous) => {
    const before = previous?.[1]
    if (before != null && before !== id) builds.closeConsole(before)
    if (!open || id === null) {
      if (id !== null && !open) builds.closeConsole(id)
      return
    }
    consoleError.value = null
    try {
      await builds.openConsole(id)
    } catch (err) {
      consoleError.value = err instanceof Error ? err.message : 'Não foi possível ler o log'
    }
  },
  { immediate: true }
)
</script>

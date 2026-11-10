<template>
  <v-dialog
    :model-value="modelValue"
    max-width="980"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-item class="px-6 pt-5">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-package-variant-closed</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">{{
          detail?.repository.name ?? 'Imagem'
        }}</v-card-title>
        <v-card-subtitle v-if="detail" class="text-high-emphasis">
          <template v-if="detail.repository.application"
            >{{ detail.repository.application }} ·
          </template>
          {{ countNoun(detail.versions.length, 'versão', 'versões') }} ·
          {{ formatBinaryBytes(detail.repository.bytes) }}
        </v-card-subtitle>
      </v-card-item>

      <v-card-text class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">
          {{ error }}
        </v-alert>
        <v-progress-linear
          v-if="loading"
          indeterminate
          color="primary"
          class="mb-3"
        ></v-progress-linear>

        <v-card
          v-for="version in detail?.versions ?? []"
          :key="version.id"
          border
          flat
          class="pa-3 mb-3"
          data-test="registry-version"
        >
          <div class="d-flex align-center flex-wrap ga-2 mb-1">
            <v-chip
              v-for="tag in version.tags"
              :key="tag"
              size="small"
              color="primary"
              variant="tonal"
              label
              :prepend-icon="isCommitTag(tag) ? 'mdi-lock' : 'mdi-tag'"
            >
              {{ shortTag(tag) }}
            </v-chip>
            <v-chip
              v-if="version.pinned"
              size="small"
              color="info"
              variant="flat"
              label
              prepend-icon="mdi-pin"
            >
              Fixada
            </v-chip>
            <v-chip
              v-for="target in version.usedBy"
              :key="target"
              size="small"
              color="success"
              variant="flat"
              label
              prepend-icon="mdi-server"
            >
              No ar em {{ target }}
            </v-chip>
            <v-spacer></v-spacer>
            <span class="text-body-2">{{ formatBinaryBytes(version.bytes) }}</span>
          </div>
          <div class="text-body-2 mb-2">
            <code>{{ shortDigest(version.digest) }}</code>
            <template v-if="version.platforms.length">
              ·
              <span :title="version.platforms.join(', ')">{{
                platformsLabel(version.platforms)
              }}</span></template
            >
            · {{ formatRelativeTime(version.createdAt) }}
            <template v-if="origin(version)"> · {{ origin(version) }}</template>
          </div>
          <div class="d-flex align-center flex-wrap ga-2">
            <v-chip
              :color="scanColor(version.scan)"
              size="small"
              variant="tonal"
              label
              prepend-icon="mdi-shield-search"
            >
              {{ scanSentence(version.scan) }}
            </v-chip>
            <v-spacer></v-spacer>
            <v-btn
              size="small"
              variant="tonal"
              color="primary"
              prepend-icon="mdi-shield-search"
              @click="openFindings(version)"
            >
              Vulnerabilidades
            </v-btn>
            <v-btn
              size="small"
              variant="text"
              color="primary"
              :prepend-icon="version.pinned ? 'mdi-pin-off' : 'mdi-pin'"
              @click="togglePin(version)"
            >
              {{ version.pinned ? 'Soltar' : 'Fixar' }}
            </v-btn>
            <v-tooltip v-if="canAdmin" :text="blocked(version) ?? ''" :disabled="!blocked(version)">
              <template #activator="{ props: tooltip }">
                <span v-bind="tooltip">
                  <v-btn
                    size="small"
                    variant="text"
                    color="error"
                    prepend-icon="mdi-delete"
                    :disabled="blocked(version) !== null"
                    @click="remove(version)"
                  >
                    Apagar
                  </v-btn>
                </span>
              </template>
            </v-tooltip>
          </div>
        </v-card>
        <div v-if="detail && !detail.versions.length" class="text-body-1">
          Nenhuma versão com tag.
        </div>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" @click="emit('update:modelValue', false)"
          >Fechar</v-btn
        >
      </v-card-actions>
    </v-card>

    <RegistryFindingsDialog
      v-model="findingsOpen"
      :version="selectedVersion"
      :application-id="detail?.repository.applicationId ?? null"
      :can-admin="canAdmin"
    />
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import RegistryFindingsDialog from '@/components/registry/RegistryFindingsDialog.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { RegistryRepositoryDetail } from '@/bindings/RegistryRepositoryDetail'
import type { RegistryVersionView } from '@/bindings/RegistryVersionView'
import { useRegistryStore } from '@/stores/registry'
import { countNoun, formatBinaryBytes, formatRelativeTime } from '@/utils/formatters'
import {
  ORIGIN_LABEL,
  isCommitTag,
  platformsLabel,
  scanColor,
  scanSentence,
  shortDigest,
  shortTag,
} from '@/utils/registryPresentation'
import type { DeployOriginKind } from '@/bindings/DeployOriginKind'

const props = defineProps<{ modelValue: boolean; repositoryId: number | null; canAdmin: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const registry = useRegistryStore()
const { confirm } = useConfirm()
const detail = ref<RegistryRepositoryDetail | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const findingsOpen = ref(false)
const selectedVersion = ref<RegistryVersionView | null>(null)

/** Por que a versão não pode ser apagada (a mesma regra do backend), ou nulo. */
function blocked(version: RegistryVersionView): string | null {
  if (version.usedBy.length) return 'No ar em um alvo: não pode ser apagada'
  if (version.protected) return 'Guardada para rollback de um deploy recente'
  if (version.pinned) return 'Fixada: solte antes de apagar'
  return null
}

/** De onde a versão veio, em poucas palavras. */
function origin(version: RegistryVersionView): string {
  const source = version.provenance?.source
  const label =
    typeof source === 'string' && source in ORIGIN_LABEL
      ? ORIGIN_LABEL[source as DeployOriginKind]
      : null
  const by = version.provenance?.triggeredBy ?? version.pushedBy
  return [label, typeof by === 'string' && by ? `por ${by}` : ''].filter(Boolean).join(' ')
}

async function load() {
  if (props.repositoryId === null) return
  loading.value = true
  error.value = null
  try {
    detail.value = await registry.detail(props.repositoryId)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível ler as versões'
  } finally {
    loading.value = false
  }
}

function openFindings(version: RegistryVersionView) {
  selectedVersion.value = version
  findingsOpen.value = true
}

async function togglePin(version: RegistryVersionView) {
  try {
    await registry.pin(version.id, !version.pinned)
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível fixar'
  }
}

async function remove(version: RegistryVersionView) {
  const ok = await confirm({
    title: 'Apagar esta versão?',
    message: `As tags ${version.tags.join(', ')} deixam de existir. O espaço volta na próxima limpeza.`,
    confirmText: 'Apagar',
    confirmColor: 'error',
    icon: 'mdi-delete',
  })
  if (!ok) return
  try {
    await registry.removeVersion(version.id)
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível apagar'
  }
}

watch(
  () => [props.modelValue, props.repositoryId] as const,
  ([open]) => {
    if (open) void load()
  },
  { immediate: true }
)

// Imagem nova ou análise concluída com o diálogo aberto: relê uma vez (SSE).
watch(
  () => registry.revision,
  () => {
    if (props.modelValue) void load()
  }
)
</script>

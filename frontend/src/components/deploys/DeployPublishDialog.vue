<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 640"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="target" class="rounded-lg">
      <v-card-item class="pt-4 px-6">
        <template #prepend>
          <v-avatar color="success" variant="tonal" rounded="lg">
            <v-icon>mdi-rocket-launch</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Publicar em {{ target.name }}</v-card-title>
        <v-card-subtitle class="text-wrap text-high-emphasis">
          Projeto <code>{{ target.project }}</code> em {{ target.agent?.name ?? 'agente removido' }}
        </v-card-subtitle>
      </v-card-item>

      <v-card-text class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" class="mb-4">{{ error }}</v-alert>

        <div class="text-subtitle-2 font-weight-bold mb-2">Qual versão</div>
        <v-skeleton-loader v-if="loading" type="list-item-two-line@3"></v-skeleton-loader>
        <v-radio-group v-else v-model="choice" hide-details class="mb-2">
          <v-radio v-if="target.live" value="live" color="primary">
            <template #label>
              <div>
                <div class="font-weight-bold">
                  Republicar a versão no ar ({{ target.live.shortSha }})
                </div>
                <div class="text-body-2 text-high-emphasis">
                  Recria os containers com o .env atual — use depois de mudar variáveis.
                </div>
              </div>
            </template>
          </v-radio>
          <v-radio
            v-for="version in versions"
            :key="version.commitSha"
            :value="version.commitSha"
            color="primary"
          >
            <template #label>
              <div>
                <div class="font-weight-bold">
                  {{ version.shortSha }} —
                  {{ commitTitle(version.commitMessage) || 'sem mensagem' }}
                </div>
                <div class="text-body-2 text-high-emphasis">
                  {{ version.reference ?? '' }} · {{ formatRelativeTime(version.createdAt) }}
                </div>
              </div>
            </template>
          </v-radio>
          <v-radio value="other" color="primary" label="Outro commit"></v-radio>
        </v-radio-group>
        <v-text-field
          v-if="choice === 'other'"
          v-model="otherSha"
          label="SHA completo do commit"
          hint="40 caracteres; a imagem com essa tag precisa estar publicada"
          persistent-hint
          variant="outlined"
          class="mb-2"
        ></v-text-field>

        <div class="text-subtitle-2 font-weight-bold mt-4 mb-2">Quando</div>
        <v-btn-toggle v-model="when" color="primary" mandatory divided density="comfortable">
          <v-btn value="now" prepend-icon="mdi-flash">Agora</v-btn>
          <v-btn v-if="windowText" value="window" prepend-icon="mdi-calendar-clock">
            Na janela ({{ windowText }})
          </v-btn>
        </v-btn-toggle>
        <div v-if="target.requiresApproval" class="text-body-2 text-high-emphasis mt-3">
          <v-icon size="16" color="info">mdi-information</v-icon>
          Este alvo pede aprovação: publicar por aqui já é a sua aprovação.
        </div>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="emit('update:modelValue', false)">Cancelar</v-btn>
        <v-btn
          color="success"
          variant="flat"
          prepend-icon="mdi-rocket-launch"
          :loading="saving"
          :disabled="!canPublish"
          @click="publish"
        >
          Publicar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { DeployRunView } from '@/bindings/DeployRunView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import type { DeployWhen } from '@/bindings/DeployWhen'
import { useDeploysStore } from '@/stores/deploys'
import { commitTitle, windowLabel } from '@/utils/deployPresentation'
import { formatRelativeTime } from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean; target: DeployTargetView | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; started: [run: DeployRunView] }>()

const deploys = useDeploysStore()
const loading = ref(false)
const saving = ref(false)
const error = ref<string | null>(null)
const history = ref<DeployRunView[]>([])
const choice = ref<string>('live')
const otherSha = ref('')
const when = ref<DeployWhen>('now')

const windowText = computed(() =>
  props.target ? windowLabel(props.target.windowStart, props.target.windowEnd) : ''
)

/** As versões conhecidas (uma por commit), sem a que está no ar. */
const versions = computed(() => {
  const seen = new Set<string>([props.target?.live?.commitSha ?? ''])
  const list: DeployRunView[] = []
  for (const run of history.value) {
    if (seen.has(run.commitSha)) continue
    seen.add(run.commitSha)
    list.push(run)
    if (list.length >= 8) break
  }
  return list
})

const SHA = /^[0-9a-f]{40}$/i
const canPublish = computed(() => {
  if (choice.value === 'other') return SHA.test(otherSha.value.trim())
  return Boolean(choice.value)
})

watch(
  () => props.modelValue,
  async (visible) => {
    if (!visible || !props.target) return
    error.value = null
    otherSha.value = ''
    when.value = 'now'
    choice.value = props.target.live ? 'live' : ''
    loading.value = true
    try {
      history.value = await deploys.history(props.target.id)
      if (!choice.value) choice.value = versions.value[0]?.commitSha ?? 'other'
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Não foi possível ler o histórico'
    } finally {
      loading.value = false
    }
  },
  { immediate: true }
)

async function publish() {
  if (!props.target) return
  saving.value = true
  error.value = null
  try {
    const commitSha =
      choice.value === 'live'
        ? undefined
        : choice.value === 'other'
          ? otherSha.value.trim().toLowerCase()
          : choice.value
    const run = await deploys.deploy(props.target.id, { commitSha, when: when.value })
    emit('started', run)
    emit('update:modelValue', false)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível publicar'
  } finally {
    saving.value = false
  }
}
</script>

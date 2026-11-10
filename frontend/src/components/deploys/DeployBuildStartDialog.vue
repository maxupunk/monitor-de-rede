<template>
  <v-dialog
    :model-value="modelValue"
    max-width="560"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="application">
      <v-card-item class="px-6 pt-5">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-hammer-wrench</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Compilar agora</v-card-title>
        <v-card-subtitle class="text-high-emphasis">{{ application.name }}</v-card-subtitle>
      </v-card-item>
      <v-card-text class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">
          {{ error }}
        </v-alert>
        <v-combobox
          v-model="reference"
          :items="branches"
          label="Branch"
          hint="Compila o commit mais novo dela; os alvos que a acompanham recebem o deploy"
          persistent-hint
          variant="outlined"
          prepend-inner-icon="mdi-source-branch"
          class="mb-2"
        ></v-combobox>
        <v-text-field
          v-model="commit"
          label="Commit exato (opcional)"
          placeholder="Vazio: o mais novo da branch"
          variant="outlined"
          prepend-inner-icon="mdi-source-commit"
          :rules="[commitRule]"
        ></v-text-field>
        <v-alert
          v-if="commit.trim()"
          type="warning"
          variant="tonal"
          density="compact"
          class="mb-2"
          data-test="exact-commit-warning"
        >
          Os alvos que acompanham a branch recebem este commit, mesmo que seja mais antigo do que o
          que está no ar — na prática, um rollback.
        </v-alert>
        <v-switch
          v-model="noCache"
          color="primary"
          inset
          hide-details
          label="Sem cache (compila tudo do zero)"
        ></v-switch>
      </v-card-text>
      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" color="primary" @click="emit('update:modelValue', false)">
          Cancelar
        </v-btn>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-play"
          :loading="busy"
          :disabled="!reference || commitRule(commit) !== true"
          data-test="build-start"
          @click="start"
        >
          Compilar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import { useDeployBuildsStore } from '@/stores/deployBuilds'

const props = defineProps<{ modelValue: boolean; application: DeployApplicationView | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; started: [id: number] }>()

const builds = useDeployBuildsStore()
const reference = ref('')
const commit = ref('')
const noCache = ref(false)
const busy = ref(false)
const error = ref<string | null>(null)

/** As branches que os alvos acompanham (padrão de tag não compila à mão). */
const branches = computed(() => [
  ...new Set(
    (props.application?.targets ?? [])
      .map((target) => target.reference)
      .filter((value) => !value.includes('*'))
  ),
])

function commitRule(value: string): true | string {
  return (
    !value.trim() || /^[0-9a-fA-F]{40}$/.test(value.trim()) || 'O commit completo (40 caracteres)'
  )
}

async function start() {
  if (!props.application) return
  busy.value = true
  error.value = null
  try {
    const build = await builds.start(props.application.id, {
      reference: reference.value.trim(),
      commit: commit.value.trim() || undefined,
      noCache: noCache.value,
    })
    emit('update:modelValue', false)
    emit('started', build.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível compilar'
  } finally {
    busy.value = false
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    reference.value = branches.value[0] ?? 'main'
    commit.value = ''
    noCache.value = false
    error.value = null
  }
)
</script>

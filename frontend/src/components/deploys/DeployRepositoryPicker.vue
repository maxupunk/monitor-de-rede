<template>
  <v-autocomplete
    :model-value="modelValue || null"
    :items="repositories"
    item-title="fullName"
    item-value="fullName"
    label="Repositório *"
    placeholder="Digite para buscar"
    variant="outlined"
    prepend-inner-icon="mdi-source-repository"
    :loading="loading"
    :error-messages="error ? [error] : []"
    :hint="hint"
    persistent-hint
    :rules="[(value: string | null) => Boolean(value) || 'Escolha o repositório']"
    no-data-text="Nenhum repositório com esse nome na instalação"
    @update:model-value="choose"
  >
    <template #item="{ props: itemProps, item }">
      <v-list-item v-bind="itemProps" :subtitle="item.description ?? undefined">
        <template #append>
          <v-chip size="x-small" variant="flat" :color="item.private ? 'warning' : 'success'">
            {{ item.private ? 'Privado' : 'Público' }}
          </v-chip>
        </template>
      </v-list-item>
    </template>
    <template #append>
      <IconAction icon="mdi-refresh" label="Ler a lista de novo" :loading="loading" @click="load" />
    </template>
  </v-autocomplete>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import IconAction from '@/components/IconAction.vue'
import type { DeployRepositoryView } from '@/bindings/DeployRepositoryView'
import { useDeploysStore, type GithubSource } from '@/stores/deploys'

const props = defineProps<{ modelValue: string; source: GithubSource }>()
const emit = defineEmits<{
  'update:modelValue': [value: string]
  chosen: [repository: DeployRepositoryView]
}>()

const deploys = useDeploysStore()
const repositories = ref<DeployRepositoryView[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const hint = computed(() =>
  repositories.value.length
    ? `${repositories.value.length} repositório(s) que a conexão enxerga. Falta algum? Em Conexões, escolha os repositórios do app.`
    : ''
)

function choose(value: string | null) {
  emit('update:modelValue', value ?? '')
  const repository = repositories.value.find((item) => item.fullName === value)
  if (repository) emit('chosen', repository)
}

async function load() {
  loading.value = true
  error.value = null
  try {
    repositories.value = await deploys.githubRepositories(props.source)
  } catch (err) {
    repositories.value = []
    error.value = err instanceof Error ? err.message : 'Não foi possível listar os repositórios'
  } finally {
    loading.value = false
  }
}

watch(
  () => [props.source.connectionId, props.source.credentialId] as const,
  () => void load(),
  { immediate: true }
)
</script>

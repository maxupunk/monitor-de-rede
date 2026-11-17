<template>
  <section class="container-picker rounded-lg border pa-3 mb-4" aria-label="Bancos em contêineres">
    <div class="d-flex align-center flex-wrap ga-2 mb-2">
      <v-icon color="primary">mdi-docker</v-icon>
      <div class="flex-grow-1 min-w-0">
        <div class="d-flex align-center flex-wrap ga-2">
          <span class="font-weight-bold text-body-2">Bancos em contêineres dos agentes</span>
          <v-chip v-if="ofEngine.length > 0" size="x-small" color="primary" variant="tonal">
            {{ runningCount }} {{ runningCount === 1 ? 'ativo' : 'ativos' }}
          </v-chip>
          <v-chip
            v-if="protectedCount > 0"
            size="x-small"
            color="success"
            variant="tonal"
            prepend-icon="mdi-shield-check"
          >
            {{ protectedCount }} já no backup
          </v-chip>
        </div>
        <div v-if="!collapsed" class="text-caption text-high-emphasis">
          Escolha um contêiner pela busca, pelo seletor ou na lista: o endereço é testado na hora.
        </div>
        <div v-else-if="selectedItem" class="text-caption text-high-emphasis">
          Selecionado: <strong>{{ selectedItem.containerName }}</strong> ({{
            selectedItem.local ? 'Central' : selectedItem.hostName
          }})
        </div>
      </div>
      <v-btn
        v-if="selectedItem"
        size="small"
        color="primary"
        variant="text"
        :prepend-icon="collapsed ? 'mdi-chevron-down' : 'mdi-chevron-up'"
        @click="collapsed = !collapsed"
      >
        {{ collapsed ? 'Trocar contêiner' : 'Recolher' }}
      </v-btn>
      <v-btn
        size="small"
        color="primary"
        variant="tonal"
        prepend-icon="mdi-refresh"
        :loading="store.containersLoading"
        @click="store.fetchContainers()"
      >
        Procurar de novo
      </v-btn>
    </div>

    <template v-if="!collapsed">
      <v-progress-linear
        v-if="store.containersLoading && !ofEngine.length"
        indeterminate
        color="primary"
        class="my-2"
      ></v-progress-linear>

      <v-row v-if="ofEngine.length > 0" density="comfortable" class="mt-1 mb-2">
        <v-col cols="12" sm="6">
          <v-text-field
            v-model="search"
            prepend-inner-icon="mdi-magnify"
            placeholder="Buscar contêiner, projeto ou servidor…"
            variant="outlined"
            density="compact"
            clearable
            hide-details
            data-test="container-search"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6">
          <v-select
            :model-value="selectedKey"
            :items="selectOptions"
            item-title="title"
            item-value="value"
            label="Selecionar contêiner"
            prepend-inner-icon="mdi-cube-outline"
            variant="outlined"
            density="compact"
            clearable
            hide-details
            no-data-text="Nenhum contêiner correspondente"
            data-test="container-select"
            @update:model-value="onSelectKey"
          >
            <template #item="{ props: itemProps, item }">
              <v-list-item
                v-bind="itemProps"
                :title="item.container.containerName"
                :subtitle="containerSubtitle(item.container)"
              >
                <template #prepend>
                  <v-icon :color="engineInfo(item.container.engine).color" class="mr-2">
                    {{ engineInfo(item.container.engine).icon }}
                  </v-icon>
                </template>
                <template #append>
                  <v-chip
                    v-if="item.protectedMatch"
                    size="x-small"
                    color="success"
                    variant="flat"
                    prepend-icon="mdi-shield-check"
                  >
                    Já no backup
                  </v-chip>
                </template>
              </v-list-item>
            </template>
          </v-select>
        </v-col>
      </v-row>

      <div
        v-if="hostOptions.length > 1 || protectedCount > 0"
        class="d-flex flex-wrap align-center ga-1 mb-2"
      >
        <template v-if="hostOptions.length > 1">
          <v-chip
            size="x-small"
            :color="hostFilter === null ? 'primary' : 'secondary'"
            :variant="hostFilter === null ? 'flat' : 'tonal'"
            @click="hostFilter = null"
          >
            Todos os servidores
          </v-chip>
          <v-chip
            v-for="host in hostOptions"
            :key="host.key"
            size="x-small"
            :color="hostFilter === host.key ? 'primary' : 'info'"
            :variant="hostFilter === host.key ? 'flat' : 'tonal'"
            :prepend-icon="host.local ? 'mdi-server' : 'mdi-transit-connection-variant'"
            @click="hostFilter = hostFilter === host.key ? null : host.key"
          >
            {{ host.label }} ({{ host.count }})
          </v-chip>
        </template>

        <v-spacer v-if="hostOptions.length > 1 && protectedCount > 0"></v-spacer>

        <template v-if="protectedCount > 0">
          <v-chip
            size="x-small"
            :color="backupFilter === 'unprotected' ? 'primary' : 'secondary'"
            :variant="backupFilter === 'unprotected' ? 'flat' : 'tonal'"
            @click="backupFilter = backupFilter === 'unprotected' ? 'all' : 'unprotected'"
          >
            Sem backup ({{ unprotectedCount }})
          </v-chip>
          <v-chip
            size="x-small"
            color="success"
            :variant="backupFilter === 'protected' ? 'flat' : 'tonal'"
            prepend-icon="mdi-shield-check"
            @click="backupFilter = backupFilter === 'protected' ? 'all' : 'protected'"
          >
            Já no backup ({{ protectedCount }})
          </v-chip>
        </template>
      </div>

      <v-list
        v-if="items.length"
        density="compact"
        class="container-picker__list py-0 bg-transparent"
        lines="two"
      >
        <v-list-item
          v-for="item in items"
          :key="containerKey(item)"
          :title="item.containerName"
          :subtitle="containerSubtitle(item)"
          :active="selectedKey === containerKey(item)"
          color="primary"
          rounded="lg"
          class="mb-1"
          @click="pick(item)"
        >
          <template #prepend>
            <v-avatar :color="engineInfo(item.engine).color" variant="tonal" rounded="lg">
              <v-icon>{{ engineInfo(item.engine).icon }}</v-icon>
            </v-avatar>
          </template>
          <template #append>
            <div class="d-flex align-center flex-wrap justify-end ga-1">
              <v-chip
                v-if="protectedOf(item)"
                size="x-small"
                color="success"
                variant="flat"
                prepend-icon="mdi-shield-check"
                data-test="already-protected"
              >
                {{
                  protectedOf(item)!.name !== item.containerName
                    ? `Já no backup · ${protectedOf(item)!.name}`
                    : 'Já no backup'
                }}
              </v-chip>
              <v-chip
                class="d-none d-sm-inline-flex"
                size="x-small"
                variant="flat"
                :color="item.local ? 'primary' : 'info'"
                :prepend-icon="item.local ? 'mdi-server' : 'mdi-transit-connection-variant'"
              >
                {{ item.local ? 'Central' : item.hostName }}
              </v-chip>
              <v-chip v-if="!item.running" size="x-small" variant="flat" color="warning">
                Parado
              </v-chip>
              <v-btn
                v-if="protectedOf(item)"
                size="small"
                color="success"
                variant="tonal"
                @click.stop="emit('editConnection', protectedOf(item)!.id)"
              >
                Editar
              </v-btn>
              <v-btn
                size="small"
                color="primary"
                :variant="
                  selectedKey === containerKey(item)
                    ? 'flat'
                    : protectedOf(item)
                      ? 'text'
                      : 'outlined'
                "
                :loading="resolvingKey === containerKey(item)"
                @click.stop="pick(item)"
              >
                Usar
              </v-btn>
            </div>
          </template>
        </v-list-item>
      </v-list>

      <div
        v-else-if="!store.containersLoading"
        class="text-body-2 text-high-emphasis py-1"
        data-test="no-containers"
      >
        {{ emptyText }}
      </div>
      <v-btn
        v-if="stoppedCount"
        variant="text"
        color="primary"
        size="small"
        class="mt-1"
        :prepend-icon="showStopped ? 'mdi-eye-off-outline' : 'mdi-eye-outline'"
        @click="showStopped = !showStopped"
      >
        {{ showStopped ? 'Esconder os parados' : `Mostrar os parados (${stoppedCount})` }}
      </v-btn>
    </template>

    <v-alert
      v-if="store.containersError"
      type="warning"
      variant="tonal"
      density="compact"
      class="mt-2"
    >
      {{ store.containersError }}
    </v-alert>
    <v-alert
      v-for="issue in store.containers.issues"
      :key="issue.hostName"
      type="warning"
      variant="tonal"
      density="compact"
      class="mt-2"
    >
      <strong>{{ issue.hostName }}:</strong> {{ issue.reason }}
    </v-alert>
    <v-alert
      v-if="resolving"
      type="info"
      variant="tonal"
      density="compact"
      icon="mdi-lan-pending"
      class="mt-2"
    >
      Testando os endereços de <strong>{{ resolving.containerName }}</strong>
      {{ resolving.local ? 'a partir da central' : `pelo agente ${resolving.hostName}` }}: nome na
      rede Docker, IP do contêiner e porta publicada…
      <v-progress-linear indeterminate color="info" class="mt-2"></v-progress-linear>
    </v-alert>
    <v-alert v-if="resolveError" type="error" variant="tonal" density="compact" class="mt-2">
      {{ resolveError }}
    </v-alert>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { DatabaseContainer } from '@/bindings/DatabaseContainer'
import type { DatabaseContainerResolution } from '@/bindings/DatabaseContainerResolution'
import type { DatabaseEngine } from '@/bindings/DatabaseEngine'
import { useDatabasesStore } from '@/stores/databases'
import { engineInfo } from '@/utils/databasePresentation'
import {
  containerKey,
  containersFor,
  containerSubtitle,
  filterContainers,
  protectedConnectionOf,
  type ContainerBackupFilter,
  type ProtectedContainerMatch,
} from '@/utils/databaseContainers'

/**
 * Atalho do "Nova conexão": os bancos que rodam em contêineres dos agentes.
 * Escolher um testa os endereços pela rota de verdade (no backend) e devolve
 * a resolução — quem preenche o formulário é o diálogo.
 */
const props = defineProps<{
  /** Só os deste SGBD; `null` mostra todos. */
  engine: DatabaseEngine | null
  /** `host/contêiner` já escolhido no formulário, para marcar na lista. */
  selected?: string | null
  /** Recolhe a lista automaticamente após escolher um contêiner no formulário. */
  collapseOnSelect?: boolean
}>()

const emit = defineEmits<{
  picked: [resolution: DatabaseContainerResolution]
  editConnection: [connectionId: number]
}>()

const store = useDatabasesStore()
const selectedKey = computed(() => props.selected ?? null)
const resolvingKey = ref<string | null>(null)
const resolveError = ref<string | null>(null)
const search = ref('')
const hostFilter = ref<string | null>(null)
const backupFilter = ref<ContainerBackupFilter>('all')
const showStopped = ref(false)
const collapsed = ref(false)

watch(selectedKey, (next) => {
  if (props.collapseOnSelect && next) {
    collapsed.value = true
  } else if (!next) {
    collapsed.value = false
  }
})

const resolving = computed(
  () =>
    store.containers.containers.find((item) => containerKey(item) === resolvingKey.value) ?? null
)
const ofEngine = computed(() => containersFor(store.containers.containers, props.engine))
const selectedItem = computed(
  () => ofEngine.value.find((item) => containerKey(item) === selectedKey.value) ?? null
)
const runningCount = computed(() => ofEngine.value.filter((item) => item.running).length)
const stoppedCount = computed(() => ofEngine.value.filter((item) => !item.running).length)

function protectedOf(item: DatabaseContainer): ProtectedContainerMatch | null {
  return protectedConnectionOf(item, store.connections)
}

const activeContainers = computed(() =>
  showStopped.value ? ofEngine.value : ofEngine.value.filter((item) => item.running)
)
const protectedCount = computed(
  () => activeContainers.value.filter((item) => protectedOf(item) != null).length
)
const unprotectedCount = computed(() => activeContainers.value.length - protectedCount.value)

const hostOptions = computed(() => {
  const byHost = new Map<string, { key: string; label: string; local: boolean; count: number }>()
  for (const item of activeContainers.value) {
    const existing = byHost.get(item.hostKey)
    if (existing) {
      existing.count += 1
    } else {
      byHost.set(item.hostKey, {
        key: item.hostKey,
        label: item.local ? 'Central' : item.hostName,
        local: item.local,
        count: 1,
      })
    }
  }
  return [...byHost.values()]
})

/** Parado não dá para testar nem copiar: fica escondido até o operador pedir. */
const items = computed(() =>
  filterContainers(
    store.containers.containers,
    {
      engine: props.engine,
      search: search.value,
      hostKey: hostFilter.value,
      backupFilter: backupFilter.value,
      showStopped: showStopped.value,
    },
    store.connections
  )
)

const selectOptions = computed(() =>
  items.value.map((container) => {
    const protectedMatch = protectedOf(container)
    const hostLabel = container.local ? 'Central' : container.hostName
    const suffix = protectedMatch ? ' · Já no backup' : ''
    return {
      value: containerKey(container),
      title: `${container.containerName} (${hostLabel})${suffix}`,
      container,
      protectedMatch,
    }
  })
)

const emptyText = computed(() => {
  if (search.value.trim() || hostFilter.value || backupFilter.value !== 'all') {
    return 'Nenhum contêiner corresponde à busca ou ao filtro selecionado.'
  }
  if (stoppedCount.value) {
    return 'Nenhum deles está rodando agora.'
  }
  return props.engine
    ? `Nenhum contêiner de ${engineInfo(props.engine).label} nos agentes conectados. Preencha o servidor abaixo.`
    : 'Nenhum banco em contêiner nos agentes conectados.'
})

function onSelectKey(key: string | null) {
  if (!key) return
  const chosen = ofEngine.value.find((item) => containerKey(item) === key)
  if (chosen) void pick(chosen)
}

async function pick(item: DatabaseContainer) {
  const key = containerKey(item)
  if (resolvingKey.value) return
  resolvingKey.value = key
  resolveError.value = null
  try {
    const resolution = await store.resolveContainer(item.hostKey, item.containerId)
    emit('picked', resolution)
    if (props.collapseOnSelect) collapsed.value = true
  } catch (err) {
    resolveError.value = err instanceof Error ? err.message : 'Não foi possível testar o contêiner'
  } finally {
    resolvingKey.value = null
  }
}
</script>

<style scoped>
.container-picker {
  background: rgb(var(--v-theme-surface));
}
.min-w-0 {
  min-width: 0;
}

/* Muitos bancos não empurram o resto do formulário para longe. */
.container-picker__list {
  max-height: 260px;
  overflow-y: auto;
}
</style>

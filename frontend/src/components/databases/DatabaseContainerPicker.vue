<template>
  <section class="container-picker rounded-lg border pa-3 mb-4" aria-label="Bancos em contêineres">
    <div class="d-flex align-center flex-wrap ga-2 mb-2">
      <v-icon color="primary">mdi-docker</v-icon>
      <div class="flex-grow-1">
        <div class="font-weight-bold text-body-2">Bancos em contêineres dos agentes</div>
        <div class="text-caption text-high-emphasis">
          Escolha um e o endereço é testado na hora: primeiro o nome na rede Docker, depois o IP do
          contêiner e a porta publicada no host.
        </div>
      </div>
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

    <v-progress-linear
      v-if="store.containersLoading && !items.length"
      indeterminate
      color="primary"
      class="my-2"
    ></v-progress-linear>

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
              size="small"
              color="primary"
              :variant="selectedKey === containerKey(item) ? 'flat' : 'outlined'"
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
import { computed, ref } from 'vue'
import type { DatabaseContainer } from '@/bindings/DatabaseContainer'
import type { DatabaseContainerResolution } from '@/bindings/DatabaseContainerResolution'
import type { DatabaseEngine } from '@/bindings/DatabaseEngine'
import { useDatabasesStore } from '@/stores/databases'
import { engineInfo } from '@/utils/databasePresentation'
import { containerKey, containersFor, containerSubtitle } from '@/utils/databaseContainers'

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
}>()

const emit = defineEmits<{ picked: [resolution: DatabaseContainerResolution] }>()

const store = useDatabasesStore()
const selectedKey = computed(() => props.selected ?? null)
const resolvingKey = ref<string | null>(null)
const resolveError = ref<string | null>(null)

const resolving = computed(
  () =>
    store.containers.containers.find((item) => containerKey(item) === resolvingKey.value) ?? null
)
const showStopped = ref(false)
const ofEngine = computed(() => containersFor(store.containers.containers, props.engine))
const stoppedCount = computed(() => ofEngine.value.filter((item) => !item.running).length)
/** Parado não dá para testar nem copiar: fica escondido até o operador pedir. */
const items = computed(() =>
  showStopped.value ? ofEngine.value : ofEngine.value.filter((item) => item.running)
)
const emptyText = computed(() =>
  stoppedCount.value
    ? 'Nenhum deles está rodando agora.'
    : props.engine
      ? `Nenhum contêiner de ${engineInfo(props.engine).label} nos agentes conectados. Preencha o servidor abaixo.`
      : 'Nenhum banco em contêiner nos agentes conectados.'
)

async function pick(item: DatabaseContainer) {
  const key = containerKey(item)
  if (resolvingKey.value) return
  resolvingKey.value = key
  resolveError.value = null
  try {
    const resolution = await store.resolveContainer(item.hostKey, item.containerId)
    emit('picked', resolution)
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

/* Muitos bancos não empurram o resto do formulário para longe. */
.container-picker__list {
  max-height: 340px;
  overflow-y: auto;
}
</style>

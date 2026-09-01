<template>
  <div>
    <PageHeader title="Redes Docker" subtitle="Topologia virtual e conexões entre containers">
      <template #actions>
        <v-btn
          v-if="auth.isAdmin"
          color="primary"
          prepend-icon="mdi-plus"
          @click="createDialog = true"
        >
          Nova rede
        </v-btn>
        <v-btn
          variant="tonal"
          prepend-icon="mdi-refresh"
          :loading="docker.loading"
          @click="docker.refreshAll()"
        >
          Atualizar
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="docker.error" type="error" variant="tonal" closable class="mb-4">
      {{ docker.error }}
    </v-alert>
    <v-card rounded="xl" variant="outlined">
      <ResponsiveDataTable
        :headers="headers"
        :items="docker.networks"
        :loading="docker.loading"
        no-data-text="Nenhuma rede Docker encontrada"
        clickable
        @click:row="onRowClick"
      >
        <template #item.name="{ item }">
          <div class="py-2">
            <div class="font-weight-bold">{{ item.name }}</div>
            <div class="text-caption text-medium-emphasis font-mono">{{ shortId(item.id) }}</div>
          </div>
        </template>
        <template #item.internal="{ item }">
          <v-chip :color="item.internal ? 'warning' : 'success'" size="small" variant="tonal">
            {{ item.internal ? 'Interna' : 'Externa' }}
          </v-chip>
        </template>
        <template #item.ipamConfig="{ item }">
          {{ formatSubnets(item) }}
        </template>
        <template #item.actions="{ item }">
          <div class="d-flex justify-end ga-1" @click.stop>
            <v-btn
              icon="mdi-eye-outline"
              size="small"
              variant="text"
              title="Inspecionar"
              @click="openDetail(item)"
            ></v-btn>
            <v-btn
              v-if="auth.isAdmin && !isBuiltIn(item.name)"
              icon="mdi-delete-outline"
              size="small"
              color="error"
              variant="text"
              title="Remover"
              :loading="docker.actionLoading"
              @click="removeNetwork(item)"
            ></v-btn>
          </div>
        </template>
        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-2">
            <div>
              <div class="font-weight-bold">{{ item.name }}</div>
              <div class="text-caption text-medium-emphasis">
                {{ item.driver }} · {{ item.scope }}
              </div>
              <div class="text-caption">{{ item.connectedContainers }} container(s)</div>
            </div>
            <v-icon color="primary">mdi-lan</v-icon>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <v-dialog v-model="createDialog" max-width="520">
      <v-card rounded="xl">
        <v-card-title>Nova rede Docker</v-card-title>
        <v-card-text>
          <v-text-field
            v-model="newNetwork.name"
            label="Nome"
            variant="outlined"
            :rules="[(value: string) => Boolean(value.trim()) || 'Informe um nome']"
          ></v-text-field>
          <v-select
            v-model="newNetwork.driver"
            label="Driver"
            :items="['bridge', 'overlay', 'macvlan', 'ipvlan']"
            variant="outlined"
            hide-details
          ></v-select>
        </v-card-text>
        <v-card-actions class="justify-end pa-4">
          <v-btn variant="text" @click="createDialog = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            :loading="docker.actionLoading"
            :disabled="!newNetwork.name.trim()"
            @click="createNetwork"
          >
            Criar rede
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog v-model="detailDialog" max-width="900" scrollable>
      <v-card rounded="xl">
        <v-card-title class="d-flex align-center ga-2">
          <v-icon color="primary">mdi-lan</v-icon>
          {{ detail?.name || 'Rede Docker' }}
          <v-spacer></v-spacer>
          <v-btn icon="mdi-close" variant="text" @click="detailDialog = false"></v-btn>
        </v-card-title>
        <v-divider></v-divider>
        <v-card-text class="pa-5">
          <v-skeleton-loader v-if="detailLoading" type="article"></v-skeleton-loader>
          <template v-else-if="detail">
            <v-row dense class="mb-3">
              <v-col cols="6" md="3">
                <strong>Driver</strong>
                <div>{{ detail.driver }}</div>
              </v-col>
              <v-col cols="6" md="3">
                <strong>Escopo</strong>
                <div>{{ detail.scope }}</div>
              </v-col>
              <v-col cols="6" md="3">
                <strong>IPAM</strong>
                <div>{{ detail.ipamDriver }}</div>
              </v-col>
              <v-col cols="6" md="3">
                <strong>Interna</strong>
                <div>{{ detail.internal ? 'Sim' : 'Não' }}</div>
              </v-col>
            </v-row>
            <v-alert
              v-for="config in detail.ipamConfig"
              :key="`${config.subnet}-${config.gateway}`"
              type="info"
              variant="tonal"
              density="compact"
              class="mb-2"
            >
              Sub-rede {{ config.subnet || '—' }} · gateway {{ config.gateway || '—' }}
            </v-alert>

            <div v-if="auth.isAdmin" class="d-flex flex-column flex-sm-row ga-2 my-4">
              <v-select
                v-model="containerToConnect"
                :items="connectableContainers"
                item-title="title"
                item-value="value"
                label="Conectar container"
                variant="outlined"
                density="compact"
                hide-details
              ></v-select>
              <v-btn
                color="primary"
                prepend-icon="mdi-lan-connect"
                :disabled="!containerToConnect"
                :loading="docker.actionLoading"
                @click="connectContainer"
              >
                Conectar
              </v-btn>
            </div>

            <v-table density="compact">
              <thead>
                <tr>
                  <th>Container</th>
                  <th>IPv4</th>
                  <th>MAC</th>
                  <th v-if="auth.isAdmin"></th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="container in detail.containers" :key="container.containerId">
                  <td>{{ container.name || shortId(container.containerId) }}</td>
                  <td>{{ container.ipv4Address || '—' }}</td>
                  <td>{{ container.macAddress || '—' }}</td>
                  <td v-if="auth.isAdmin" class="text-right">
                    <v-btn
                      icon="mdi-lan-disconnect"
                      size="small"
                      color="warning"
                      variant="text"
                      title="Desconectar"
                      @click="disconnectContainer(container.containerId)"
                    ></v-btn>
                  </td>
                </tr>
                <tr v-if="detail.containers.length === 0">
                  <td :colspan="auth.isAdmin ? 4 : 3" class="text-center text-medium-emphasis py-6">
                    Nenhum container conectado.
                  </td>
                </tr>
              </tbody>
            </v-table>
          </template>
        </v-card-text>
      </v-card>
    </v-dialog>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4500">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { confirm } from '@/composables/useConfirm'
import { dockerService } from '@/services/dockerService'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import type { DockerNetworkDetail } from '@/bindings/DockerNetworkDetail'
import type { DockerNetworkSummary } from '@/bindings/DockerNetworkSummary'

const docker = useDockerStore()
const auth = useAuthStore()
const createDialog = ref(false)
const detailDialog = ref(false)
const detailLoading = ref(false)
const detail = ref<DockerNetworkDetail | null>(null)
const selectedNetworkId = ref('')
const containerToConnect = ref<string | null>(null)
const newNetwork = reactive({ name: '', driver: 'bridge' })
const feedback = ref({ visible: false, color: 'success', message: '' })

const headers = [
  { title: 'Rede', key: 'name' },
  { title: 'Driver', key: 'driver', width: '120px' },
  { title: 'Escopo', key: 'scope', width: '110px' },
  { title: 'Acesso', key: 'internal', width: '110px' },
  { title: 'Sub-redes', key: 'ipamConfig' },
  { title: 'Containers', key: 'connectedContainers', width: '110px' },
  { title: 'Ações', key: 'actions', width: '110px', sortable: false },
]

const connectableContainers = computed(() => {
  const connected = new Set(
    detail.value?.containers.map((container) => container.containerId) ?? []
  )
  return docker.containers
    .filter((container) => !connected.has(container.id))
    .map((container) => ({
      title: container.names[0]?.replace(/^\//, '') || shortId(container.id),
      value: container.id,
    }))
})

function shortId(id: string): string {
  return id.slice(0, 12)
}

function isBuiltIn(name: string): boolean {
  return ['bridge', 'host', 'none'].includes(name)
}

function formatSubnets(network: DockerNetworkSummary): string {
  return (
    network.ipamConfig
      .map((config) => config.subnet)
      .filter(Boolean)
      .join(', ') || '—'
  )
}

function onRowClick(_event: MouseEvent, row: { item: DockerNetworkSummary }): void {
  void openDetail(row.item)
}

async function openDetail(network: DockerNetworkSummary): Promise<void> {
  detailDialog.value = true
  detailLoading.value = true
  selectedNetworkId.value = network.id
  containerToConnect.value = null
  try {
    detail.value = await dockerService.network(network.id)
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao inspecionar rede', 'error')
  } finally {
    detailLoading.value = false
  }
}

async function reloadDetail(): Promise<void> {
  detail.value = await dockerService.network(selectedNetworkId.value)
}

async function createNetwork(): Promise<void> {
  const success = await docker.runAction(() =>
    dockerService.createNetwork(newNetwork.name.trim(), newNetwork.driver)
  )
  if (success) {
    createDialog.value = false
    newNetwork.name = ''
  }
  notify(
    success ? 'Rede criada.' : docker.error || 'Erro ao criar rede',
    success ? 'success' : 'error'
  )
}

async function removeNetwork(network: DockerNetworkSummary): Promise<void> {
  const accepted = await confirm({
    title: 'Remover rede Docker',
    message: `Remover a rede "${network.name}"? Ela precisa estar sem containers conectados.`,
    confirmText: 'Remover rede',
    confirmColor: 'error',
    icon: 'mdi-lan-disconnect',
  })
  if (!accepted) return
  const success = await docker.runAction(() => dockerService.removeNetwork(network.id))
  notify(
    success ? 'Rede removida.' : docker.error || 'Erro ao remover rede',
    success ? 'success' : 'error'
  )
}

async function connectContainer(): Promise<void> {
  if (!containerToConnect.value) return
  const success = await docker.runAction(
    () => dockerService.connectNetwork(selectedNetworkId.value, containerToConnect.value as string),
    reloadDetail
  )
  if (success) containerToConnect.value = null
  notify(
    success ? 'Container conectado.' : docker.error || 'Erro ao conectar container',
    success ? 'success' : 'error'
  )
}

async function disconnectContainer(containerId: string): Promise<void> {
  const accepted = await confirm({
    title: 'Desconectar container',
    message: 'Desconectar este container da rede? A comunicação por essa rede será interrompida.',
    confirmText: 'Desconectar',
    confirmColor: 'warning',
    icon: 'mdi-lan-disconnect',
  })
  if (!accepted) return
  const success = await docker.runAction(
    () => dockerService.disconnectNetwork(selectedNetworkId.value, containerId),
    reloadDetail
  )
  notify(
    success ? 'Container desconectado.' : docker.error || 'Erro ao desconectar container',
    success ? 'success' : 'error'
  )
}

function notify(message: string, color: string): void {
  feedback.value = { visible: true, color, message }
}
</script>

<template>
  <div>
    <PageHeader
      title="Sub-redes (Networks)"
      subtitle="Faixas de IP CIDR e gatilhos de descoberta automática"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
          <span class="hidden-sm-and-down">Nova Rede</span>
          <span class="hidden-md-and-up">Nova</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Tabela de Sub-redes -->
    <v-card elevation="2" rounded="lg">
      <v-card-title class="pa-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar rede por CIDR ou nome"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="w-100"
          style="max-width: 420px"
        ></v-text-field>
      </v-card-title>

      <ResponsiveDataTable
        :headers="headers"
        :items="networksStore.networks"
        :search="search"
        :loading="networksStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhuma rede cadastrada"
        :clickable="false"
      >
        <template #item.site="{ item }">
          <v-chip v-if="item.site?.name || item.siteId" size="small" variant="tonal" color="info">
            {{ item.site?.name || `Site #${item.siteId}` }}
          </v-chip>
          <span v-else class="text-caption text-grey">Sem site</span>
        </template>

        <template #item.gateway="{ item }">
          <div v-if="item.gateway" class="d-flex align-center ga-1">
            <span class="font-mono text-body-2">{{ item.gateway }}</span>
            <template v-if="getGatewayDevice(item.gateway)">
              <router-link
                :to="'/devices/' + getGatewayDevice(item.gateway)?.id"
                class="text-decoration-none"
                @click.stop
              >
                <v-chip size="x-small" color="primary" variant="tonal" class="ml-1">
                  <v-icon start size="12">mdi-router-network</v-icon>
                  {{ getGatewayDevice(item.gateway)?.name }}
                </v-chip>
              </router-link>
            </template>
          </div>
          <span v-else class="text-caption text-grey">Não configurado</span>
        </template>

        <template #item.devicesCount="{ item }">
          <v-chip size="small" variant="text" class="font-weight-medium">
            <v-icon start size="14" color="primary">mdi-devices</v-icon>
            {{ getDeviceCountForNetwork(item.id) }} disp.
          </v-chip>
        </template>

        <template #item.cidr="{ item }">
          <div class="d-flex align-center ga-2">
            <span class="font-weight-medium font-mono">{{ item.cidr }}</span>
            <v-chip v-if="item.scannable" size="x-small" variant="tonal" color="grey-darken-1">
              {{ item.usableHosts }} host(s)
            </v-chip>
            <v-chip v-else size="x-small" variant="tonal" color="error">
              faixa inválida
              <v-tooltip activator="parent" location="top">
                Sem uma faixa CIDR válida (ex.: 192.168.1.0/24) não é possível varrer o bloco.
              </v-tooltip>
            </v-chip>
          </div>
        </template>

        <template #item.lastScanAt="{ item }">
          <span v-if="item.lastScanAt" class="text-body-2">
            {{ formatDateTime(item.lastScanAt) }}
          </span>
          <span v-else class="text-caption text-grey">Nunca varrida</span>
        </template>

        <template #item.actions="{ item }">
          <div class="d-flex align-center ga-1">
            <v-btn
              size="small"
              color="secondary"
              variant="flat"
              prepend-icon="mdi-radar"
              :disabled="item.scannable === false"
              @click="triggerScan(item)"
            >
              Escanear
            </v-btn>

            <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
              <v-icon>mdi-pencil</v-icon>
            </v-btn>
            <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
              <v-icon>mdi-delete</v-icon>
            </v-btn>
          </div>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex flex-column ga-2">
            <div class="d-flex align-start justify-space-between ga-2">
              <div class="flex-grow-1 text-break">
                <div
                  class="text-subtitle-2 font-weight-bold d-flex align-center justify-space-between"
                >
                  <span>{{ item.name }}</span>
                  <v-chip size="x-small" variant="tonal" color="primary">
                    {{ getDeviceCountForNetwork(item.id) }} dispositivo(s)
                  </v-chip>
                </div>
                <div class="d-flex flex-wrap align-center ga-2 mt-1">
                  <span class="text-body-2 font-weight-medium font-mono">{{ item.cidr }}</span>
                  <v-chip
                    v-if="item.scannable"
                    size="x-small"
                    variant="tonal"
                    color="grey-darken-1"
                  >
                    {{ item.usableHosts }} host(s)
                  </v-chip>
                  <v-chip v-else size="x-small" variant="tonal" color="error">
                    faixa inválida
                  </v-chip>
                </div>
                <div v-if="item.gateway" class="text-caption text-grey mt-1">
                  Gateway: <span class="font-mono">{{ item.gateway }}</span>
                  <span v-if="getGatewayDevice(item.gateway)">
                    ({{ getGatewayDevice(item.gateway)?.name }})
                  </span>
                </div>
                <div class="text-caption text-grey mt-1">
                  Site: {{ item.site?.name || (item.siteId ? `Site #${item.siteId}` : 'sem site') }}
                </div>
                <div class="text-caption text-grey">
                  <span v-if="item.lastScanAt">{{ formatDateTime(item.lastScanAt) }}</span>
                  <span v-else>Nunca varrida</span>
                </div>
              </div>
            </div>
            <div class="d-flex align-center ga-1 mt-1">
              <v-btn
                size="small"
                color="secondary"
                variant="flat"
                prepend-icon="mdi-radar"
                :disabled="item.scannable === false"
                @click="triggerScan(item)"
              >
                Escanear
              </v-btn>
              <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
                <v-icon>mdi-pencil</v-icon>
              </v-btn>
              <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
                <v-icon>mdi-delete</v-icon>
              </v-btn>
            </div>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <!-- Modal Form de Rede -->
    <v-dialog
      v-model="dialog"
      :max-width="$vuetify.display.xs ? undefined : 500"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">
          {{ editedId ? 'Editar Rede' : 'Cadastrar Nova Sub-rede' }}
        </v-card-title>
        <v-card-text>
          <v-form @submit.prevent="save">
            <v-row>
              <v-col cols="12">
                <div class="d-flex align-start ga-2">
                  <v-select
                    v-model="formModel.siteId"
                    :items="sitesStore.sites"
                    item-title="name"
                    item-value="id"
                    label="Site de Origem (opcional)"
                    placeholder="Sem site"
                    variant="outlined"
                    density="comfortable"
                    clearable
                    persistent-hint
                    hint="Deixe em branco para cadastrar a faixa sem vincular a um local."
                    class="flex-grow-1"
                  ></v-select>
                  <v-btn
                    icon="mdi-plus"
                    color="secondary"
                    variant="flat"
                    density="comfortable"
                    class="mt-1"
                    aria-label="Cadastrar novo site"
                    @click="siteDialog = true"
                  >
                    <v-icon>mdi-plus</v-icon>
                    <v-tooltip activator="parent" location="top">Cadastrar novo site</v-tooltip>
                  </v-btn>
                </div>
              </v-col>
              <v-col cols="12">
                <v-text-field
                  v-model="formModel.name"
                  label="Nome da Rede"
                  placeholder="Ex: LAN Matriz"
                  variant="outlined"
                  density="comfortable"
                  required
                ></v-text-field>
              </v-col>
              <v-col cols="12">
                <v-text-field
                  v-model="formModel.cidr"
                  label="Faixa CIDR *"
                  placeholder="192.168.1.0/24"
                  variant="outlined"
                  density="comfortable"
                  hint="Formato CIDR (ex: 192.168.1.0/24 ou 10.0.0.0/16)"
                  persistent-hint
                  required
                  @update:model-value="onCidrChanged"
                ></v-text-field>
              </v-col>
              <v-col cols="12">
                <v-text-field
                  v-model="formModel.gateway"
                  label="Gateway (IP do Roteador / Switch Core)"
                  placeholder="192.168.1.1"
                  variant="outlined"
                  density="comfortable"
                  prepend-inner-icon="mdi-router-network"
                  :append-inner-icon="canApplySuggestedGateway ? 'mdi-auto-fix' : undefined"
                  hint="IP do roteador principal que atua como uplink desta sub-rede."
                  persistent-hint
                  @click:append-inner="applySuggestedGateway"
                ></v-text-field>

                <!-- Sugestão Automática de Gateway -->
                <div v-if="canApplySuggestedGateway" class="mt-2">
                  <v-chip
                    size="small"
                    color="primary"
                    variant="tonal"
                    class="cursor-pointer"
                    prepend-icon="mdi-lightbulb-on-outline"
                    append-icon="mdi-arrow-right-circle"
                    @click="applySuggestedGateway"
                  >
                    Sugerido: <strong>{{ suggestedGateway }}</strong>
                    <v-tooltip activator="parent" location="bottom">
                      Clique para preencher o primeiro IP utilizável como gateway
                    </v-tooltip>
                  </v-chip>
                </div>

                <!-- Equipamento Físico Detectado para o Gateway -->
                <div v-if="modalMatchedGatewayDevice" class="mt-2">
                  <v-alert
                    type="info"
                    variant="tonal"
                    density="compact"
                    class="rounded-lg text-caption"
                  >
                    <div class="d-flex align-center ga-1">
                      <v-icon size="16">mdi-link-variant</v-icon>
                      <span>
                        Equipamento cadastrado para este IP:
                        <strong>{{ modalMatchedGatewayDevice.name }}</strong>
                        ({{ modalMatchedGatewayDevice.type }})
                      </span>
                    </div>
                  </v-alert>
                </div>
              </v-col>
            </v-row>
          </v-form>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="dialog = false">Cancelar</v-btn>
          <v-btn color="primary" :loading="saving" @click="save">Salvar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Cadastro de Site sem sair do formulário da rede -->
    <SiteDialog v-model="siteDialog" @saved="onSiteCreated" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useNetworksStore, type Network } from '@/stores/networks'
import { useSitesStore, type Site } from '@/stores/sites'
import { useDevicesStore, type Device } from '@/stores/devices'
import { formatDateTime } from '@/utils/formatters'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import SiteDialog from '@/components/SiteDialog.vue'
import { confirm } from '@/composables/useConfirm'

const router = useRouter()
const networksStore = useNetworksStore()
const sitesStore = useSitesStore()
const devicesStore = useDevicesStore()
const search = ref('')
const dialog = ref(false)
const siteDialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)

const formModel = reactive<{
  siteId: number | null
  name: string
  cidr: string
  gateway: string
}>({
  siteId: null,
  name: '',
  cidr: '',
  gateway: '',
})

const suggestedGateway = computed(() => {
  const cidr = formModel.cidr.trim()
  if (!cidr.includes('/') || !cidr.includes('.')) return ''
  const [ipPart] = cidr.split('/')
  const octets = ipPart.split('.')
  if (octets.length !== 4) return ''
  return `${octets[0]}.${octets[1]}.${octets[2]}.1`
})

const canApplySuggestedGateway = computed(() => {
  return Boolean(
    suggestedGateway.value &&
    formModel.gateway.trim() !== suggestedGateway.value &&
    !formModel.gateway.trim()
  )
})

function applySuggestedGateway() {
  if (suggestedGateway.value) {
    formModel.gateway = suggestedGateway.value
  }
}

function onCidrChanged() {
  if (canApplySuggestedGateway.value) {
    applySuggestedGateway()
  }
}

const modalMatchedGatewayDevice = computed(() => {
  const gw = formModel.gateway.trim()
  if (!gw) return null
  return devicesStore.devices.find((d) => d.ipAddress === gw) || null
})

function getGatewayDevice(gatewayIp?: string): Device | undefined {
  if (!gatewayIp) return undefined
  return devicesStore.devices.find((d) => d.ipAddress === gatewayIp)
}

function getDeviceCountForNetwork(networkId: number): number {
  return devicesStore.devices.filter((d) => d.networkId === networkId).length
}

const headers = [
  { title: 'ID', key: 'id', width: '70px' },
  { title: 'Nome da Rede', key: 'name' },
  { title: 'Faixa CIDR', key: 'cidr' },
  { title: 'Gateway', key: 'gateway' },
  { title: 'Dispositivos', key: 'devicesCount', width: '130px' },
  { title: 'Site', key: 'site' },
  { title: 'Última varredura', key: 'lastScanAt', width: '170px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
]

onMounted(async () => {
  await Promise.all([
    networksStore.fetchNetworks(),
    sitesStore.fetchSites(),
    devicesStore.fetchDevices(),
  ])
})

function openDialog(network?: Network) {
  if (network) {
    editedId.value = network.id
    formModel.siteId = network.siteId ?? null
    formModel.name = network.name
    formModel.cidr = network.cidr
    formModel.gateway = network.gateway || ''
  } else {
    editedId.value = null
    // Sem pré-seleção: o vínculo é opcional e escolher um site por conta
    // própria esconderia do operador que ele pode ficar sem nenhum.
    formModel.siteId = null
    formModel.name = ''
    formModel.cidr = ''
    formModel.gateway = ''
  }
  dialog.value = true
}

/** Site recém-criado no diálogo aninhado já entra selecionado. */
function onSiteCreated(site: Site) {
  formModel.siteId = site.id
}

async function save() {
  if (!formModel.name || !formModel.cidr) return
  saving.value = true
  if (editedId.value) {
    await networksStore.updateNetwork(editedId.value, formModel)
  } else {
    await networksStore.createNetwork(formModel)
  }
  saving.value = false
  dialog.value = false
}

/**
 * Varrer é acompanhar: o progresso, o log e os equipamentos achados só existem
 * em /discovery. Por isso o botão leva o operador para lá com o bloco já
 * escolhido (`networkId`) e a ordem de disparar na chegada (`scan=1`), em vez
 * de enfileirar a varredura numa tela que não mostra nada do que aconteceu.
 */
function triggerScan(network: Network) {
  router.push({ path: '/discovery', query: { networkId: String(network.id), scan: '1' } })
}

async function confirmDelete(id: number) {
  const ok = await confirm({
    title: 'Excluir rede',
    message:
      'Tem certeza de que deseja excluir esta rede? Dispositivos vinculados perderão a associação.',
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (ok) {
    await networksStore.deleteNetwork(id)
  }
}
</script>

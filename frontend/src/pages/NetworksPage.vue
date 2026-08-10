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
    <v-card elevation="2" class="mobile-full-bleed">
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

        <template #item.cidr="{ item }">
          <div class="d-flex align-center ga-2">
            <span class="font-weight-medium">{{ item.cidr }}</span>
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
              :loading="networksStore.scanningId === item.id"
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
                <div class="text-subtitle-2 font-weight-bold">{{ item.name }}</div>
                <div class="d-flex flex-wrap align-center ga-2 mt-1">
                  <span class="text-body-2 font-weight-medium">{{ item.cidr }}</span>
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
                :loading="networksStore.scanningId === item.id"
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
            <div class="d-flex align-start ga-2">
              <v-select
                v-model="formModel.siteId"
                :items="sitesStore.sites"
                item-title="name"
                item-value="id"
                label="Site de Origem (opcional)"
                placeholder="Sem site"
                variant="outlined"
                clearable
                persistent-hint
                hint="Deixe em branco para cadastrar a faixa sem vincular a um local."
                class="flex-grow-1"
              ></v-select>
              <v-btn
                icon="mdi-plus"
                color="secondary"
                variant="flat"
                class="mt-1"
                aria-label="Cadastrar novo site"
                @click="siteDialog = true"
              >
                <v-icon>mdi-plus</v-icon>
                <v-tooltip activator="parent" location="top">Cadastrar novo site</v-tooltip>
              </v-btn>
            </div>
            <v-text-field
              v-model="formModel.name"
              label="Nome da Rede"
              placeholder="Ex: LAN Matriz"
              variant="outlined"
              required
            ></v-text-field>
            <v-text-field
              v-model="formModel.cidr"
              label="Faixa CIDR"
              placeholder="192.168.1.0/24"
              variant="outlined"
              required
            ></v-text-field>
            <v-text-field
              v-model="formModel.gateway"
              label="Gateway (IP)"
              placeholder="192.168.1.1"
              variant="outlined"
            ></v-text-field>
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

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="7000">
      {{ feedback.message }}
      <template #actions>
        <v-btn variant="text" to="/discovery">Ver Descoberta</v-btn>
      </template>
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useNetworksStore, type Network } from '@/stores/networks'
import { useSitesStore, type Site } from '@/stores/sites'
import { formatDateTime } from '@/utils/formatters'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import SiteDialog from '@/components/SiteDialog.vue'

/** Espelha `MAX_SCAN_HOSTS` de `modules/discovery/cidr_range.ts` */
const MAX_SCAN_HOSTS = 1024

const networksStore = useNetworksStore()
const sitesStore = useSitesStore()
const search = ref('')
const dialog = ref(false)
const siteDialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)
const feedback = reactive({ visible: false, message: '', color: 'success' })

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

const headers = [
  { title: 'ID', key: 'id', width: '70px' },
  { title: 'Nome da Rede', key: 'name' },
  { title: 'Faixa CIDR', key: 'cidr' },
  { title: 'Gateway', key: 'gateway' },
  { title: 'Site', key: 'site' },
  { title: 'Última varredura', key: 'lastScanAt', width: '170px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
]

onMounted(async () => {
  await Promise.all([networksStore.fetchNetworks(), sitesStore.fetchSites()])
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
 * O botão não devolve equipamentos: ele enfileira a varredura, que roda no
 * scheduler. O feedback precisa dizer isso e apontar onde os achados aparecem —
 * senão a tela parece não ter feito nada.
 */
async function triggerScan(network: Network) {
  const result = await networksStore.scanNetwork(network.id)

  if (!result) {
    feedback.color = 'error'
    feedback.message = networksStore.error || 'Não foi possível iniciar a varredura.'
    feedback.visible = true
    return
  }

  const truncationNote = result.truncated
    ? ` A faixa tem ${result.usableHosts} endereços; apenas os primeiros ${MAX_SCAN_HOSTS} serão varridos.`
    : ''

  feedback.color = result.alreadyQueued ? 'warning' : 'success'
  feedback.message = `${result.message}${truncationNote}`
  feedback.visible = true

  // A coluna "Última varredura" muda quando o scheduler concluir
  await networksStore.fetchNetworks()
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir esta rede?')) {
    await networksStore.deleteNetwork(id)
  }
}
</script>

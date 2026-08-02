<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Sub-redes (Networks)</h1>
        <p class="text-subtitle-1 text-grey-darken-1">Faixas de IP CIDR e gatilhos de descoberta automática</p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
        Nova Rede
      </v-btn>
    </div>

    <!-- Tabela de Sub-redes -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar rede por CIDR ou nome"
          single-line
          hide-details
          variant="outlined"
          density="compact"
        ></v-text-field>
      </v-card-title>

      <v-data-table
        :headers="headers"
        :items="networksStore.networks"
        :search="search"
        :loading="networksStore.loading"
        no-data-text="Nenhuma rede cadastrada"
      >
        <template #item.site="{ item }">
          <v-chip size="small" variant="tonal" color="info">
            {{ item.site?.name || `Site #${item.siteId}` }}
          </v-chip>
        </template>

        <template #item.actions="{ item }">
          <v-btn
            size="small"
            color="secondary"
            variant="tonal"
            prepend-icon="mdi-radar"
            class="mr-2"
            :loading="networksStore.scanningId === item.id"
            @click="triggerScan(item.id)"
          >
            Escanear
          </v-btn>

          <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </template>
      </v-data-table>
    </v-card>

    <!-- Modal Form de Rede -->
    <v-dialog v-model="dialog" max-width="500">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">
          {{ editedId ? 'Editar Rede' : 'Cadastrar Nova Sub-rede' }}
        </v-card-title>
        <v-card-text>
          <v-form @submit.prevent="save">
            <v-select
              v-model="formModel.siteId"
              :items="sitesStore.sites"
              item-title="name"
              item-value="id"
              label="Site de Origem"
              variant="outlined"
              required
            ></v-select>
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useNetworksStore, type Network } from '@/stores/networks'
import { useSitesStore } from '@/stores/sites'

const networksStore = useNetworksStore()
const sitesStore = useSitesStore()
const search = ref('')
const dialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)

const formModel = reactive<{ siteId: number; name: string; cidr: string; gateway: string }>({
  siteId: 1,
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
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
]

onMounted(async () => {
  await Promise.all([networksStore.fetchNetworks(), sitesStore.fetchSites()])
})

function openDialog(network?: Network) {
  if (network) {
    editedId.value = network.id
    formModel.siteId = network.siteId
    formModel.name = network.name
    formModel.cidr = network.cidr
    formModel.gateway = network.gateway || ''
  } else {
    editedId.value = null
    formModel.siteId = sitesStore.sites[0]?.id || 1
    formModel.name = ''
    formModel.cidr = ''
    formModel.gateway = ''
  }
  dialog.value = true
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

async function triggerScan(id: number) {
  await networksStore.scanNetwork(id)
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir esta rede?')) {
    await networksStore.deleteNetwork(id)
  }
}
</script>

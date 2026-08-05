<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Sites (Locais)</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Gerenciamento de locais físicos e filiais monitoradas
        </p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()"> Novo Site </v-btn>
    </div>

    <!-- Tabela de Sites -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome ou localização"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="max-w-300"
        ></v-text-field>
      </v-card-title>

      <v-data-table
        :headers="headers"
        :items="sitesStore.sites"
        :search="search"
        :loading="sitesStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum site cadastrado"
        class="elevation-0"
      >
        <template #item.actions="{ item }">
          <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </template>
      </v-data-table>
    </v-card>

    <!-- Componente Reusável Dialog de Site -->
    <SiteDialog v-model="dialog" :site-to-edit="selectedSite" @saved="onSiteSaved" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSitesStore, type Site } from '@/stores/sites'
import SiteDialog from '@/components/SiteDialog.vue'

const sitesStore = useSitesStore()
const search = ref('')
const dialog = ref(false)
const selectedSite = ref<Site | null>(null)

const headers = [
  { title: 'ID', key: 'id', width: '80px' },
  { title: 'Nome do Site', key: 'name' },
  { title: 'Localização', key: 'location' },
  { title: 'Descrição', key: 'description' },
  { title: 'Ações', key: 'actions', sortable: false, width: '120px' },
]

onMounted(() => {
  sitesStore.fetchSites()
})

function openDialog(site?: Site) {
  selectedSite.value = site || null
  dialog.value = true
}

function onSiteSaved() {
  sitesStore.fetchSites()
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir este site?')) {
    await sitesStore.deleteSite(id)
  }
}
</script>

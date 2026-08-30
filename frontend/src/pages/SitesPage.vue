<template>
  <div>
    <PageHeader
      title="Sites (Locais)"
      subtitle="Gerenciamento de locais físicos e filiais monitoradas"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()">
          <span class="hidden-sm-and-down">Novo Site</span>
          <span class="hidden-md-and-up">Novo</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Tabela de Sites -->
    <v-card elevation="2" rounded="lg">
      <v-card-title class="pa-2.5 pa-sm-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome ou localização"
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
        :items="sitesStore.sites"
        :search="search"
        :loading="sitesStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum site cadastrado"
        :clickable="false"
      >
        <template #item.actions="{ item }">
          <div class="d-flex ga-1">
            <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
              <v-icon>mdi-pencil</v-icon>
            </v-btn>
            <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
              <v-icon>mdi-delete</v-icon>
            </v-btn>
          </div>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-2">
            <div class="flex-grow-1 text-break">
              <div class="text-subtitle-2 font-weight-bold">{{ item.name }}</div>
              <div class="text-body-2 text-grey-darken-1">{{ item.location || '—' }}</div>
              <div class="text-caption text-grey mt-1">{{ item.description || '—' }}</div>
            </div>
            <div class="d-flex ga-1">
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

    <!-- Componente Reusável Dialog de Site -->
    <SiteDialog v-model="dialog" :site-to-edit="selectedSite" @saved="onSiteSaved" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSitesStore, type Site } from '@/stores/sites'
import SiteDialog from '@/components/SiteDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { confirm } from '@/composables/useConfirm'

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
  const ok = await confirm({
    title: 'Excluir site',
    message:
      'Tem certeza de que deseja excluir este site? Redes e dispositivos vinculados perderão a associação.',
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (ok) {
    await sitesStore.deleteSite(id)
  }
}
</script>

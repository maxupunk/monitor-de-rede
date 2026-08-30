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
      <div class="pa-3 pa-sm-4">
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
      </div>

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
          <div class="d-flex flex-column ga-2">
            <!-- Top Row: Nome do Site -->
            <div class="d-flex align-center justify-space-between ga-2">
              <span class="text-subtitle-1 font-weight-bold text-truncate">{{ item.name }}</span>
              <span
                v-if="item.location"
                class="text-caption text-grey d-flex align-center ga-1 flex-shrink-0"
              >
                <v-icon size="13">mdi-map-marker-outline</v-icon>
                {{ item.location }}
              </span>
            </div>

            <!-- Middle: Descrição -->
            <div v-if="item.description" class="text-body-2 text-grey-darken-1 text-break">
              {{ item.description }}
            </div>

            <!-- Footer Actions -->
            <div class="d-flex align-center justify-end ga-1 pt-2 mt-1 border-t">
              <v-btn
                size="small"
                variant="tonal"
                color="primary"
                prepend-icon="mdi-pencil"
                class="text-caption px-2"
                @click="openDialog(item)"
              >
                Editar
              </v-btn>
              <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
                <v-icon size="18">mdi-delete</v-icon>
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

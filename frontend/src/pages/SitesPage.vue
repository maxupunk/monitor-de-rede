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

    <!-- Modal Form de Criação/Edição -->
    <v-dialog v-model="dialog" max-width="500">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">
          {{ editedId ? 'Editar Site' : 'Cadastrar Novo Site' }}
        </v-card-title>
        <v-card-text>
          <v-form ref="form" @submit.prevent="save">
            <v-text-field
              v-model="formModel.name"
              label="Nome do Local"
              variant="outlined"
              required
            ></v-text-field>
            <v-text-field
              v-model="formModel.location"
              label="Localização / Cidade / UF"
              variant="outlined"
            ></v-text-field>
            <v-textarea
              v-model="formModel.description"
              label="Descrição"
              variant="outlined"
              rows="3"
            ></v-textarea>
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
import { useSitesStore, type Site } from '@/stores/sites'

const sitesStore = useSitesStore()
const search = ref('')
const dialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)

const formModel = reactive<{ name: string; location: string; description: string }>({
  name: '',
  location: '',
  description: '',
})

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
  if (site) {
    editedId.value = site.id
    formModel.name = site.name
    formModel.location = site.location || ''
    formModel.description = site.description || ''
  } else {
    editedId.value = null
    formModel.name = ''
    formModel.location = ''
    formModel.description = ''
  }
  dialog.value = true
}

async function save() {
  if (!formModel.name) return
  saving.value = true
  if (editedId.value) {
    await sitesStore.updateSite(editedId.value, formModel)
  } else {
    await sitesStore.createSite(formModel)
  }
  saving.value = false
  dialog.value = false
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir este site?')) {
    await sitesStore.deleteSite(id)
  }
}
</script>

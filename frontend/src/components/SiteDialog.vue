<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 500"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg pa-4">
      <v-card-title class="font-weight-bold">
        {{ siteToEdit ? 'Editar Site' : 'Cadastrar Novo Site' }}
      </v-card-title>
      <v-card-text>
        <v-form @submit.prevent="save">
          <v-row>
            <v-col cols="12">
              <v-text-field
                v-model="formModel.name"
                label="Nome do Local *"
                variant="outlined"
                density="comfortable"
                required
              ></v-text-field>
            </v-col>
            <v-col cols="12">
              <v-text-field
                v-model="formModel.location"
                label="Localização / Cidade / UF"
                variant="outlined"
                density="comfortable"
              ></v-text-field>
            </v-col>
            <v-col cols="12">
              <v-textarea
                v-model="formModel.description"
                label="Descrição"
                variant="outlined"
                density="comfortable"
                rows="3"
              ></v-textarea>
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>
      <v-card-actions class="justify-end">
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-btn color="primary" :loading="saving" @click="save">Salvar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import { useSitesStore, type Site } from '@/stores/sites'

const props = defineProps<{
  modelValue: boolean
  siteToEdit?: Site | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', site: Site): void
}>()

const sitesStore = useSitesStore()
const saving = ref(false)

const formModel = reactive<{
  name: string
  location: string
  description: string
}>({
  name: '',
  location: '',
  description: '',
})

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) {
      if (props.siteToEdit) {
        formModel.name = props.siteToEdit.name || ''
        formModel.location = props.siteToEdit.location || ''
        formModel.description = props.siteToEdit.description || ''
      } else {
        formModel.name = ''
        formModel.location = ''
        formModel.description = ''
      }
    }
  }
)

function onUpdateModelValue(val: boolean) {
  emit('update:modelValue', val)
}

function close() {
  emit('update:modelValue', false)
}

async function save() {
  if (!formModel.name.trim()) return
  saving.value = true
  try {
    if (props.siteToEdit && props.siteToEdit.id) {
      const success = await sitesStore.updateSite(props.siteToEdit.id, formModel)
      if (success) {
        const updated = sitesStore.sites.find((s) => s.id === props.siteToEdit?.id)
        if (updated) emit('saved', updated)
        close()
      }
    } else {
      const success = await sitesStore.createSite(formModel)
      if (success) {
        // Newly created site will be the last in store
        const created = sitesStore.sites[sitesStore.sites.length - 1]
        if (created) emit('saved', created)
        close()
      }
    }
  } finally {
    saving.value = false
  }
}
</script>

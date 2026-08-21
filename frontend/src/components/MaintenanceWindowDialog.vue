<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 560"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg pa-4">
      <v-card-title class="font-weight-bold">
        {{ windowToEdit ? 'Editar Janela de Manutenção' : 'Nova Janela de Manutenção' }}
      </v-card-title>
      <v-card-subtitle class="pb-2">
        Alertas e notificações do site ou dispositivo escolhido serão suprimidos durante este
        intervalo.
      </v-card-subtitle>

      <v-card-text>
        <v-form ref="formRef" @submit.prevent="save">
          <v-text-field
            v-model="form.name"
            label="Nome *"
            placeholder="Ex.: Manutenção preventiva no link principal"
            variant="outlined"
            :rules="[(v: string) => !!v.trim() || 'Informe um nome']"
            class="mb-4"
          />

          <v-textarea
            v-model="form.description"
            label="Descrição"
            placeholder="Motivo, protocolo ou chamado relacionado"
            variant="outlined"
            rows="2"
            class="mb-4"
          />

          <v-btn-toggle v-model="scopeMode" color="primary" variant="outlined" divided class="mb-4">
            <v-btn value="site">Site</v-btn>
            <v-btn value="device">Dispositivo</v-btn>
          </v-btn-toggle>

          <v-select
            v-if="scopeMode === 'site'"
            v-model="form.siteId"
            :items="sitesStore.sites"
            item-title="name"
            item-value="id"
            label="Site *"
            placeholder="Selecione o site"
            variant="outlined"
            :rules="[(v: number | null) => v != null || 'Selecione um site']"
            class="mb-4"
          />

          <v-select
            v-else
            v-model="form.deviceId"
            :items="devicesStore.devices"
            item-title="name"
            item-value="id"
            label="Dispositivo *"
            placeholder="Selecione o dispositivo"
            variant="outlined"
            :rules="[(v: number | null) => v != null || 'Selecione um dispositivo']"
            class="mb-4"
          />

          <v-row dense>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="form.startsAt"
                label="Início *"
                type="datetime-local"
                variant="outlined"
                :rules="[(v: string) => !!v || 'Informe o início']"
              />
            </v-col>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="form.endsAt"
                label="Término *"
                type="datetime-local"
                variant="outlined"
                :rules="[
                  (v: string) => !!v || 'Informe o término',
                  (v: string) =>
                    !form.startsAt ||
                    new Date(v) > new Date(form.startsAt) ||
                    'O término deve ser posterior ao início',
                ]"
              />
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>

      <v-card-actions class="justify-end">
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-btn color="primary" :loading="saving" @click="save">
          {{ windowToEdit ? 'Salvar' : 'Criar' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { useSitesStore } from '@/stores/sites'
import { useDevicesStore } from '@/stores/devices'
import { useMaintenanceWindowsStore, type MaintenanceWindow } from '@/stores/maintenanceWindows'

const props = defineProps<{
  modelValue: boolean
  windowToEdit?: MaintenanceWindow | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved'): void
}>()

const sitesStore = useSitesStore()
const devicesStore = useDevicesStore()
const windowsStore = useMaintenanceWindowsStore()

const formRef = ref()
const saving = ref(false)
const scopeMode = ref<'site' | 'device'>('site')

const form = reactive({
  name: '',
  description: '',
  siteId: null as number | null,
  deviceId: null as number | null,
  startsAt: '',
  endsAt: '',
})

function toLocalInput(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`
}

function fromLocalInput(local: string): string {
  return new Date(local).toISOString()
}

function resetForm() {
  form.name = ''
  form.description = ''
  form.siteId = null
  form.deviceId = null
  form.startsAt = ''
  form.endsAt = ''
  scopeMode.value = 'site'
}

function fillForm() {
  const w = props.windowToEdit
  if (!w) {
    resetForm()
    return
  }
  form.name = w.name
  form.description = w.description ?? ''
  form.siteId = w.siteId ?? null
  form.deviceId = w.deviceId ?? null
  scopeMode.value = w.deviceId != null ? 'device' : 'site'
  form.startsAt = toLocalInput(w.startsAt)
  form.endsAt = toLocalInput(w.endsAt)
}

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    fillForm()
    if (sitesStore.sites.length === 0) void sitesStore.fetchSites()
    if (devicesStore.devices.length === 0) void devicesStore.fetchDevices()
  }
)

function close() {
  emit('update:modelValue', false)
}

async function save() {
  const validation = await formRef.value?.validate()
  if (validation && validation.valid === false) return

  const payload = {
    name: form.name.trim(),
    description: form.description.trim() || null,
    siteId: scopeMode.value === 'site' ? form.siteId : null,
    deviceId: scopeMode.value === 'device' ? form.deviceId : null,
    startsAt: fromLocalInput(form.startsAt),
    endsAt: fromLocalInput(form.endsAt),
  }

  saving.value = true
  const ok = props.windowToEdit
    ? await windowsStore.updateWindow(props.windowToEdit.id, payload)
    : await windowsStore.createWindow(payload)
  saving.value = false

  if (!ok) return
  emit('saved')
  close()
}
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 650"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg pa-4">
      <v-card-title class="font-weight-bold">
        {{ deviceToEdit ? 'Editar Dispositivo' : 'Cadastrar Novo Dispositivo' }}
      </v-card-title>
      <v-card-text>
        <v-form @submit.prevent="save">
          <v-row>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="formModel.name"
                label="Nome do Equipamento *"
                variant="outlined"
                density="comfortable"
                required
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="formModel.ipAddress"
                label="Endereço IP *"
                placeholder="Ex: 192.168.1.1"
                variant="outlined"
                density="comfortable"
                required
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6">
              <v-select
                v-model="formModel.type"
                :items="['router', 'switch', 'server', 'firewall', 'printer', 'ap', 'other']"
                label="Tipo de Dispositivo"
                variant="outlined"
                density="comfortable"
                required
              ></v-select>
            </v-col>

            <!-- Seleção de Site Opcional com Botão para Novo Site -->
            <v-col cols="12" sm="6">
              <div class="d-flex align-center ga-2">
                <v-select
                  v-model="formModel.siteId"
                  :items="sitesStore.sites"
                  item-title="name"
                  item-value="id"
                  label="Site (Opcional)"
                  variant="outlined"
                  density="comfortable"
                  clearable
                  hide-details
                  class="flex-grow-1"
                ></v-select>
                <v-btn
                  icon
                  color="primary"
                  variant="tonal"
                  density="comfortable"
                  @click="siteDialog = true"
                >
                  <v-icon>mdi-plus</v-icon>
                  <v-tooltip activator="parent" location="top">Cadastrar Novo Site</v-tooltip>
                </v-btn>
              </div>
            </v-col>

            <!-- Campo "Está atrás de" para Topologia -->
            <v-col cols="12" sm="6">
              <v-select
                v-model="formModel.parentId"
                :items="availableParentDevices"
                item-title="name"
                item-value="id"
                label="Está atrás de (Dispositivo Pai)"
                variant="outlined"
                density="comfortable"
                clearable
                hint="Indica a qual equipamento (ex: Switch/Roteador) este dispositivo está conectado."
                persistent-hint
              >
                <template #append-inner>
                  <v-icon size="small" color="grey-darken-1">mdi-help-circle-outline</v-icon>
                  <v-tooltip activator="parent" location="top">
                    Esta associação mapeia o caminho físico da rede para montar a estrutura de
                    topologia.
                  </v-tooltip>
                </template>
              </v-select>
            </v-col>

            <v-col cols="12" sm="6">
              <v-text-field
                v-model="formModel.vendor"
                label="Fabricante / Vendor"
                placeholder="Cisco, MikroTik, Ubiquiti"
                variant="outlined"
                density="comfortable"
              ></v-text-field>
            </v-col>

            <v-col cols="12" sm="6">
              <v-text-field
                v-model="formModel.model"
                label="Modelo"
                variant="outlined"
                density="comfortable"
              ></v-text-field>
            </v-col>

            <!-- Opção de Monitorar -->
            <v-col cols="12">
              <v-checkbox
                v-model="formModel.isMonitored"
                label="Monitorar este dispositivo (Disponível em /monitors)"
                color="primary"
                hide-details
              ></v-checkbox>
            </v-col>

            <v-col cols="12">
              <v-checkbox
                v-model="formModel.snmpEnabled"
                label="Habilitar Coleta SNMP"
                color="primary"
                hide-details
              ></v-checkbox>
            </v-col>
            <v-col v-if="formModel.snmpEnabled" cols="12" sm="6">
              <v-text-field
                v-model="formModel.snmpCommunity"
                label="Comunidade SNMP"
                variant="outlined"
                density="comfortable"
              ></v-text-field>
            </v-col>
            <v-col v-if="formModel.snmpEnabled" cols="12" sm="6">
              <v-select
                v-model="formModel.snmpVersion"
                :items="['v1', 'v2c', 'v3']"
                label="Versão SNMP"
                variant="outlined"
                density="comfortable"
              ></v-select>
            </v-col>
            <v-col v-if="formModel.snmpEnabled" cols="12">
              <div class="d-flex align-center flex-wrap ga-2">
                <v-btn
                  variant="tonal"
                  color="primary"
                  size="small"
                  prepend-icon="mdi-lan-check"
                  :loading="snmpTestStore.testing"
                  :disabled="!formModel.ipAddress"
                  @click="testSnmp(false)"
                >
                  Testar SNMP
                </v-btn>
                <v-btn
                  variant="text"
                  color="primary"
                  size="small"
                  prepend-icon="mdi-auto-fix"
                  :loading="snmpTestStore.testing"
                  :disabled="!formModel.ipAddress"
                  @click="testSnmp(true)"
                >
                  Detectar Automaticamente
                </v-btn>
              </div>
              <v-alert
                v-if="snmpTestResult"
                :type="snmpTestResult.ok ? 'success' : 'warning'"
                variant="tonal"
                density="compact"
                class="mt-2"
              >
                {{ snmpTestResult.message }}
              </v-alert>
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>
      <v-card-actions class="justify-end">
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-btn color="primary" :loading="saving" @click="save">Salvar</v-btn>
      </v-card-actions>
    </v-card>

    <SiteDialog v-model="siteDialog" @saved="onSiteCreated" />
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch } from 'vue'
import { useDevicesStore, type Device } from '@/stores/devices'
import { useSitesStore, type Site } from '@/stores/sites'
import { useSnmpTestStore } from '@/stores/snmpTest'
import SiteDialog from '@/components/SiteDialog.vue'

const props = defineProps<{
  modelValue: boolean
  deviceToEdit?: Device | null
  prefillData?: Partial<Device> | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', device: Device): void
}>()

const devicesStore = useDevicesStore()
const sitesStore = useSitesStore()
const snmpTestStore = useSnmpTestStore()

const siteDialog = ref(false)
const saving = ref(false)
const snmpTestResult = ref<{ ok: boolean; message: string } | null>(null)

const formModel = reactive<{
  name: string
  ipAddress: string
  type: string
  siteId: number | null
  parentId: number | null
  vendor: string
  model: string
  isMonitored: boolean
  snmpEnabled: boolean
  snmpCommunity: string
  snmpVersion: 'v1' | 'v2c' | 'v3'
}>({
  name: '',
  ipAddress: '',
  type: 'router',
  siteId: null,
  parentId: null,
  vendor: '',
  model: '',
  isMonitored: true,
  snmpEnabled: false,
  snmpCommunity: 'public',
  snmpVersion: 'v2c',
})

const availableParentDevices = computed(() => {
  return devicesStore.devices.filter((d) => d.id !== props.deviceToEdit?.id)
})

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) {
      if (devicesStore.devices.length === 0) devicesStore.fetchDevices()
      if (sitesStore.sites.length === 0) sitesStore.fetchSites()

      snmpTestResult.value = null
      if (props.deviceToEdit) {
        formModel.name = props.deviceToEdit.name || ''
        formModel.ipAddress = props.deviceToEdit.ipAddress || ''
        formModel.type = props.deviceToEdit.type || 'router'
        formModel.siteId = props.deviceToEdit.siteId ?? null
        formModel.parentId = props.deviceToEdit.parentId ?? null
        formModel.vendor = props.deviceToEdit.vendor || ''
        formModel.model = props.deviceToEdit.model || ''
        formModel.isMonitored = Boolean(props.deviceToEdit.isMonitored)
        formModel.snmpEnabled = Boolean(props.deviceToEdit.snmpEnabled)
        formModel.snmpCommunity = props.deviceToEdit.snmpCommunity || 'public'
        formModel.snmpVersion = props.deviceToEdit.snmpVersion || 'v2c'
      } else if (props.prefillData) {
        formModel.name = props.prefillData.name || ''
        formModel.ipAddress = props.prefillData.ipAddress || ''
        formModel.type = (props.prefillData.type as string) || 'other'
        formModel.siteId = props.prefillData.siteId ?? null
        formModel.parentId = props.prefillData.parentId ?? null
        formModel.vendor = props.prefillData.vendor || ''
        formModel.model = props.prefillData.model || ''
        formModel.isMonitored = props.prefillData.isMonitored ?? true
        formModel.snmpEnabled = props.prefillData.snmpEnabled ?? false
        formModel.snmpCommunity = props.prefillData.snmpCommunity || 'public'
        formModel.snmpVersion = props.prefillData.snmpVersion || 'v2c'
      } else {
        formModel.name = ''
        formModel.ipAddress = ''
        formModel.type = 'router'
        formModel.siteId = null
        formModel.parentId = null
        formModel.vendor = ''
        formModel.model = ''
        formModel.isMonitored = true
        formModel.snmpEnabled = false
        formModel.snmpCommunity = 'public'
        formModel.snmpVersion = 'v2c'
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

function onSiteCreated(newSite: Site) {
  formModel.siteId = newSite.id
}

async function testSnmp(autoDetect = false) {
  snmpTestResult.value = null
  if (!formModel.ipAddress) {
    snmpTestResult.value = { ok: false, message: 'Informe o Endereço IP antes de testar.' }
    return
  }

  const res = await snmpTestStore.testConnection({
    host: formModel.ipAddress,
    version: formModel.snmpVersion,
    community: formModel.snmpCommunity,
    autoDetect,
  })

  if (!res) {
    snmpTestResult.value = {
      ok: false,
      message: snmpTestStore.error || 'Falha ao testar conexão SNMP.',
    }
    return
  }

  if (res.responded) {
    if (autoDetect && res.version) formModel.snmpVersion = res.version
    if (autoDetect && res.community) formModel.snmpCommunity = res.community
    snmpTestResult.value = {
      ok: true,
      message: `SNMP respondeu (${res.version || formModel.snmpVersion}/${res.community || formModel.snmpCommunity}): ${res.sysDescr || res.sysName || 'dispositivo detectado'}`,
    }
  } else {
    // O backend sabe se o agente calou ou se recusou a credencial; só quando
    // ele não diz nada é que cabe o texto genérico.
    snmpTestResult.value = {
      ok: false,
      message:
        res.message ||
        (autoDetect
          ? 'Nenhuma combinação comum de versão/comunidade respondeu (public/private em v1/v2c).'
          : 'O dispositivo não respondeu com essa versão/comunidade em SNMP.'),
    }
  }
}

async function save() {
  if (!formModel.name || !formModel.ipAddress) return
  saving.value = true
  try {
    if (props.deviceToEdit && props.deviceToEdit.id) {
      const success = await devicesStore.updateDevice(props.deviceToEdit.id, formModel)
      if (success) {
        await devicesStore.fetchDevices()
        const updated = devicesStore.devices.find((d) => d.id === props.deviceToEdit?.id)
        if (updated) emit('saved', updated)
        close()
      }
    } else {
      const success = await devicesStore.createDevice({
        ...formModel,
        status: 'unknown' as const,
      })
      if (success) {
        await devicesStore.fetchDevices()
        const created = devicesStore.devices[devicesStore.devices.length - 1]
        if (created) emit('saved', created)
        close()
      }
    }
  } finally {
    saving.value = false
  }
}
</script>

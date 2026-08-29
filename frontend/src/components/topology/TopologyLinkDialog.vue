<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 640"
    :fullscreen="$vuetify.display.xs"
    persistent
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card class="rounded-xl overflow-hidden elevation-12">
      <!-- Header com gradiente moderno -->
      <v-card-item class="bg-primary text-white py-4 px-6">
        <div class="d-flex align-center justify-space-between w-100">
          <div class="d-flex align-center">
            <v-avatar color="white" variant="flat" size="38" class="mr-3 text-primary">
              <v-icon size="24">
                {{ isEditMode ? 'mdi-pencil-ruler' : 'mdi-vector-polyline-plus' }}
              </v-icon>
            </v-avatar>
            <div>
              <v-card-title class="text-h6 font-weight-bold pa-0 text-white">
                {{ isEditMode ? 'Editar Conexão de Topologia' : 'Adicionar Conexão de Topologia' }}
              </v-card-title>
              <div class="text-caption text-white opacity-80">
                {{
                  isEditMode
                    ? 'Altere as portas de saída/chegada e a tecnologia do enlace'
                    : 'Ligue dois equipamentos mapeando as portas de saída e chegada'
                }}
              </div>
            </div>
          </div>
          <v-btn
            icon="mdi-close"
            variant="text"
            density="comfortable"
            color="white"
            @click="close"
          ></v-btn>
        </div>
      </v-card-item>

      <v-card-text class="pa-6">
        <!-- Visual Connection Preview Banner -->
        <div class="connection-preview-card pa-4 rounded-lg mb-6 elevation-1">
          <div class="text-caption font-weight-bold text-medium-emphasis mb-2 text-uppercase">
            Prévia da Interconexão
          </div>
          <div class="d-flex align-center justify-space-between flex-wrap gap-2">
            <!-- Source Node Box -->
            <div class="preview-box pa-2 rounded text-center flex-grow-1">
              <div class="font-weight-bold text-body-2 text-truncate">
                {{ selectedSourceNode?.name || 'Origem não selecionada' }}
              </div>
              <v-chip
                size="x-small"
                :color="selectedSourceInterface ? 'primary' : 'default'"
                class="mt-1 font-weight-medium"
              >
                {{ selectedSourceInterface?.name || 'Porta Automática/Padrão' }}
              </v-chip>
            </div>

            <!-- Cable Indicator -->
            <div class="d-flex flex-column align-center px-3">
              <v-icon :color="getLinkColor(form.linkType)" size="28" class="cable-pulse-icon">
                {{ getLinkTypeIcon(form.linkType) }}
              </v-icon>
              <span class="text-caption font-weight-bold mt-1 text-primary">
                {{ getLinkTypeLabel(form.linkType) }}
              </span>
            </div>

            <!-- Target Node Box -->
            <div class="preview-box pa-2 rounded text-center flex-grow-1">
              <div class="font-weight-bold text-body-2 text-truncate">
                {{ selectedTargetNode?.name || 'Destino não selecionado' }}
              </div>
              <v-chip
                size="x-small"
                :color="selectedTargetInterface ? 'success' : 'default'"
                class="mt-1 font-weight-medium"
              >
                {{ selectedTargetInterface?.name || 'Porta Automática/Padrão' }}
              </v-chip>
            </div>
          </div>
        </div>

        <v-form ref="formRef" @submit.prevent="save">
          <v-row dense>
            <!-- Lado Esquerdo: Origem -->
            <v-col cols="12" md="6">
              <v-card variant="outlined" class="pa-3 rounded-lg fill-height border-primary-subtle">
                <div class="d-flex align-center mb-3">
                  <v-avatar color="primary" size="28" class="mr-2 text-white">
                    <span class="text-caption font-weight-bold">A</span>
                  </v-avatar>
                  <span class="font-weight-bold text-subtitle-2">Dispositivo de Origem</span>
                </div>

                <v-select
                  v-model="form.sourceDeviceId"
                  :items="deviceOptions"
                  item-title="name"
                  item-value="id"
                  label="Equipamento de Origem *"
                  variant="outlined"
                  density="comfortable"
                  :rules="[rules.required]"
                  :disabled="isEditMode"
                  prepend-inner-icon="mdi-server-network"
                  @update:model-value="onSourceDeviceChanged"
                >
                  <template #item="{ props: itemProps, item }">
                    <v-list-item v-bind="itemProps" :subtitle="item.ipAddress || item.type">
                      <template #prepend>
                        <v-icon :color="item.status === 'online' ? 'success' : 'grey'">
                          {{ getNodeIcon(item.type) }}
                        </v-icon>
                      </template>
                    </v-list-item>
                  </template>
                </v-select>

                <!-- Interface de Saída (Origem) -->
                <div class="mt-2">
                  <div class="d-flex align-center justify-space-between mb-1">
                    <span class="text-caption font-weight-bold text-medium-emphasis">
                      Interface de Saída (Porta)
                    </span>
                    <v-progress-circular
                      v-if="loadingSourceInterfaces"
                      indeterminate
                      size="14"
                      width="2"
                      color="primary"
                    ></v-progress-circular>
                  </div>

                  <v-select
                    v-model="form.sourceInterfaceId"
                    :items="sourceInterfaceOptions"
                    item-title="name"
                    item-value="id"
                    label="Selecionar porta de saída (SNMP/Física)"
                    variant="outlined"
                    density="comfortable"
                    clearable
                    prepend-inner-icon="mdi-ethernet"
                    :disabled="!form.sourceDeviceId || loadingSourceInterfaces"
                    :hint="
                      sourceInterfaceOptions.length === 0 && form.sourceDeviceId
                        ? 'Nenhuma interface SNMP cadastrada. O link será conectado diretamente ao dispositivo.'
                        : 'Identifica por qual porta física/lógica o sinal sai'
                    "
                    persistent-hint
                  >
                    <template #item="{ props: itemProps, item }">
                      <v-list-item
                        v-bind="itemProps"
                        :title="item.name"
                        :subtitle="formatInterfaceSubtitle(item)"
                      >
                        <template #append>
                          <v-chip
                            size="x-small"
                            :color="item.operStatus === 'up' ? 'success' : 'grey'"
                            variant="tonal"
                          >
                            {{ item.operStatus || 'up' }}
                          </v-chip>
                        </template>
                      </v-list-item>
                    </template>
                  </v-select>
                </div>
              </v-card>
            </v-col>

            <!-- Lado Direito: Destino -->
            <v-col cols="12" md="6">
              <v-card variant="outlined" class="pa-3 rounded-lg fill-height border-success-subtle">
                <div class="d-flex align-center mb-3">
                  <v-avatar color="success" size="28" class="mr-2 text-white">
                    <span class="text-caption font-weight-bold">B</span>
                  </v-avatar>
                  <span class="font-weight-bold text-subtitle-2">Dispositivo de Destino</span>
                </div>

                <v-select
                  v-model="form.targetDeviceId"
                  :items="targetDeviceOptions"
                  item-title="name"
                  item-value="id"
                  label="Equipamento de Destino *"
                  variant="outlined"
                  density="comfortable"
                  :rules="[rules.required, rules.differentDevices]"
                  :disabled="isEditMode"
                  prepend-inner-icon="mdi-server-network"
                  @update:model-value="onTargetDeviceChanged"
                >
                  <template #item="{ props: itemProps, item }">
                    <v-list-item v-bind="itemProps" :subtitle="item.ipAddress || item.type">
                      <template #prepend>
                        <v-icon :color="item.status === 'online' ? 'success' : 'grey'">
                          {{ getNodeIcon(item.type) }}
                        </v-icon>
                      </template>
                    </v-list-item>
                  </template>
                </v-select>

                <!-- Interface de Chegada (Destino) -->
                <div class="mt-2">
                  <div class="d-flex align-center justify-space-between mb-1">
                    <span class="text-caption font-weight-bold text-medium-emphasis">
                      Interface de Chegada (Porta)
                    </span>
                    <v-progress-circular
                      v-if="loadingTargetInterfaces"
                      indeterminate
                      size="14"
                      width="2"
                      color="success"
                    ></v-progress-circular>
                  </div>

                  <v-select
                    v-model="form.targetInterfaceId"
                    :items="targetInterfaceOptions"
                    item-title="name"
                    item-value="id"
                    label="Selecionar porta de entrada (SNMP/Física)"
                    variant="outlined"
                    density="comfortable"
                    clearable
                    prepend-inner-icon="mdi-ethernet"
                    :disabled="!form.targetDeviceId || loadingTargetInterfaces"
                    :hint="
                      targetInterfaceOptions.length === 0 && form.targetDeviceId
                        ? 'Nenhuma interface SNMP cadastrada. O link será conectado diretamente ao dispositivo.'
                        : 'Identifica em qual porta física/lógica o sinal chega'
                    "
                    persistent-hint
                  >
                    <template #item="{ props: itemProps, item }">
                      <v-list-item
                        v-bind="itemProps"
                        :title="item.name"
                        :subtitle="formatInterfaceSubtitle(item)"
                      >
                        <template #append>
                          <v-chip
                            size="x-small"
                            :color="item.operStatus === 'up' ? 'success' : 'grey'"
                            variant="tonal"
                          >
                            {{ item.operStatus || 'up' }}
                          </v-chip>
                        </template>
                      </v-list-item>
                    </template>
                  </v-select>
                </div>
              </v-card>
            </v-col>

            <!-- Tipo de Cabo / Enlace -->
            <v-col cols="12" class="mt-2">
              <v-select
                v-model="form.linkType"
                :items="linkTypeOptions"
                item-title="label"
                item-value="value"
                label="Tipo de Meio / Tecnologia de Enlace"
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-transit-connection-variant"
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item v-bind="itemProps">
                    <template #prepend>
                      <v-icon :color="item.color">{{ item.icon }}</v-icon>
                    </template>
                  </v-list-item>
                </template>
              </v-select>
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 px-6 justify-end bg-surface">
        <v-btn variant="text" :disabled="saving" @click="close">Cancelar</v-btn>
        <v-btn
          color="primary"
          variant="elevated"
          prepend-icon="mdi-check-bold"
          :loading="saving"
          @click="save"
        >
          {{ isEditMode ? 'Salvar Alterações' : 'Salvar Conexão' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch } from 'vue'
import { useTopologyStore, type DeviceInterfaceItem } from '@/stores/topology'

const props = defineProps<{
  modelValue: boolean
  editingLinkId?: number | null
  initialSourceDeviceId?: number | null
  initialTargetDeviceId?: number | null
  initialSourceInterfaceId?: number | null
  initialTargetInterfaceId?: number | null
  initialLinkType?: string | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved'): void
}>()

const isEditMode = computed(() => !!props.editingLinkId && props.editingLinkId > 0)

const topologyStore = useTopologyStore()
const formRef = ref()
const saving = ref(false)
const loadingSourceInterfaces = ref(false)
const loadingTargetInterfaces = ref(false)

const sourceInterfaces = ref<DeviceInterfaceItem[]>([])
const targetInterfaces = ref<DeviceInterfaceItem[]>([])

const form = reactive<{
  sourceDeviceId: number | null
  targetDeviceId: number | null
  sourceInterfaceId: number | null
  targetInterfaceId: number | null
  linkType: string
}>({
  sourceDeviceId: null,
  targetDeviceId: null,
  sourceInterfaceId: null,
  targetInterfaceId: null,
  linkType: 'manual',
})

const rules = {
  required: (v: unknown) => !!v || 'Campo obrigatório',
  differentDevices: (v: number | null) =>
    !v || v !== form.sourceDeviceId || 'Origem e Destino devem ser equipamentos diferentes',
}

const linkTypeOptions = [
  {
    value: 'manual',
    label: 'Cabo Ethernet / Par Trançado (UTP)',
    icon: 'mdi-ethernet',
    color: '#2196F3',
  },
  {
    value: 'fiber',
    label: 'Fibra Óptica (GBIC / SFP)',
    icon: 'mdi-laser-pointer',
    color: '#9C27B0',
  },
  {
    value: 'wireless',
    label: 'Enlace Sem Fio / Rádio (Wireless)',
    icon: 'mdi-wifi',
    color: '#4CAF50',
  },
  {
    value: 'vpn',
    label: 'Túnel VPN / Lógico',
    icon: 'mdi-shield-lock',
    color: '#FF9800',
  },
  {
    value: 'lldp',
    label: 'Descoberto via LLDP',
    icon: 'mdi-auto-fix',
    color: '#00BCD4',
  },
]

const deviceOptions = computed(() => topologyStore.nodes)
const targetDeviceOptions = computed(() =>
  topologyStore.nodes.filter((n) => n.id !== form.sourceDeviceId)
)

const sourceInterfaceOptions = computed(() => sourceInterfaces.value)
const targetInterfaceOptions = computed(() => targetInterfaces.value)

const selectedSourceNode = computed(() =>
  topologyStore.nodes.find((n) => n.id === form.sourceDeviceId)
)
const selectedTargetNode = computed(() =>
  topologyStore.nodes.find((n) => n.id === form.targetDeviceId)
)

const selectedSourceInterface = computed(() =>
  sourceInterfaces.value.find((i) => i.id === form.sourceInterfaceId)
)
const selectedTargetInterface = computed(() =>
  targetInterfaces.value.find((i) => i.id === form.targetInterfaceId)
)

watch(
  () => props.modelValue,
  async (isOpen) => {
    if (isOpen) {
      form.sourceDeviceId = props.initialSourceDeviceId ?? null
      form.targetDeviceId = props.initialTargetDeviceId ?? null
      form.sourceInterfaceId = props.initialSourceInterfaceId ?? null
      form.targetInterfaceId = props.initialTargetInterfaceId ?? null
      form.linkType = props.initialLinkType || 'manual'

      if (form.sourceDeviceId) {
        await loadSourceInterfaces(form.sourceDeviceId)
      }
      if (form.targetDeviceId) {
        await loadTargetInterfaces(form.targetDeviceId)
      }
    }
  }
)

async function loadSourceInterfaces(deviceId: number) {
  loadingSourceInterfaces.value = true
  try {
    sourceInterfaces.value = await topologyStore.fetchDeviceInterfaces(deviceId)
  } finally {
    loadingSourceInterfaces.value = false
  }
}

async function loadTargetInterfaces(deviceId: number) {
  loadingTargetInterfaces.value = true
  try {
    targetInterfaces.value = await topologyStore.fetchDeviceInterfaces(deviceId)
  } finally {
    loadingTargetInterfaces.value = false
  }
}

async function onSourceDeviceChanged(deviceId: number | null) {
  form.sourceInterfaceId = null
  sourceInterfaces.value = []
  if (!deviceId) return
  await loadSourceInterfaces(deviceId)
}

async function onTargetDeviceChanged(deviceId: number | null) {
  form.targetInterfaceId = null
  targetInterfaces.value = []
  if (!deviceId) return
  await loadTargetInterfaces(deviceId)
}

function formatInterfaceSubtitle(iface: DeviceInterfaceItem): string {
  const parts: string[] = []
  if (iface.description && iface.description !== iface.name) {
    parts.push(iface.description)
  }
  if (iface.alias) {
    parts.push(`Alias: ${iface.alias}`)
  }
  if (iface.speed) {
    const mbps = iface.speed / 1_000_000
    parts.push(mbps >= 1000 ? `${mbps / 1000} Gbps` : `${mbps} Mbps`)
  }
  return parts.join(' • ') || 'Interface padrão'
}

function getNodeIcon(type: string) {
  switch (type?.toLowerCase()) {
    case 'router':
      return 'mdi-router'
    case 'switch':
      return 'mdi-expansion-card'
    case 'unmanaged_switch':
    case 'hub':
      return 'mdi-hub'
    case 'server':
      return 'mdi-server'
    case 'firewall':
      return 'mdi-shield-check'
    case 'ap':
    case 'wireless':
      return 'mdi-access-point'
    default:
      return 'mdi-desktop-tower'
  }
}

function getLinkColor(type: string): string {
  const opt = linkTypeOptions.find((o) => o.value === type)
  return opt ? opt.color : '#2196F3'
}

function getLinkTypeIcon(type: string): string {
  const opt = linkTypeOptions.find((o) => o.value === type)
  return opt ? opt.icon : 'mdi-ethernet'
}

function getLinkTypeLabel(type: string): string {
  const opt = linkTypeOptions.find((o) => o.value === type)
  return opt ? opt.label : 'Cabo Ethernet'
}

function close() {
  emit('update:modelValue', false)
}

async function save() {
  const isValid = await formRef.value?.validate()
  if (!isValid?.valid) return

  if (!form.sourceDeviceId || !form.targetDeviceId) return

  saving.value = true
  try {
    let success = false
    if (isEditMode.value && props.editingLinkId) {
      success = await topologyStore.updateLink(props.editingLinkId, {
        sourceInterfaceId: form.sourceInterfaceId,
        targetInterfaceId: form.targetInterfaceId,
        linkType: form.linkType,
      })
    } else {
      success = await topologyStore.addLink({
        sourceDeviceId: form.sourceDeviceId,
        targetDeviceId: form.targetDeviceId,
        sourceInterfaceId: form.sourceInterfaceId,
        targetInterfaceId: form.targetInterfaceId,
        linkType: form.linkType,
      })
    }
    if (success) {
      emit('saved')
      close()
    }
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.connection-preview-card {
  background: rgba(var(--v-theme-surface-variant), 0.35);
  border: 1px dashed rgba(var(--v-theme-primary), 0.4);
}
.preview-box {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.1);
  min-width: 140px;
}
.cable-pulse-icon {
  animation: pulse-glow 2s infinite ease-in-out;
}
@keyframes pulse-glow {
  0% {
    transform: scale(1);
    opacity: 0.8;
  }
  50% {
    transform: scale(1.15);
    opacity: 1;
  }
  100% {
    transform: scale(1);
    opacity: 0.8;
  }
}
.border-primary-subtle {
  border-color: rgba(var(--v-theme-primary), 0.3) !important;
}
.border-success-subtle {
  border-color: rgba(var(--v-theme-success), 0.3) !important;
}
</style>

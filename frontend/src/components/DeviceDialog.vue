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

            <!--
              Fica junto do IP, e não numa seção avançada: é a mesma pergunta
              que o endereço responde pela metade — "onde está o equipamento" —
              e é ela que decide qual endereço deste servidor será gravado
              dentro dele quando o log for ativado.
            -->
            <v-col cols="12" sm="6">
              <v-select
                v-model="formModel.accessMode"
                :items="accessModeItems"
                item-title="title"
                item-value="value"
                :item-props="accessModeItemProps"
                label="Forma de acesso (Opcional)"
                variant="outlined"
                density="comfortable"
                :prepend-inner-icon="accessModeMeta(formModel.accessMode).icon"
                :hint="accessModeHint"
                persistent-hint
              ></v-select>
            </v-col>

            <!--
              O sistema, e não o fabricante: é ele que decide os comandos de
              syslog, se o MAC-Telnet é possível e qual perfil da VPN serve. O
              catálogo vem do servidor — é a mesma lista da ativação de log e do
              assistente da VPN.
            -->
            <v-col cols="12" sm="6">
              <v-select
                v-model="formModel.operatingSystem"
                :items="operatingSystemItems"
                item-title="title"
                item-value="value"
                :item-props="operatingSystemItemProps"
                label="Sistema (Opcional)"
                variant="outlined"
                density="comfortable"
                :loading="systemsStore.loading"
                :prepend-inner-icon="operatingSystemIcon"
                :hint="operatingSystemHint"
                persistent-hint
              ></v-select>
            </v-col>

            <v-col cols="12" sm="6">
              <v-text-field
                v-model="formModel.vendor"
                label="Fabricante / Vendor"
                placeholder="Cisco, MikroTik, Ubiquiti"
                variant="outlined"
                density="comfortable"
                hint="O nome do fabricante do equipamento — o sistema é o campo ao lado."
                persistent-hint
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
            <v-col v-if="formModel.snmpEnabled" cols="12" sm="6">
              <v-select
                v-model="formModel.snmpPollIntervalSeconds"
                :items="snmpIntervalItems"
                item-title="title"
                item-value="value"
                label="Intervalo de coleta SNMP"
                prepend-inner-icon="mdi-timer-sync-outline"
                variant="outlined"
                density="comfortable"
                hint="Uma coleta por dispositivo atualiza todas as métricas SNMP vinculadas."
                persistent-hint
              ></v-select>
            </v-col>
            <v-col v-if="formModel.snmpEnabled" cols="12">
              <v-alert
                type="info"
                variant="tonal"
                density="compact"
                icon="mdi-database-refresh-outline"
              >
                CPU, memória e interfaces compartilham este intervalo. Para evitar consultas
                repetidas, ajuste-o aqui — não em cada item SNMP.
              </v-alert>
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

    <v-dialog v-model="snmpIntervalConfirmation" max-width="520">
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold">Aplicar intervalo SNMP?</v-card-title>
        <v-card-text>
          O intervalo de coleta SNMP será alterado para todos os itens SNMP deste dispositivo. Ping
          e os demais monitores não serão modificados.
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="snmpIntervalConfirmation = false">Cancelar</v-btn>
          <v-btn color="primary" @click="confirmSnmpIntervalChange">Aplicar a todos</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch } from 'vue'
import { useDevicesStore, type Device } from '@/stores/devices'
import { useSitesStore, type Site } from '@/stores/sites'
import { useSnmpTestStore } from '@/stores/snmpTest'
import { usePreferencesStore } from '@/stores/preferences'
import SiteDialog from '@/components/SiteDialog.vue'
import { INTERVAL_PRESETS, formatSeconds } from '@/utils/monitorTypes'
import {
  AUTO_ACCESS_MODE,
  accessModeMeta,
  accessModeOptions,
  type AccessModeChoice,
} from '@/utils/accessMode'
import {
  AUTO_OPERATING_SYSTEM,
  operatingSystemSourceLabel,
  useOperatingSystemsStore,
} from '@/stores/operatingSystems'

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
const prefsStore = usePreferencesStore()
const systemsStore = useOperatingSystemsStore()

const siteDialog = ref(false)
const saving = ref(false)
const snmpTestResult = ref<{ ok: boolean; message: string } | null>(null)
const snmpIntervalConfirmation = ref(false)
const originalSnmpPollIntervalSeconds = ref(15)
const snmpIntervalItems = INTERVAL_PRESETS.map((value) => ({
  value,
  title: `A cada ${formatSeconds(value)}`,
}))

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
  snmpPollIntervalSeconds: number
  accessMode: AccessModeChoice
  operatingSystem: string
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
  snmpPollIntervalSeconds: 15,
  accessMode: AUTO_ACCESS_MODE,
  operatingSystem: AUTO_OPERATING_SYSTEM,
})

const availableParentDevices = computed(() => {
  return devicesStore.devices.filter((d) => d.id !== props.deviceToEdit?.id)
})

/**
 * As opções, com a conclusão do sistema no subtítulo do "Automático".
 *
 * Num cadastro novo ainda não há o que concluir — o dispositivo não existe e o
 * servidor não tem como olhar rota nem VPN. Ali o subtítulo explica o critério
 * em vez de anunciar um resultado que seria inventado.
 */
const accessModeItems = computed(() =>
  accessModeOptions({
    mode: props.deviceToEdit?.effectiveAccessMode,
    reason: props.deviceToEdit?.accessModeReason,
  })
)

function accessModeItemProps(item: { subtitle: string; icon: string }): Record<string, unknown> {
  return { subtitle: item.subtitle, prependIcon: item.icon }
}

/**
 * O "Automático" primeiro, com a conclusão do sistema no subtítulo.
 *
 * Mesma regra do modo de acesso: num cadastro novo não há o que concluir, e o
 * subtítulo explica o critério em vez de anunciar um resultado inventado.
 */
const operatingSystemItems = computed(() => {
  const detectado = props.deviceToEdit?.effectiveOperatingSystem
  const origem = props.deviceToEdit?.operatingSystemSource
  return [
    {
      value: AUTO_OPERATING_SYSTEM,
      title: 'Detectar automaticamente',
      subtitle: detectado
        ? `Detectado: ${systemsStore.label(detectado)} — ${operatingSystemSourceLabel(origem).toLowerCase()}`
        : 'Pelo SNMP do equipamento, ou pelo fabricante informado',
      icon: detectado ? systemsStore.icon(detectado) : 'mdi-auto-fix',
    },
    ...systemsStore.systems.map((sistema) => ({
      value: sistema.id,
      title: sistema.label,
      subtitle: sistema.supportsSyslog
        ? 'Tem comandos prontos para ativação automática de log'
        : 'Sem comandos de syslog — a ativação automática não atende',
      icon: sistema.icon,
    })),
  ]
})

function operatingSystemItemProps(item: {
  subtitle: string
  icon: string
}): Record<string, unknown> {
  return { subtitle: item.subtitle, prependIcon: item.icon }
}

const operatingSystemIcon = computed(() =>
  formModel.operatingSystem === AUTO_OPERATING_SYSTEM
    ? 'mdi-auto-fix'
    : systemsStore.icon(formModel.operatingSystem)
)

const operatingSystemHint = computed(() =>
  formModel.operatingSystem === AUTO_OPERATING_SYSTEM
    ? 'Declare apenas se a detecção errar — ela usa o SNMP quando disponível.'
    : 'Decide os comandos de log, o meio de acesso e o perfil da VPN.'
)

const accessModeHint = computed(() => {
  if (formModel.accessMode !== AUTO_ACCESS_MODE) {
    return 'Decide qual endereço deste servidor é gravado no equipamento ao ativar o log.'
  }
  return props.deviceToEdit
    ? 'O sistema decide sozinho. Declare apenas se a conclusão acima estiver errada.'
    : 'O sistema decide depois de salvar, pela rota até o equipamento.'
})

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) {
      if (devicesStore.devices.length === 0) devicesStore.fetchDevices()
      if (sitesStore.sites.length === 0) sitesStore.fetchSites()

      snmpTestResult.value = null
      // Sem isto a preferência só valeria depois de o operador visitar
      // Configurações na mesma sessão.
      void prefsStore.fetchAll()
      // O catálogo de sistemas é do servidor: é a mesma lista que a ativação de
      // log e o assistente da VPN usam.
      void systemsStore.fetchAll()
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
        formModel.snmpPollIntervalSeconds = props.deviceToEdit.snmpPollIntervalSeconds || 15
        // Só a **declaração** volta para o campo. Trazer o efetivo faria a
        // dedução parecer escolha do operador, e ele nunca conseguiria voltar
        // ao automático — não teria como saber que já não estava nele.
        formModel.accessMode = props.deviceToEdit.accessMode ?? AUTO_ACCESS_MODE
        formModel.operatingSystem = props.deviceToEdit.operatingSystem ?? AUTO_OPERATING_SYSTEM
        originalSnmpPollIntervalSeconds.value = formModel.snmpPollIntervalSeconds
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
        formModel.snmpCommunity =
          props.prefillData.snmpCommunity || prefsStore.preferences.defaultSnmpCommunity
        formModel.snmpVersion = props.prefillData.snmpVersion || 'v2c'
        formModel.snmpPollIntervalSeconds = props.prefillData.snmpPollIntervalSeconds || 15
        formModel.accessMode = props.prefillData.accessMode ?? AUTO_ACCESS_MODE
        formModel.operatingSystem = props.prefillData.operatingSystem ?? AUTO_OPERATING_SYSTEM
        originalSnmpPollIntervalSeconds.value = formModel.snmpPollIntervalSeconds
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
        // A preferência global, não um literal: é ela que faz a
        // "Comunidade SNMP padrão" das Configurações valer para um cadastro novo.
        formModel.snmpCommunity = prefsStore.preferences.defaultSnmpCommunity
        formModel.snmpVersion = 'v2c'
        formModel.snmpPollIntervalSeconds = 15
        formModel.accessMode = AUTO_ACCESS_MODE
        formModel.operatingSystem = AUTO_OPERATING_SYSTEM
        originalSnmpPollIntervalSeconds.value = 15
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
  const intervalChanged =
    props.deviceToEdit &&
    formModel.snmpEnabled &&
    formModel.snmpPollIntervalSeconds !== originalSnmpPollIntervalSeconds.value
  if (intervalChanged) {
    snmpIntervalConfirmation.value = true
    return
  }
  await persist()
}

async function confirmSnmpIntervalChange() {
  snmpIntervalConfirmation.value = false
  await persist()
}

/**
 * O corpo enviado ao servidor.
 *
 * O `accessMode` é assimétrico de propósito: na escrita a API aceita `auto`
 * (que apaga a declaração), e na leitura ela nunca devolve essa palavra — lá o
 * automático é `null`. Por isso o `Device`, que descreve a resposta, não tem
 * como tipar este valor, e a conversão fica explícita aqui.
 */
function payload(): Partial<Device> {
  return {
    ...formModel,
    accessMode: formModel.accessMode as Device['accessMode'],
    operatingSystem: formModel.operatingSystem as Device['operatingSystem'],
  }
}

async function persist() {
  if (!formModel.name || !formModel.ipAddress) return
  saving.value = true
  try {
    if (props.deviceToEdit && props.deviceToEdit.id) {
      const success = await devicesStore.updateDevice(props.deviceToEdit.id, payload())
      if (success) {
        await devicesStore.fetchDevices()
        const updated = devicesStore.devices.find((d) => d.id === props.deviceToEdit?.id)
        if (updated) emit('saved', updated)
        close()
      }
    } else {
      const success = await devicesStore.createDevice({
        ...payload(),
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

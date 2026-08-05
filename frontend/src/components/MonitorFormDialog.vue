<template>
  <v-dialog
    :model-value="modelValue"
    max-width="860"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-item class="pa-5 pb-3">
        <template #prepend>
          <v-avatar :color="definition.color" size="44" rounded="lg" variant="tonal">
            <v-icon size="24">{{ definition.icon }}</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-h6">
          {{ isEditing ? 'Editar monitor' : 'Novo monitor' }} · {{ definition.label }}
        </v-card-title>
        <v-card-subtitle>{{ definition.description }}</v-card-subtitle>
      </v-card-item>

      <v-divider></v-divider>

      <v-card-text class="pa-5">
        <v-form @submit.prevent="save(false)">
          <!-- Etapa 1: o que verificar -->
          <div class="text-overline text-medium-emphasis mb-2">1 · Tipo de checagem</div>
          <div class="d-flex flex-wrap ga-2 mb-2">
            <v-card
              v-for="def in MONITOR_KINDS"
              :key="def.kind"
              class="kind-card pa-3 text-center"
              :variant="form.kind === def.kind ? 'tonal' : 'outlined'"
              :color="form.kind === def.kind ? def.color : undefined"
              @click="selectKind(def.kind)"
            >
              <v-icon size="24">{{ def.icon }}</v-icon>
              <div class="text-body-2 font-weight-medium mt-1">{{ def.label }}</div>
              <div class="text-caption text-medium-emphasis">{{ def.tagline }}</div>
            </v-card>
          </div>

          <v-alert
            v-if="isEditing && form.kind !== originalKind"
            type="warning"
            variant="tonal"
            density="compact"
            class="mb-4"
            text="Mudar o tipo substitui a configuração da checagem e reinicia o histórico de leituras."
          ></v-alert>

          <!-- Etapa 2: alvo da verificação -->
          <div class="text-overline text-medium-emphasis mt-8 mb-3">2 · Alvo da verificação</div>
          <v-row class="form-rows">
            <v-col v-if="definition.requiresDevice" cols="12">
              <v-select
                v-model="form.deviceId"
                :items="devicesStore.devices"
                item-title="name"
                item-value="id"
                label="Equipamento consultado *"
                prepend-inner-icon="mdi-router-network"
                variant="outlined"
                density="comfortable"
                :disabled="deviceLocked"
                :rules="[(v: unknown) => !!v || 'Selecione o equipamento']"
                :hint="definition.deviceHint"
                persistent-hint
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item v-bind="itemProps" :subtitle="deviceSubtitle(item)"></v-list-item>
                </template>
              </v-select>
            </v-col>

            <v-col cols="12">
              <v-text-field
                :model-value="form.target"
                :label="`${definition.target.label} *`"
                :placeholder="definition.target.placeholder"
                :hint="definition.target.hint"
                :prepend-inner-icon="definition.target.icon"
                :rules="[definition.target.rule]"
                variant="outlined"
                density="comfortable"
                persistent-hint
                spellcheck="false"
                autocapitalize="off"
                @update:model-value="onTargetInput"
                @blur="onTargetBlur"
              >
                <template v-if="canFillFromDevice" #append-inner>
                  <v-btn
                    icon
                    size="x-small"
                    variant="text"
                    density="comfortable"
                    @click.stop="fillTargetFromDevice"
                  >
                    <v-icon size="18">mdi-auto-fix</v-icon>
                    <v-tooltip activator="parent" location="top">
                      Usar o IP do dispositivo ({{ selectedDevice?.ipAddress }})
                    </v-tooltip>
                  </v-btn>
                </template>
              </v-text-field>
            </v-col>

            <v-col v-if="definition.usesPort" cols="12">
              <v-text-field
                v-model.number="form.port"
                label="Porta TCP *"
                type="number"
                min="1"
                max="65535"
                prepend-inner-icon="mdi-numeric"
                variant="outlined"
                density="comfortable"
                :rules="[(v: unknown) => isValidPort(v) || 'Informe uma porta entre 1 e 65535']"
                hide-details="auto"
              ></v-text-field>
              <div class="d-flex flex-wrap ga-1 mt-2">
                <v-chip
                  v-for="preset in COMMON_TCP_PORTS"
                  :key="preset.port"
                  size="small"
                  :variant="form.port === preset.port ? 'flat' : 'outlined'"
                  :color="form.port === preset.port ? 'primary' : undefined"
                  @click="form.port = preset.port"
                >
                  {{ preset.label }} · {{ preset.port }}
                </v-chip>
              </div>
            </v-col>

            <v-col v-if="form.kind === 'snmp'" cols="12" md="6">
              <v-select
                v-model="form.snmpMode"
                :items="SNMP_MODES"
                item-title="label"
                item-value="value"
                label="O que coletar via SNMP"
                prepend-inner-icon="mdi-format-list-bulleted-type"
                variant="outlined"
                density="comfortable"
                :hint="snmpModeDescription"
                persistent-hint
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item
                    v-bind="itemProps"
                    :subtitle="itemField(item, 'description')"
                    :prepend-icon="itemField(item, 'icon')"
                  ></v-list-item>
                </template>
              </v-select>
            </v-col>

            <v-col v-if="form.kind === 'snmp' && form.snmpMode === 'interface'" cols="12" md="6">
              <v-select
                v-if="interfaceItems.length > 0"
                v-model="form.ifIndex"
                :items="interfaceItems"
                item-title="title"
                item-value="value"
                label="Interface monitorada *"
                prepend-inner-icon="mdi-ethernet"
                variant="outlined"
                density="comfortable"
                :loading="loadingInterfaces"
                :rules="[(v: unknown) => v !== null || 'Selecione a interface']"
                hint="Interfaces descobertas no último scan SNMP do dispositivo"
                persistent-hint
                @update:model-value="onInterfaceSelected"
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item
                    v-bind="itemProps"
                    :subtitle="itemField(item, 'subtitle')"
                  ></v-list-item>
                </template>
              </v-select>
              <v-text-field
                v-else
                v-model.number="form.ifIndex"
                label="ifIndex da interface *"
                type="number"
                min="1"
                prepend-inner-icon="mdi-ethernet"
                variant="outlined"
                density="comfortable"
                :loading="loadingInterfaces"
                :rules="[(v: unknown) => v !== null || 'Informe o ifIndex']"
                hint="Nenhuma interface descoberta — rode um scan SNMP no dispositivo ou informe o índice manualmente"
                persistent-hint
              ></v-text-field>
            </v-col>

            <v-col v-if="form.kind === 'dns'" cols="12" md="6">
              <v-select
                v-model="form.dnsProtocol"
                :items="DNS_PROTOCOLS"
                item-title="label"
                item-value="value"
                label="Transporte da consulta"
                prepend-inner-icon="mdi-transit-connection"
                variant="outlined"
                density="comfortable"
                :hint="dnsProtocolDefinition.description"
                persistent-hint
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item
                    v-bind="itemProps"
                    :subtitle="itemField(item, 'description')"
                    :prepend-icon="itemField(item, 'icon')"
                  ></v-list-item>
                </template>
              </v-select>
            </v-col>

            <v-col v-if="form.kind === 'dns'" cols="12" md="6">
              <v-select
                v-model="form.recordType"
                :items="DNS_RECORD_TYPES"
                item-title="title"
                item-value="value"
                label="Tipo de registro"
                prepend-inner-icon="mdi-file-document-outline"
                variant="outlined"
                density="comfortable"
                hide-details="auto"
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item
                    v-bind="itemProps"
                    :subtitle="itemField(item, 'subtitle')"
                  ></v-list-item>
                </template>
              </v-select>
            </v-col>

            <v-col v-if="form.kind === 'dns' && dnsProtocolDefinition.requiresServer" cols="12">
              <div class="d-flex align-start ga-2">
                <v-combobox
                  :model-value="form.dnsServer"
                  :items="dnsServerItems"
                  item-title="title"
                  item-value="value"
                  :return-object="false"
                  label="Servidor DNS medido *"
                  placeholder="Escolha um cadastrado ou digite um novo"
                  prepend-inner-icon="mdi-server"
                  variant="outlined"
                  density="comfortable"
                  class="flex-grow-1"
                  :loading="dnsServersStore.loading"
                  :rules="[dnsServerRule]"
                  :hint="dnsServerHint"
                  persistent-hint
                  @update:model-value="onDnsServerChange"
                >
                  <template #item="{ props: itemProps, item }">
                    <v-list-item
                      v-bind="itemProps"
                      :subtitle="itemField(item, 'subtitle')"
                      :prepend-icon="itemField(item, 'icon')"
                    ></v-list-item>
                  </template>
                </v-combobox>
                <v-btn
                  icon
                  variant="tonal"
                  color="deep-purple"
                  density="comfortable"
                  class="mt-1"
                  @click="openServersDialog"
                >
                  <v-icon>mdi-cog-outline</v-icon>
                  <v-tooltip activator="parent" location="top">Gerenciar servidores DNS</v-tooltip>
                </v-btn>
              </div>
            </v-col>

            <v-col v-if="form.kind === 'dns' && dnsProtocolDefinition.requiresEndpoint" cols="12">
              <div class="d-flex align-start ga-2">
                <v-combobox
                  :model-value="form.dohUrl"
                  :items="dohEndpointItems"
                  item-title="title"
                  item-value="value"
                  :return-object="false"
                  label="Endpoint DoH *"
                  placeholder="https://cloudflare-dns.com/dns-query"
                  prepend-inner-icon="mdi-lock-outline"
                  variant="outlined"
                  density="comfortable"
                  class="flex-grow-1"
                  :rules="[dohUrlRule]"
                  :hint="dohEndpointHint"
                  persistent-hint
                  @update:model-value="onDohUrlChange"
                >
                  <template #item="{ props: itemProps, item }">
                    <v-list-item
                      v-bind="itemProps"
                      :subtitle="itemField(item, 'subtitle')"
                    ></v-list-item>
                  </template>
                </v-combobox>
                <v-btn
                  icon
                  variant="tonal"
                  color="deep-purple"
                  density="comfortable"
                  class="mt-1"
                  @click="openServersDialog"
                >
                  <v-icon>mdi-cog-outline</v-icon>
                  <v-tooltip activator="parent" location="top">Gerenciar servidores DNS</v-tooltip>
                </v-btn>
              </div>
            </v-col>

            <v-col v-if="form.kind === 'http'" cols="12" md="6">
              <v-select
                v-model="form.httpMethod"
                :items="['GET', 'HEAD', 'POST']"
                label="Método HTTP"
                prepend-inner-icon="mdi-swap-horizontal"
                variant="outlined"
                density="comfortable"
                hide-details="auto"
              ></v-select>
            </v-col>

            <v-col cols="12">
              <v-text-field
                v-model="form.name"
                label="Nome do monitor *"
                :placeholder="suggestedName || 'Ex: Ping — Roteador Matriz'"
                prepend-inner-icon="mdi-tag-outline"
                variant="outlined"
                density="comfortable"
                :rules="[(v: string) => !!(v || '').trim() || 'Informe um nome para o monitor']"
                :hint="
                  nameTouched
                    ? 'Nome personalizado'
                    : 'Preenchido automaticamente — edite se quiser outro nome'
                "
                persistent-hint
                @update:model-value="nameTouched = true"
              ></v-text-field>
            </v-col>
          </v-row>

          <!-- Etapa 3: de onde a checagem parte e a que equipamento pertence -->
          <div class="text-overline text-medium-emphasis mt-8 mb-3">3 · Origem e vínculo</div>
          <v-row class="form-rows">
            <v-col cols="12" md="6">
              <v-select
                v-model="form.probeId"
                :items="probeItems"
                item-title="title"
                item-value="value"
                label="Executar a partir de"
                prepend-inner-icon="mdi-play-network-outline"
                variant="outlined"
                density="comfortable"
                hint="Ponto da rede que dispara a checagem e mede o tempo"
                persistent-hint
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item
                    v-bind="itemProps"
                    :subtitle="itemField(item, 'subtitle')"
                  ></v-list-item>
                </template>
              </v-select>
            </v-col>

            <v-col v-if="!definition.requiresDevice" cols="12" md="6">
              <v-select
                v-model="form.deviceId"
                :items="devicesStore.devices"
                item-title="name"
                item-value="id"
                label="Vincular a um equipamento (opcional)"
                prepend-inner-icon="mdi-link-variant"
                variant="outlined"
                density="comfortable"
                clearable
                :disabled="deviceLocked"
                :hint="definition.deviceHint"
                persistent-hint
              >
                <template #item="{ props: itemProps, item }">
                  <v-list-item v-bind="itemProps" :subtitle="deviceSubtitle(item)"></v-list-item>
                </template>
              </v-select>
            </v-col>
          </v-row>

          <!-- Etapa 4: frequência e ajustes finos -->
          <div class="text-overline text-medium-emphasis mt-8 mb-3">4 · Frequência</div>
          <v-row class="form-rows">
            <v-col cols="12" md="6">
              <v-select
                v-model="form.intervalSeconds"
                :items="intervalItems"
                item-title="title"
                item-value="value"
                label="Verificar a cada"
                prepend-inner-icon="mdi-timer-sync-outline"
                variant="outlined"
                density="comfortable"
                hide-details="auto"
              ></v-select>
            </v-col>
            <v-col cols="12" md="6">
              <v-select
                v-model="form.timeoutSeconds"
                :items="timeoutItems"
                item-title="title"
                item-value="value"
                label="Aguardar resposta por até"
                prepend-inner-icon="mdi-timer-alert-outline"
                variant="outlined"
                density="comfortable"
                :error-messages="timeoutError"
                hide-details="auto"
              ></v-select>
            </v-col>
          </v-row>

          <v-expansion-panels variant="accordion" class="mt-4">
            <v-expansion-panel>
              <v-expansion-panel-title>
                <v-icon size="18" class="mr-2">mdi-tune-variant</v-icon>
                Opções avançadas de {{ definition.label }}
              </v-expansion-panel-title>
              <v-expansion-panel-text>
                <v-row class="form-rows pt-2">
                  <v-col v-if="form.kind === 'ping'" cols="12" md="6">
                    <v-text-field
                      v-model.number="form.packetCount"
                      label="Pacotes por checagem"
                      type="number"
                      min="1"
                      max="20"
                      variant="outlined"
                      density="comfortable"
                      hint="Mais pacotes dão uma medida de perda mais confiável"
                      persistent-hint
                    ></v-text-field>
                  </v-col>

                  <v-col v-if="form.kind === 'http'" cols="12" md="8">
                    <v-combobox
                      :model-value="form.acceptedStatusCodes"
                      :items="[200, 201, 202, 204, 301, 302, 401, 403]"
                      label="Códigos HTTP aceitos"
                      variant="outlined"
                      density="comfortable"
                      multiple
                      chips
                      closable-chips
                      hint="Qualquer outro código marca o monitor como instável"
                      persistent-hint
                      @update:model-value="onStatusCodesChange"
                    ></v-combobox>
                  </v-col>
                  <v-col v-if="form.kind === 'http'" cols="12" md="4">
                    <v-switch
                      v-model="form.validateCertificate"
                      color="primary"
                      density="comfortable"
                      label="Validar certificado TLS"
                      hide-details
                    ></v-switch>
                  </v-col>

                  <v-col v-if="form.kind === 'dns'" cols="12" md="8">
                    <v-combobox
                      :model-value="form.extraHostnames"
                      label="Outros nomes medidos na mesma checagem"
                      placeholder="Digite e pressione Enter"
                      variant="outlined"
                      density="comfortable"
                      multiple
                      chips
                      closable-chips
                      hint="A latência publicada é a média de todos os nomes"
                      persistent-hint
                      @update:model-value="onExtraHostnamesChange"
                    ></v-combobox>
                  </v-col>
                  <v-col v-if="form.kind === 'dns'" cols="12" md="4">
                    <v-text-field
                      v-model.number="form.dnsWarningThresholdMs"
                      label="Alertar acima de (ms)"
                      type="number"
                      min="0"
                      variant="outlined"
                      density="comfortable"
                      clearable
                      hint="Em branco, só falhas geram alerta"
                      persistent-hint
                    ></v-text-field>
                  </v-col>

                  <v-col v-if="form.kind === 'snmp'" cols="12" md="4">
                    <v-select
                      v-model="form.snmpVersion"
                      :items="['v1', 'v2c', 'v3']"
                      label="Versão SNMP"
                      variant="outlined"
                      density="comfortable"
                      hide-details="auto"
                    ></v-select>
                  </v-col>
                  <v-col v-if="form.kind === 'snmp'" cols="12" md="4">
                    <v-text-field
                      v-model="form.snmpCommunity"
                      label="Comunidade"
                      variant="outlined"
                      density="comfortable"
                      hide-details="auto"
                    ></v-text-field>
                  </v-col>
                  <v-col v-if="form.kind === 'snmp'" cols="12" md="4">
                    <v-text-field
                      v-model.number="form.snmpPort"
                      label="Porta SNMP"
                      type="number"
                      min="1"
                      max="65535"
                      variant="outlined"
                      density="comfortable"
                      hide-details="auto"
                    ></v-text-field>
                  </v-col>

                  <v-col cols="12" md="6">
                    <v-text-field
                      v-model.number="form.retryCount"
                      label="Tentativas antes de marcar como offline"
                      type="number"
                      min="0"
                      max="10"
                      variant="outlined"
                      density="comfortable"
                      hide-details="auto"
                    ></v-text-field>
                  </v-col>
                  <v-col cols="12" md="6" class="d-flex align-center">
                    <v-switch
                      v-model="form.enabled"
                      color="success"
                      density="comfortable"
                      :label="form.enabled ? 'Monitor ativo' : 'Monitor pausado'"
                      hide-details
                    ></v-switch>
                  </v-col>
                </v-row>
              </v-expansion-panel-text>
            </v-expansion-panel>
          </v-expansion-panels>

          <v-alert
            variant="tonal"
            :color="definition.color"
            density="compact"
            class="mt-5"
            icon="mdi-clipboard-check-outline"
          >
            <div class="text-caption text-medium-emphasis">Resumo da checagem</div>
            <div class="text-body-2">{{ summary }}</div>
          </v-alert>

          <v-alert
            v-if="errorMessage"
            type="error"
            variant="tonal"
            density="compact"
            class="mt-3"
            :text="errorMessage"
          ></v-alert>
        </v-form>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4">
        <v-tooltip v-if="validationErrors.length > 0" location="top">
          <template #activator="{ props: tooltipProps }">
            <span v-bind="tooltipProps" class="text-caption text-medium-emphasis">
              <v-icon size="16" class="mr-1">mdi-alert-circle-outline</v-icon>
              {{ validationErrors.length }} pendência(s) no formulário
            </span>
          </template>
          <ul class="pl-4">
            <li v-for="issue in validationErrors" :key="issue">{{ issue }}</li>
          </ul>
        </v-tooltip>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-btn
          variant="tonal"
          color="primary"
          :disabled="!canSave"
          :loading="saving === 'test'"
          @click="save(true)"
        >
          Salvar e testar
        </v-btn>
        <v-btn
          color="primary"
          :disabled="!canSave"
          :loading="saving === 'save'"
          @click="save(false)"
        >
          Salvar
        </v-btn>
      </v-card-actions>
    </v-card>

    <DnsServersDialog
      v-model="serversDialog"
      :prefill-address="prefillAddress"
      @saved="onServerSaved"
    ></DnsServersDialog>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useDevicesStore } from '@/stores/devices'
import { useProbesStore } from '@/stores/probes'
import { useDnsServersStore, type DnsServer } from '@/stores/dnsServers'
import { apiService } from '@/services/apiService'
import type { DeviceInterface } from '@/stores/deviceDetail'
import DnsServersDialog from '@/components/DnsServersDialog.vue'
import {
  COMMON_TCP_PORTS,
  DNS_PROTOCOLS,
  DNS_RECORD_TYPES,
  INTERVAL_PRESETS,
  MONITOR_KINDS,
  SNMP_MODES,
  TIMEOUT_PRESETS,
  createMonitorForm,
  describeMonitor,
  dnsProtocol,
  formToPayload,
  formatSeconds,
  isHostname,
  isIpAddress,
  isValidPort,
  monitorKind,
  monitorToForm,
  suggestMonitorName,
  validateMonitorForm,
  type MonitorFormModel,
  type MonitorKind,
} from '@/utils/monitorTypes'

const props = defineProps<{
  modelValue: boolean
  monitor?: Monitor | null
  defaultDeviceId?: number | null
  /** Aberto de dentro de um equipamento: o vínculo já vem definido e travado */
  lockDevice?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', monitorId: number | null): void
}>()

const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()
const probesStore = useProbesStore()
const dnsServersStore = useDnsServersStore()

const serversDialog = ref(false)
const deviceLocked = computed(() => props.lockDevice === true)

const probeItems = computed(() => [
  {
    title: 'Servidor da aplicação',
    value: null,
    subtitle: 'A checagem parte de onde o sistema está instalado',
  },
  ...probesStore.probes
    .filter((probe) => probe.status !== 'revoked')
    .map((probe) => ({
      title: probe.name,
      value: probe.id as number | null,
      subtitle: `${probe.location || 'Sem localização'} · ${probe.status}`,
    })),
])

/** Cadastrados no transporte atual + o valor digitado que ainda não existe */
const dnsServerItems = computed(() =>
  dnsServersStore.servers
    .filter((server) => server.protocol === form.dnsProtocol)
    .map((server) => ({
      title: server.address,
      value: server.address,
      subtitle: server.description ? `${server.name} · ${server.description}` : server.name,
      icon: 'mdi-server',
    }))
)

const dohEndpointItems = computed(() =>
  dnsServersStore.servers
    .filter((server) => server.protocol === 'doh')
    .map((server) => ({
      title: server.address,
      value: server.address,
      subtitle: server.name,
    }))
)

/** Avisa quando o endereço digitado será cadastrado junto com o monitor */
function registryHint(address: string, fallback: string): string {
  const trimmed = address.trim()
  if (!trimmed) return fallback
  const known = dnsServersStore.findByAddress(trimmed, form.dnsProtocol)
  return known
    ? `${known.name}${known.description ? ` · ${known.description}` : ''}`
    : 'Servidor novo — será adicionado à sua lista ao salvar'
}

const dnsServerHint = computed(() =>
  registryHint(form.dnsServer, 'Escolha um servidor cadastrado ou digite um novo')
)

const dohEndpointHint = computed(() =>
  registryHint(form.dohUrl, 'Consulta enviada em POST no formato wire da RFC 8484')
)

const form = reactive<MonitorFormModel>(createMonitorForm())
const saving = ref<'save' | 'test' | null>(null)
const errorMessage = ref<string | null>(null)
const nameTouched = ref(false)
const originalKind = ref<MonitorKind>('ping')
const interfaces = ref<DeviceInterface[]>([])
const loadingInterfaces = ref(false)

const isEditing = computed(() => !!props.monitor?.id)
const definition = computed(() => monitorKind(form.kind))
const selectedDevice = computed(() =>
  devicesStore.devices.find((device) => device.id === form.deviceId)
)
const summary = computed(() => describeMonitor(form))
const validationErrors = computed(() => validateMonitorForm(form))
const canSave = computed(() => validationErrors.value.length === 0 && saving.value === null)

const suggestedName = computed(() => suggestMonitorName(form, selectedDevice.value?.name))

const snmpModeDescription = computed(
  () => SNMP_MODES.find((mode) => mode.value === form.snmpMode)?.description ?? ''
)

const canFillFromDevice = computed(
  () =>
    definition.value.suggestsDeviceIp &&
    !!selectedDevice.value?.ipAddress &&
    form.target !== selectedDevice.value.ipAddress
)

const timeoutError = computed(() =>
  form.timeoutSeconds >= form.intervalSeconds ? 'Deve ser menor que o intervalo' : undefined
)

/** Mantém valores fora dos presets (monitores antigos) visíveis no select */
function buildPresetItems(presets: number[], current: number) {
  const values = presets.includes(current)
    ? [...presets]
    : [...presets, current].sort((a, b) => a - b)
  return values.map((value) => ({ title: formatSeconds(value), value }))
}

const intervalItems = computed(() => buildPresetItems(INTERVAL_PRESETS, form.intervalSeconds))
const timeoutItems = computed(() => buildPresetItems(TIMEOUT_PRESETS, form.timeoutSeconds))

const interfaceItems = computed(() =>
  interfaces.value
    .map((iface) => {
      const index = iface.ifIndex ?? iface.snmpIndex
      if (index === undefined || index === null) return null
      const name = iface.ifName || iface.name || `Interface #${index}`
      const status = iface.ifOperStatus || iface.operStatus || 'desconhecido'
      return {
        title: `${name} (#${index})`,
        value: Number(index),
        subtitle: `Estado: ${status}`,
        name,
      }
    })
    .filter((item): item is { title: string; value: number; subtitle: string; name: string } =>
      Boolean(item)
    )
)

/**
 * Nos slots de item o Vuetify entrega o objeto original dentro de `raw`, mas a
 * tipagem gerada varia conforme a versão — este acesso tolerante evita depender
 * de um formato específico.
 */
function itemField(item: unknown, field: string): string {
  const wrapper = item as { raw?: Record<string, unknown> } | Record<string, unknown> | null
  const source = (
    wrapper && typeof wrapper === 'object' && 'raw' in wrapper
      ? (wrapper as { raw?: Record<string, unknown> }).raw
      : wrapper
  ) as Record<string, unknown> | null
  const value = source?.[field]
  return value === undefined || value === null ? '' : String(value)
}

function deviceSubtitle(item: unknown): string {
  return itemField(item, 'ipAddress') || 'Sem IP cadastrado'
}

const dnsProtocolDefinition = computed(() => dnsProtocol(form.dnsProtocol))

const dnsServerRule = (value: unknown) => {
  const text = String(value ?? '').trim()
  if (!text) {
    return dnsProtocolDefinition.value.requiresServer ? 'Informe o servidor DNS medido' : true
  }
  const host = text.split(':')[0] ?? ''
  return isIpAddress(host) || isHostname(host) || 'Informe um IP ou hostname válido (ex: 1.1.1.1)'
}

const dohUrlRule = (value: unknown) => {
  const text = String(value ?? '').trim()
  if (!text) return 'Informe o endpoint DoH'
  return /^https:\/\/.+/i.test(text) || 'O endpoint DoH precisa começar com https://'
}

function onDnsServerChange(value: unknown) {
  form.dnsServer =
    typeof value === 'string' ? value.trim() : ((value as { value?: string })?.value ?? '')
}

function onDohUrlChange(value: unknown) {
  form.dohUrl =
    typeof value === 'string' ? value.trim() : ((value as { value?: string })?.value ?? '')
}

/** Leva o que já foi digitado para o diálogo de cadastro */
const prefillAddress = computed(() =>
  form.dnsProtocol === 'doh' ? form.dohUrl.trim() : form.dnsServer.trim()
)

function openServersDialog() {
  serversDialog.value = true
}

/** Servidor recém-cadastrado no diálogo já entra selecionado no formulário */
function onServerSaved(server: DnsServer) {
  if (server.protocol === 'doh') {
    form.dnsProtocol = 'doh'
    form.dohUrl = server.address
  } else {
    form.dnsProtocol = server.protocol
    form.dnsServer = server.address
  }
}

function onExtraHostnamesChange(value: unknown) {
  const list = Array.isArray(value) ? value : []
  form.extraHostnames = list.map((item) => String(item).trim().toLowerCase()).filter(Boolean)
}

function resetForm() {
  errorMessage.value = null
  interfaces.value = []

  // Permite abrir o diálogo a partir de telas que ainda não carregaram os dados
  if (devicesStore.devices.length === 0) devicesStore.fetchDevices()
  if (probesStore.probes.length === 0) probesStore.fetchProbes()
  dnsServersStore.fetchServers()

  if (props.monitor) {
    Object.assign(form, monitorToForm(props.monitor))
    nameTouched.value = true
  } else {
    Object.assign(form, createMonitorForm(props.defaultDeviceId ?? devicesStore.devices[0]?.id))
    nameTouched.value = false
    applyDeviceTarget()
    // O watcher de sugestão só dispara quando algum campo muda — ao reabrir o
    // diálogo com o mesmo estado anterior o nome ficaria vazio sem esta linha
    form.name = suggestedName.value
  }

  originalKind.value = form.kind
}

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) resetForm()
  }
)

function selectKind(kind: MonitorKind) {
  if (form.kind === kind) return
  form.kind = kind

  const nextDefinition = monitorKind(kind)
  form.port = nextDefinition.usesPort ? (form.port ?? nextDefinition.defaultPort ?? null) : null

  // Tenta reaproveitar o alvo no formato do novo tipo (URL vira host, etc.)
  if (form.target && nextDefinition.target.rule(form.target) !== true) {
    const coerced = nextDefinition.target.coerce(form.target)
    form.target = coerced.target
    if (coerced.port && nextDefinition.usesPort) form.port = coerced.port
  }

  // Continua inválido? Melhor esvaziar do que deixar um IP num campo de domínio
  if (form.target && nextDefinition.target.rule(form.target) !== true) {
    form.target = ''
  }

  if (!form.target && nextDefinition.suggestsDeviceIp) applyDeviceTarget()
}

function applyDeviceTarget() {
  const ip = selectedDevice.value?.ipAddress
  if (!ip) return
  if (!definition.value.suggestsDeviceIp) return
  form.target = ip
}

function fillTargetFromDevice() {
  const ip = selectedDevice.value?.ipAddress
  if (ip) form.target = ip
}

function onTargetInput(value: string) {
  form.target = definition.value.target.sanitize(value ?? '')
}

function onTargetBlur() {
  if (!form.target) return
  const coerced = definition.value.target.coerce(form.target)
  form.target = coerced.target
  if (coerced.port && definition.value.usesPort) form.port = coerced.port
}

function onStatusCodesChange(value: unknown) {
  const list = Array.isArray(value) ? value : []
  form.acceptedStatusCodes = list
    .map((item) => Number(item))
    .filter((code) => Number.isInteger(code) && code >= 100 && code <= 599)
}

function onInterfaceSelected(value: unknown) {
  const selected = interfaceItems.value.find((item) => item.value === Number(value))
  form.ifName = selected?.name ?? ''
}

async function loadInterfaces() {
  if (form.kind !== 'snmp' || form.snmpMode !== 'interface' || !form.deviceId) return
  loadingInterfaces.value = true
  try {
    const data = await apiService.get<DeviceInterface[]>(`/devices/${form.deviceId}/interfaces`)
    interfaces.value = Array.isArray(data) ? data : []
  } catch {
    interfaces.value = []
  } finally {
    loadingInterfaces.value = false
  }
}

watch(
  () => [form.deviceId, form.snmpMode, form.kind],
  () => {
    if (form.kind === 'snmp' && form.snmpMode === 'interface') loadInterfaces()
  }
)

// Enquanto o usuário não digitar um nome próprio, ele acompanha o que foi configurado
watch(
  () => [
    form.kind,
    form.target,
    form.port,
    form.deviceId,
    form.recordType,
    form.snmpMode,
    form.ifName,
  ],
  () => {
    if (!nameTouched.value) form.name = suggestedName.value
  }
)

watch(
  () => form.deviceId,
  (deviceId, previousDeviceId) => {
    if (!deviceId || deviceId === previousDeviceId) return
    const previousIp = devicesStore.devices.find((d) => d.id === previousDeviceId)?.ipAddress
    if (!form.target || form.target === previousIp) applyDeviceTarget()
  }
)

function close() {
  emit('update:modelValue', false)
}

async function save(runAfterSave: boolean) {
  if (!canSave.value) return
  saving.value = runAfterSave ? 'test' : 'save'
  errorMessage.value = null
  monitorsStore.error = null

  try {
    // Endereço digitado à mão entra no cadastro para virar atalho da próxima vez
    if (form.kind === 'dns' && form.dnsProtocol !== 'system') {
      const address = form.dnsProtocol === 'doh' ? form.dohUrl : form.dnsServer
      await dnsServersStore.ensureServer(address, form.dnsProtocol)
    }

    const payload = formToPayload(form)
    let savedId: number | null = props.monitor?.id ?? null
    let succeeded = false

    if (props.monitor?.id) {
      succeeded = await monitorsStore.updateMonitor(props.monitor.id, payload)
    } else {
      succeeded = await monitorsStore.createMonitor(payload)
      savedId = monitorsStore.monitors[monitorsStore.monitors.length - 1]?.id ?? null
    }

    if (!succeeded) {
      errorMessage.value = monitorsStore.error || 'Não foi possível salvar o monitor'
      return
    }

    if (runAfterSave && savedId) await monitorsStore.runMonitor(savedId)

    emit('saved', savedId)
    close()
  } finally {
    saving.value = null
  }
}
</script>

<style scoped>
.kind-card {
  flex: 1 1 140px;
  min-width: 140px;
  cursor: pointer;
  transition: transform 0.12s ease;
}

/* Respiro entre os campos: com dicas persistentes o padrão do Vuetify fica apertado */
.form-rows > .v-col,
.form-rows > [class*='v-col-'] {
  padding-top: 10px;
  padding-bottom: 10px;
}

.kind-card:hover {
  transform: translateY(-2px);
}
</style>

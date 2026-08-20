<template>
  <div class="w-100 d-contents">
    <PingFields
      v-if="kind === 'ping'"
      :packet-count="packetCount"
      @update:packet-count="emit('update:packetCount', $event)"
    />

    <template v-if="kind === 'http'">
      <v-col cols="12" md="8">
        <v-combobox
          :model-value="acceptedStatusCodes"
          :items="[200, 201, 202, 204, 301, 302, 401, 403]"
          label="Códigos HTTP aceitos"
          variant="outlined"
          density="comfortable"
          multiple
          chips
          closable-chips
          hint="Qualquer outro código marca o monitor como instável"
          persistent-hint
          @update:model-value="emit('statusCodesChange', $event)"
        ></v-combobox>
      </v-col>
      <v-col cols="12" md="4">
        <v-switch
          :model-value="validateCertificate"
          color="primary"
          density="comfortable"
          label="Validar certificado TLS"
          hide-details
          @update:model-value="emit('update:validateCertificate', $event === true)"
        ></v-switch>
      </v-col>
    </template>

    <template v-if="kind === 'dns'">
      <v-col cols="12" md="8">
        <v-combobox
          :model-value="extraHostnames"
          label="Outros nomes medidos na mesma checagem"
          placeholder="Digite e pressione Enter"
          variant="outlined"
          density="comfortable"
          multiple
          chips
          closable-chips
          hint="A latência publicada é a média de todos os nomes"
          persistent-hint
          @update:model-value="emit('extraHostnamesChange', $event)"
        ></v-combobox>
      </v-col>
      <v-col cols="12" md="4">
        <v-text-field
          :model-value="dnsWarningThresholdMs"
          label="Alertar acima de (ms)"
          type="number"
          min="0"
          variant="outlined"
          density="comfortable"
          clearable
          hint="Em branco, só falhas geram alerta"
          persistent-hint
          @update:model-value="
            emit(
              'update:dnsWarningThresholdMs',
              $event !== '' && $event !== null ? Number($event) : null
            )
          "
        ></v-text-field>
      </v-col>
    </template>

    <template v-if="kind === 'snmp'">
      <v-col cols="12" md="4">
        <v-select
          :model-value="snmpVersion"
          :items="['v1', 'v2c', 'v3']"
          label="Versão SNMP"
          variant="outlined"
          density="comfortable"
          hide-details="auto"
          @update:model-value="emit('update:snmpVersion', $event)"
        ></v-select>
      </v-col>
      <v-col cols="12" md="4">
        <v-text-field
          :model-value="snmpCommunity"
          label="Comunidade"
          variant="outlined"
          density="comfortable"
          hide-details="auto"
          @update:model-value="emit('update:snmpCommunity', String($event || ''))"
        ></v-text-field>
      </v-col>
      <v-col cols="12" md="4">
        <v-text-field
          :model-value="snmpPort"
          label="Porta SNMP"
          type="number"
          min="1"
          max="65535"
          variant="outlined"
          density="comfortable"
          hide-details="auto"
          @update:model-value="emit('update:snmpPort', Number($event) || 161)"
        ></v-text-field>
      </v-col>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { MonitorKind, SnmpVersion } from '@/utils/monitorTypes'
import PingFields from './PingFields.vue'

defineProps<{
  kind: MonitorKind
  packetCount: number
  acceptedStatusCodes: number[]
  validateCertificate: boolean
  extraHostnames: string[]
  dnsWarningThresholdMs: number | null
  snmpVersion: SnmpVersion
  snmpCommunity: string
  snmpPort: number
}>()

const emit = defineEmits<{
  (e: 'update:packetCount', value: number): void
  (e: 'statusCodesChange', value: unknown): void
  (e: 'update:validateCertificate', value: boolean): void
  (e: 'extraHostnamesChange', value: unknown): void
  (e: 'update:dnsWarningThresholdMs', value: number | null): void
  (e: 'update:snmpVersion', value: SnmpVersion): void
  (e: 'update:snmpCommunity', value: string): void
  (e: 'update:snmpPort', value: number): void
}>()
</script>

<template>
  <div class="w-100 d-contents">
    <v-col cols="12" md="6">
      <v-select
        :model-value="snmpMode"
        :items="SNMP_MODES"
        item-title="label"
        item-value="value"
        label="O que coletar via SNMP"
        prepend-inner-icon="mdi-format-list-bulleted-type"
        variant="outlined"
        density="comfortable"
        :hint="snmpModeDescription"
        persistent-hint
        @update:model-value="emit('update:snmpMode', $event)"
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

    <v-col v-if="snmpMode === 'interface' || snmpMode === 'interface_traffic'" cols="12" md="6">
      <v-select
        v-if="interfaceItems.length > 0"
        :model-value="ifIndex"
        :items="interfaceItems"
        item-title="title"
        item-value="value"
        label="Interface monitorada *"
        prepend-inner-icon="mdi-ethernet"
        variant="outlined"
        density="comfortable"
        :loading="loadingInterfaces || scanningSnmp"
        :rules="[(v: unknown) => v !== null || 'Selecione a interface']"
        hint="Interfaces descobertas no último scan SNMP do dispositivo"
        persistent-hint
        @update:model-value="emit('interfaceSelected', $event)"
      >
        <template #item="{ props: itemProps, item }">
          <v-list-item v-bind="itemProps" :subtitle="itemField(item, 'subtitle')"></v-list-item>
        </template>
        <template #append-inner>
          <v-btn
            icon
            size="x-small"
            variant="text"
            density="comfortable"
            :loading="scanningSnmp"
            :disabled="!hasTargetOrDevice"
            @click.stop="emit('runSnmpScan')"
          >
            <v-icon size="18">mdi-radar</v-icon>
            <v-tooltip activator="parent" location="top"> Escanear interfaces via SNMP </v-tooltip>
          </v-btn>
        </template>
      </v-select>

      <v-card
        v-else
        variant="outlined"
        class="pa-3 rounded-lg border-dashed d-flex align-center justify-space-between ga-2 flex-wrap"
        style="min-height: 48px"
      >
        <div class="d-flex align-center ga-2 text-caption text-medium-emphasis">
          <v-icon size="20" color="warning">mdi-information-outline</v-icon>
          <span>Nenhuma interface descoberta para este dispositivo.</span>
        </div>
        <v-btn
          color="primary"
          variant="tonal"
          height="36"
          size="small"
          :loading="scanningSnmp || loadingInterfaces"
          :disabled="!hasTargetOrDevice"
          @click="emit('runSnmpScan')"
        >
          <v-icon start size="16">mdi-radar</v-icon>
          Escanear SNMP
        </v-btn>
      </v-card>
    </v-col>

    <v-col v-if="snmpMode === 'interface_traffic' && ifIndex !== null" cols="12">
      <v-card color="warning" variant="tonal" class="pa-4 rounded-lg">
        <div class="d-flex align-center justify-space-between mb-2">
          <div class="d-flex align-center ga-2">
            <v-icon size="20">mdi-bell-ring-outline</v-icon>
            <span class="font-weight-bold text-subtitle-1">Regra de alerta de tráfego</span>
          </div>
          <v-switch
            :model-value="trafficAlertEnabled"
            color="warning"
            density="compact"
            hide-details
            @update:model-value="emit('update:trafficAlertEnabled', $event === true)"
          ></v-switch>
        </div>

        <v-row v-if="trafficAlertEnabled" dense class="mt-2">
          <v-col cols="12" md="4">
            <v-select
              :model-value="trafficAlertDirection"
              :items="[
                { title: 'Entrada (Download / inBps)', value: 'inBps' },
                { title: 'Saída (Upload / outBps)', value: 'outBps' },
              ]"
              item-title="title"
              item-value="value"
              label="Direção *"
              variant="outlined"
              density="comfortable"
              hide-details="auto"
              bg-color="surface"
              @update:model-value="emit('update:trafficAlertDirection', $event)"
            ></v-select>
          </v-col>
          <v-col cols="12" md="4">
            <v-select
              :model-value="trafficAlertOperator"
              :items="[
                { title: 'Avisar quando passar de (>)', value: 'gt' },
                { title: 'Avisar quando for menor que (<)', value: 'lt' },
              ]"
              item-title="title"
              item-value="value"
              label="Condição *"
              variant="outlined"
              density="comfortable"
              hide-details="auto"
              bg-color="surface"
              @update:model-value="emit('update:trafficAlertOperator', $event)"
            ></v-select>
          </v-col>
          <v-col cols="12" md="4">
            <DataRateInput
              :model-value="trafficAlertValueBps"
              label="Limite de tráfego *"
              density="comfortable"
              :rules="[(v: unknown) => Number(v) > 0 || 'Informe um valor maior que 0']"
              hide-details="auto"
              bg-color="surface"
              @update:model-value="emit('update:trafficAlertValueBps', $event)"
            ></DataRateInput>
          </v-col>
          <v-col cols="12" md="6" class="mt-2">
            <v-select
              :model-value="trafficAlertDurationSeconds"
              :items="ALERT_DURATIONS"
              item-title="title"
              item-value="value"
              label="Tolerância antes de disparar"
              variant="outlined"
              density="comfortable"
              hide-details="auto"
              bg-color="surface"
              @update:model-value="emit('update:trafficAlertDurationSeconds', $event)"
            ></v-select>
          </v-col>
          <v-col cols="12" md="6" class="mt-2">
            <v-select
              :model-value="trafficAlertSeverity"
              :items="ALERT_SEVERITIES"
              item-title="title"
              item-value="value"
              label="Nível de severidade"
              variant="outlined"
              density="comfortable"
              hide-details="auto"
              bg-color="surface"
              @update:model-value="emit('update:trafficAlertSeverity', $event)"
            ></v-select>
          </v-col>
        </v-row>
        <div v-else class="text-body-2 mt-1">
          Ative a opção acima para ser notificado automaticamente quando o tráfego desta interface
          atingir o limite configurado em Mbps.
        </div>
      </v-card>
    </v-col>
  </div>
</template>

<script setup lang="ts">
import { SNMP_MODES, type SnmpMode } from '@/utils/monitorTypes'
import { ALERT_DURATIONS, ALERT_SEVERITIES } from '@/utils/alertPresentation'
import DataRateInput from '@/components/DataRateInput.vue'

defineProps<{
  snmpMode: SnmpMode
  ifIndex: number | null
  snmpModeDescription: string
  interfaceItems: Array<{ title: string; value: number; subtitle: string }>
  loadingInterfaces: boolean
  scanningSnmp: boolean
  hasTargetOrDevice: boolean
  trafficAlertEnabled: boolean
  trafficAlertDirection: 'inBps' | 'outBps'
  trafficAlertOperator: 'gt' | 'lt'
  trafficAlertValueBps: number | null
  trafficAlertDurationSeconds: number
  trafficAlertSeverity: 'info' | 'warning' | 'critical'
}>()

const emit = defineEmits<{
  (e: 'update:snmpMode', value: SnmpMode): void
  (e: 'interfaceSelected', value: number): void
  (e: 'runSnmpScan'): void
  (e: 'update:trafficAlertEnabled', value: boolean): void
  (e: 'update:trafficAlertDirection', value: 'inBps' | 'outBps'): void
  (e: 'update:trafficAlertOperator', value: 'gt' | 'lt'): void
  (e: 'update:trafficAlertValueBps', value: number | null): void
  (e: 'update:trafficAlertDurationSeconds', value: number): void
  (e: 'update:trafficAlertSeverity', value: 'info' | 'warning' | 'critical'): void
}>()

function itemField(item: unknown, field: string): string | undefined {
  if (!item || typeof item !== 'object') return undefined
  const raw = (item as { raw?: Record<string, unknown> }).raw
  if (raw && typeof raw === 'object' && field in raw) {
    const val = raw[field]
    return typeof val === 'string' ? val : undefined
  }
  if (field in item) {
    const val = (item as Record<string, unknown>)[field]
    return typeof val === 'string' ? val : undefined
  }
  return undefined
}
</script>

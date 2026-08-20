<template>
  <div class="w-100 d-contents">
    <v-col cols="12" md="6">
      <v-select
        :model-value="dnsProtocolValue"
        :items="DNS_PROTOCOLS"
        item-title="label"
        item-value="value"
        label="Transporte da consulta"
        prepend-inner-icon="mdi-transit-connection"
        variant="outlined"
        density="comfortable"
        :hint="protocolDefinition.description"
        persistent-hint
        @update:model-value="emit('update:dnsProtocol', $event)"
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

    <v-col cols="12" md="6">
      <v-select
        :model-value="recordType"
        :items="DNS_RECORD_TYPES"
        item-title="title"
        item-value="value"
        label="Tipo de registro"
        prepend-inner-icon="mdi-file-document-outline"
        variant="outlined"
        density="comfortable"
        hide-details="auto"
        @update:model-value="emit('update:recordType', $event)"
      >
        <template #item="{ props: itemProps, item }">
          <v-list-item v-bind="itemProps" :subtitle="itemField(item, 'subtitle')"></v-list-item>
        </template>
      </v-select>
    </v-col>

    <v-col v-if="protocolDefinition.requiresServer" cols="12">
      <div class="d-flex align-start ga-2">
        <v-combobox
          :model-value="dnsServer"
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
          @update:model-value="emit('serverChanged', $event)"
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
          @click="emit('openServersDialog')"
        >
          <v-icon>mdi-cog-outline</v-icon>
          <v-tooltip activator="parent" location="top">Gerenciar servidores DNS</v-tooltip>
        </v-btn>
      </div>
    </v-col>

    <v-col v-if="protocolDefinition.requiresEndpoint" cols="12">
      <div class="d-flex align-start ga-2">
        <v-combobox
          :model-value="dohUrl"
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
          @update:model-value="emit('dohChanged', $event)"
        >
          <template #item="{ props: itemProps, item }">
            <v-list-item v-bind="itemProps" :subtitle="itemField(item, 'subtitle')"></v-list-item>
          </template>
        </v-combobox>
        <v-btn
          icon
          variant="tonal"
          color="deep-purple"
          density="comfortable"
          class="mt-1"
          @click="emit('openServersDialog')"
        >
          <v-icon>mdi-cog-outline</v-icon>
          <v-tooltip activator="parent" location="top">Gerenciar servidores DNS</v-tooltip>
        </v-btn>
      </div>
    </v-col>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  DNS_PROTOCOLS,
  DNS_RECORD_TYPES,
  dnsProtocol,
  type DnsProtocol,
  type DnsRecordType,
} from '@/utils/monitorTypes'
import { useDnsServersStore } from '@/stores/dnsServers'

const props = defineProps<{
  dnsProtocolValue: DnsProtocol
  recordType: DnsRecordType
  dnsServer: string
  dohUrl: string
  dnsServerRule: (val: unknown) => true | string
  dohUrlRule: (val: unknown) => true | string
  dnsServerHint: string
  dohEndpointHint: string
}>()

const emit = defineEmits<{
  (e: 'update:dnsProtocol', value: DnsProtocol): void
  (e: 'update:recordType', value: DnsRecordType): void
  (e: 'serverChanged', value: unknown): void
  (e: 'dohChanged', value: unknown): void
  (e: 'openServersDialog'): void
}>()

const dnsServersStore = useDnsServersStore()

const protocolDefinition = computed(() => dnsProtocol(props.dnsProtocolValue))

const dnsServerItems = computed(() =>
  dnsServersStore.servers
    .filter((server) => server.protocol === props.dnsProtocolValue)
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
      subtitle: server.description ? `${server.name} · ${server.description}` : server.name,
      icon: 'mdi-lock-outline',
    }))
)

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

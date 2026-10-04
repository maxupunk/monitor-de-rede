<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 760"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="host" rounded="lg">
      <v-card-title class="d-flex align-center ga-3 pa-4">
        <v-avatar :color="type.color" variant="tonal" size="44" rounded="lg">
          <v-icon :icon="type.icon" size="26"></v-icon>
        </v-avatar>
        <div class="flex-grow-1 min-w-0">
          <div class="text-h6 font-weight-bold text-truncate">{{ name }}</div>
          <div class="text-body-2 font-mono text-primary">{{ host.ipAddress }}</div>
        </div>
        <v-btn icon="mdi-close" variant="text" aria-label="Fechar" @click="close"></v-btn>
      </v-card-title>
      <v-divider></v-divider>

      <v-card-text class="pa-4">
        <div class="d-flex flex-wrap ga-2 mb-4">
          <v-chip :color="type.color" variant="flat" :prepend-icon="type.icon">
            {{ type.label }}
          </v-chip>
          <v-chip :color="confidenceColor" variant="tonal" prepend-icon="mdi-gauge">
            Confiança {{ formatPercent(host.confidence, 0) }}
          </v-chip>
          <v-chip
            v-if="isGateway"
            color="primary"
            variant="tonal"
            prepend-icon="mdi-router-network"
          >
            Gateway da rede
          </v-chip>
          <v-chip
            :color="added ? 'success' : 'warning'"
            variant="tonal"
            :prepend-icon="added ? 'mdi-check-circle' : 'mdi-clock-outline'"
          >
            {{ added ? 'Já cadastrado' : 'Ainda não cadastrado' }}
          </v-chip>
        </div>

        <!-- Por que a descoberta acha que é este tipo -->
        <v-card variant="tonal" color="info" rounded="lg" class="pa-3 mb-4">
          <div class="text-subtitle-2 font-weight-bold d-flex align-center ga-2 mb-1">
            <v-icon size="18">mdi-lightbulb-on-outline</v-icon>
            Por que classificamos assim
          </div>
          <ul v-if="reasons.length > 0" class="text-body-2 ps-5">
            <li v-for="reason in reasons" :key="reason">{{ reason }}</li>
          </ul>
          <div v-else class="text-body-2">
            Nenhuma evidência suficiente — o aparelho não se apresentou por nenhum protocolo.
          </div>
          <div v-if="alternative" class="text-body-2 mt-2">
            Também pode ser: <strong>{{ alternative.label }}</strong>
          </div>
          <LayaSuggestionChip
            :suggestion="laya?.deviceType ?? null"
            :format-label="deviceTypeLabel"
            class="mt-2"
          />
        </v-card>

        <div class="text-subtitle-2 font-weight-bold mb-2">Identidade</div>
        <v-row dense class="mb-3">
          <v-col v-for="fact in facts" :key="fact.label" cols="12" sm="6">
            <div class="fact pa-2 rounded">
              <div class="text-caption font-weight-medium">{{ fact.label }}</div>
              <div
                class="text-body-2 font-weight-bold text-break"
                :class="{ 'font-mono': fact.mono }"
              >
                {{ fact.value }}
              </div>
            </div>
          </v-col>
        </v-row>

        <v-alert
          v-if="identity"
          :type="identity.source === 'snmp' ? 'success' : 'info'"
          variant="tonal"
          density="compact"
          class="mb-4"
        >
          <div class="font-weight-bold">{{ identity.reason || 'Identificado por SNMP' }}</div>
          <div v-if="identity.sysDescr" class="text-body-2">sysDescr: {{ identity.sysDescr }}</div>
          <div v-if="identity.sysObjectId" class="text-body-2 font-mono">
            sysObjectID: {{ identity.sysObjectId }}
          </div>
        </v-alert>

        <template v-if="webPage">
          <div class="text-subtitle-2 font-weight-bold mb-2">Página web</div>
          <div class="fact pa-3 rounded mb-4 text-body-2">
            <div>
              <strong>Porta {{ webPage.port }}</strong> · resposta HTTP {{ webPage.status }}
            </div>
            <div v-if="webPage.title">Título: {{ webPage.title }}</div>
            <div v-if="webPage.server">Servidor: {{ webPage.server }}</div>
            <div v-if="webPage.realm">Autenticação: {{ webPage.realm }}</div>
            <div v-if="webPage.location" class="text-break">
              Redireciona para: {{ webPage.location }}
            </div>
          </div>
        </template>

        <template v-if="mdnsServices.length > 0">
          <div class="text-subtitle-2 font-weight-bold mb-2">Serviços anunciados (mDNS)</div>
          <div class="d-flex flex-wrap ga-2 mb-4">
            <v-chip
              v-for="service in mdnsServices"
              :key="service"
              size="small"
              color="secondary"
              variant="tonal"
              class="font-mono"
            >
              {{ service }}
            </v-chip>
          </div>
        </template>

        <template v-if="ports.length > 0">
          <div class="text-subtitle-2 font-weight-bold mb-2">
            Portas abertas ({{ ports.length }})
          </div>
          <div class="d-flex flex-wrap ga-2 mb-4">
            <v-chip v-for="port in ports" :key="port" size="small" color="success" variant="tonal">
              <strong>{{ port }}</strong>
              <span v-if="portLabel(port)" class="ms-1">{{ portLabel(port) }}</span>
            </v-chip>
          </div>
        </template>

        <v-expansion-panels variant="accordion">
          <v-expansion-panel>
            <v-expansion-panel-title>
              <v-icon size="18" class="me-2" color="primary">mdi-code-json</v-icon>
              Dados brutos da descoberta
            </v-expansion-panel-title>
            <v-expansion-panel-text>
              <div class="d-flex justify-end mb-2">
                <v-btn
                  size="small"
                  variant="tonal"
                  color="primary"
                  :prepend-icon="copied ? 'mdi-check' : 'mdi-content-copy'"
                  @click="copyJson"
                >
                  {{ copied ? 'Copiado' : 'Copiar JSON' }}
                </v-btn>
              </div>
              <pre class="raw-json font-mono text-body-2">{{ rawJson }}</pre>
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="pa-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" color="primary" @click="close">Fechar</v-btn>
        <v-btn
          v-if="!added"
          color="success"
          variant="flat"
          prepend-icon="mdi-plus"
          @click="emit('add', host)"
        >
          Adicionar ao inventário
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import LayaSuggestionChip from '@/components/ai/LayaSuggestionChip.vue'
import type { StreamedDiscoveryHost } from '@/stores/discovery'
import { deviceTypeLabel } from '@/utils/deviceTypes'
import {
  discoveryAlternative,
  discoveryConfidenceColor,
  discoveryDescription,
  discoveryDeviceName,
  discoveryIdentity,
  discoveryLaya,
  discoveryMdns,
  discoveryNetbios,
  discoveryOpenPorts,
  discoveryPortLabel,
  discoveryReasons,
  discoverySources,
  discoveryTypeMeta,
  discoveryUpnp,
  discoveryVendor,
  discoveryWebPage,
} from '@/utils/discoveryPresentation'
import { formatPercent } from '@/utils/formatters'

const props = defineProps<{
  modelValue: boolean
  host: StreamedDiscoveryHost | null
  added: boolean
  isGateway: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  add: [host: StreamedDiscoveryHost]
}>()

const copied = ref(false)

const type = computed(() => discoveryTypeMeta(props.host))
const name = computed(() => discoveryDeviceName(props.host) ?? type.value.label)
const identity = computed(() => discoveryIdentity(props.host))
const webPage = computed(() => discoveryWebPage(props.host))
const laya = computed(() => discoveryLaya(props.host))
const reasons = computed(() => discoveryReasons(props.host))
const alternative = computed(() => discoveryAlternative(props.host))
const ports = computed(() => discoveryOpenPorts(props.host))
const mdnsServices = computed(() => discoveryMdns(props.host)?.services ?? [])
const confidenceColor = computed(() => discoveryConfidenceColor(props.host?.confidence))
const rawJson = computed(() => JSON.stringify(props.host ?? {}, null, 2))

/** Só o que se sabe — campo vazio não ocupa espaço na tela. */
const facts = computed(() => {
  const host = props.host
  if (!host) return []
  const netbios = discoveryNetbios(host)
  const upnp = discoveryUpnp(host)
  const sources = discoverySources(host)
  const entries: { label: string; value: string | null | undefined; mono?: boolean }[] = [
    { label: 'Endereço MAC', value: host.macAddress, mono: true },
    { label: 'Fabricante', value: discoveryVendor(host) },
    { label: 'Modelo / descrição', value: discoveryDescription(host) },
    { label: 'Sistema', value: identity.value?.label },
    { label: 'Nome DNS', value: host.hostname, mono: true },
    { label: 'Nome mDNS', value: host.mdnsName, mono: true },
    { label: 'Nome UPnP', value: upnp?.friendlyName },
    {
      label: 'NetBIOS',
      value: netbios?.name
        ? [netbios.name, netbios.workgroup].filter(Boolean).join(' · grupo ')
        : null,
    },
    { label: 'Visto por', value: sources.length > 0 ? sources.join(', ') : null },
  ]
  return entries.filter((entry): entry is { label: string; value: string; mono?: boolean } =>
    Boolean(entry.value)
  )
})

function portLabel(port: number): string | null {
  return discoveryPortLabel(props.host, port)
}

function close() {
  emit('update:modelValue', false)
}

async function copyJson() {
  try {
    await navigator.clipboard.writeText(rawJson.value)
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 2000)
  } catch {
    // Sem permissão de área de transferência: o JSON continua visível.
  }
}
</script>

<style scoped>
.fact {
  background: rgba(var(--v-theme-on-surface), 0.04);
  border: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  color: rgb(var(--v-theme-on-surface));
}

.raw-json {
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 320px;
  overflow: auto;
  margin: 0;
  padding: 12px;
  border-radius: 8px;
  background: rgba(var(--v-theme-on-surface), 0.05);
  color: rgb(var(--v-theme-on-surface));
}

.min-w-0 {
  min-width: 0;
}
</style>

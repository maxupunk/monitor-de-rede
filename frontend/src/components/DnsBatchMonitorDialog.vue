<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 860"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <!-- Cabeçalho do Modal -->
      <v-card-item class="pa-5 pb-3">
        <template #prepend>
          <v-avatar color="deep-purple" size="44" rounded="lg" variant="tonal">
            <v-icon size="24">mdi-dns-outline</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-h6">
          Monitorar Servidores DNS em Lote
        </v-card-title>
        <v-card-subtitle>
          Selecione os resolvedores DNS para monitoramento contínuo e acompanhamento no histórico do
          dashboard.
        </v-card-subtitle>
      </v-card-item>

      <v-divider></v-divider>

      <!-- Barra de Filtros e Ações Rápidas -->
      <div class="px-5 py-3 bg-surface-light border-b">
        <div
          class="d-flex flex-column flex-sm-row align-start align-sm-center justify-space-between ga-3"
        >
          <div class="d-flex align-center ga-2 flex-wrap">
            <v-btn
              size="small"
              variant="tonal"
              color="deep-purple"
              :disabled="selectableResolvers.length === 0"
              @click="toggleSelectAll"
            >
              <v-icon start size="16">
                {{ isAllSelected ? 'mdi-checkbox-blank-outline' : 'mdi-checkbox-multiple-marked' }}
              </v-icon>
              {{ isAllSelected ? 'Desmarcar Todos' : 'Selecionar Não Monitorados' }}
            </v-btn>
            <v-btn
              v-if="selectedKeys.length > 0 && !isAllSelected"
              size="small"
              variant="text"
              @click="selectedKeys = []"
            >
              Limpar seleção
            </v-btn>
          </div>

          <v-text-field
            v-model="search"
            placeholder="Filtrar por nome ou IP..."
            prepend-inner-icon="mdi-magnify"
            variant="outlined"
            density="compact"
            hide-details
            clearable
            style="min-width: 220px; max-width: 320px"
          ></v-text-field>
        </div>
      </div>

      <v-card-text class="pa-5">
        <v-alert
          v-if="dnsStore.error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-4"
          :text="dnsStore.error"
        ></v-alert>

        <!-- Etapa 1: Lista de Resolvedores DNS -->
        <div class="d-flex align-center justify-space-between mb-2">
          <div class="text-overline text-medium-emphasis">
            1 · Selecione os Resolvedores ({{ selectedKeys.length }} selecionado{{
              selectedKeys.length === 1 ? '' : 's'
            }})
          </div>
          <span class="text-caption text-grey">
            {{ filteredResolvers.length }} disponível{{
              filteredResolvers.length === 1 ? '' : 'is'
            }}
          </span>
        </div>

        <v-row dense class="mb-4">
          <v-col v-for="item in filteredResolvers" :key="item.key" cols="12" sm="6">
            <v-card
              variant="outlined"
              class="h-100 pa-3 rounded-lg transition-all resolver-card cursor-pointer"
              :class="{
                'border-deep-purple bg-deep-purple-lighten-5': isSelected(item),
                'opacity-75': item.isMonitored,
              }"
              @click="toggleItem(item)"
            >
              <div class="d-flex align-center justify-space-between ga-2">
                <div class="d-flex align-center ga-3 overflow-hidden">
                  <v-checkbox-btn
                    :model-value="isSelected(item)"
                    color="deep-purple"
                    density="compact"
                    class="ma-0 pa-0 flex-shrink-0"
                    @click.stop="toggleItem(item)"
                  ></v-checkbox-btn>

                  <v-avatar
                    :color="protocolColor(item.protocol)"
                    size="36"
                    rounded="lg"
                    variant="tonal"
                    class="flex-shrink-0"
                  >
                    <v-icon size="20">{{ protocolIcon(item.protocol) }}</v-icon>
                  </v-avatar>

                  <div class="overflow-hidden">
                    <div class="font-weight-bold text-subtitle-2 text-truncate" :title="item.name">
                      {{ item.name }}
                    </div>
                    <div
                      class="text-caption text-medium-emphasis text-truncate font-family-monospace"
                      :title="item.server"
                    >
                      {{ item.server }}
                    </div>
                  </div>
                </div>

                <div class="d-flex flex-column align-end ga-1 flex-shrink-0">
                  <v-chip
                    v-if="item.isMonitored"
                    size="x-small"
                    color="success"
                    variant="tonal"
                    class="font-weight-medium"
                  >
                    <v-icon start size="12">mdi-check-circle</v-icon>
                    Monitorado
                  </v-chip>
                  <v-chip v-else size="x-small" variant="outlined" color="grey">
                    {{ item.protocol.toUpperCase() }}
                  </v-chip>
                </div>
              </div>
            </v-card>
          </v-col>
        </v-row>

        <!-- Etapa 2: Configurações do Monitoramento -->
        <div class="text-overline text-medium-emphasis mt-6 mb-2">
          2 · Configuração dos Testes DNS
        </div>

        <v-sheet border rounded class="pa-4 bg-surface">
          <v-row dense>
            <v-col cols="12" md="6">
              <v-text-field
                v-model="targetDomain"
                label="Domínio Principal de Consulta *"
                placeholder="Ex: google.com"
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-web"
                hint="Domínio consultado periodicamente para medir a latência"
                persistent-hint
                hide-details="auto"
                class="mb-2"
              ></v-text-field>
            </v-col>

            <v-col cols="12" md="6">
              <v-combobox
                v-model="extraHostnames"
                label="Domínios Adicionais (opcional)"
                placeholder="Digite e pressione Enter"
                chips
                multiple
                closable-chips
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-format-list-bulleted"
                hint="Mede múltiplos domínios e tira a média em cada verificação"
                persistent-hint
                hide-details="auto"
                class="mb-2"
              ></v-combobox>
            </v-col>

            <v-col cols="12" sm="6" md="4">
              <v-select
                v-model="recordType"
                :items="['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS']"
                label="Tipo de Registro"
                variant="outlined"
                density="comfortable"
                hide-details="auto"
              ></v-select>
            </v-col>

            <v-col cols="12" sm="6" md="4">
              <v-select
                v-model="intervalSeconds"
                :items="INTERVAL_OPTIONS"
                item-title="label"
                item-value="value"
                label="Intervalo de Checagem"
                variant="outlined"
                density="comfortable"
                hide-details="auto"
              ></v-select>
            </v-col>

            <v-col cols="12" md="4" class="d-flex align-center">
              <v-switch
                v-model="executeNow"
                color="deep-purple"
                density="compact"
                label="Executar 1ª medição agora"
                inset
                hide-details
                class="ms-2"
              ></v-switch>
            </v-col>
          </v-row>
        </v-sheet>
      </v-card-text>

      <v-divider></v-divider>

      <!-- Rodapé de Ações -->
      <v-card-actions class="pa-4 justify-space-between flex-wrap ga-2">
        <div class="d-flex align-center ga-2">
          <span class="text-caption text-medium-emphasis">
            {{ selectedKeys.length }} servidor{{
              selectedKeys.length === 1 ? '' : 'es'
            }}
            selecionado{{ selectedKeys.length === 1 ? '' : 's' }}
          </span>
        </div>

        <div class="d-flex align-center ga-2">
          <v-btn variant="text" @click="emit('update:modelValue', false)"> Cancelar </v-btn>
          <v-btn
            color="deep-purple"
            variant="flat"
            :loading="dnsStore.provisioning"
            :disabled="selectedKeys.length === 0"
            prepend-icon="mdi-play-circle-outline"
            @click="submit"
          >
            Iniciar Monitoramento ({{ selectedKeys.length }})
          </v-btn>
        </div>
      </v-card-actions>
    </v-card>

    <v-snackbar v-model="snackbar.show" :color="snackbar.color" timeout="4000" location="top">
      {{ snackbar.text }}
    </v-snackbar>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import {
  useDnsPerformanceStore,
  type DnsBatchProvisionRequest,
  type DnsBatchProvisionResponse,
  type DnsBatchProvisionServer,
} from '@/stores/dnsPerformance'
import { useDnsServersStore } from '@/stores/dnsServers'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import type { DnsProtocol } from '@/utils/monitorTypes'

interface ResolverOption {
  key: string
  name: string
  server: string
  protocol: DnsProtocol
  dohUrl?: string
  isMonitored: boolean
}

const props = defineProps<{
  modelValue: boolean
  initialHostnames?: string[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'provisioned', response: DnsBatchProvisionResponse): void
}>()

const dnsStore = useDnsPerformanceStore()
const dnsServersStore = useDnsServersStore()
const monitorsStore = useMonitorsStore()

const search = ref('')
const selectedKeys = ref<string[]>([])
const targetDomain = ref('google.com')
const extraHostnames = ref<string[]>(['cloudflare.com', 'globo.com'])
const recordType = ref('A')
const intervalSeconds = ref(60)
const executeNow = ref(true)

const snackbar = ref({
  show: false,
  text: '',
  color: 'success',
})

const INTERVAL_OPTIONS = [
  { value: 30, label: 'A cada 30 segundos' },
  { value: 60, label: 'A cada 1 minuto (Recomendado)' },
  { value: 120, label: 'A cada 2 minutos' },
  { value: 300, label: 'A cada 5 minutos' },
]

/** Resolvedores públicos curados caso o usuário não tenha cadastrado servidores */
const DEFAULT_PRESET_RESOLVERS: Array<{
  name: string
  server: string
  protocol: DnsProtocol
  dohUrl?: string
}> = [
  { name: 'Cloudflare DNS', server: '1.1.1.1', protocol: 'udp' },
  { name: 'Google Public DNS', server: '8.8.8.8', protocol: 'udp' },
  { name: 'Quad9 DNS', server: '9.9.9.9', protocol: 'udp' },
  { name: 'OpenDNS', server: '208.67.222.222', protocol: 'udp' },
  { name: 'AdGuard DNS', server: '94.140.14.14', protocol: 'udp' },
  {
    name: 'Cloudflare DoH',
    server: 'https://cloudflare-dns.com/dns-query',
    protocol: 'doh',
    dohUrl: 'https://cloudflare-dns.com/dns-query',
  },
  {
    name: 'Google DoH',
    server: 'https://dns.google/dns-query',
    protocol: 'doh',
    dohUrl: 'https://dns.google/dns-query',
  },
]

function monitorDnsServer(monitor: Monitor): { server: string; protocol: string } | null {
  if (monitor.type !== 'dns') return null
  const config = (monitor.configuration || {}) as Record<string, unknown>
  const protocol = String(config.protocol ?? 'udp')
  const server = String(protocol === 'doh' ? (config.dohUrl ?? '') : (config.dnsServer ?? ''))
  return server ? { server, protocol } : null
}

function checkIsMonitored(server: string, protocol: string): boolean {
  return monitorsStore.monitors.some((monitor) => {
    const existing = monitorDnsServer(monitor)
    return existing?.server === server && existing.protocol === protocol
  })
}

const allResolvers = computed<ResolverOption[]>(() => {
  const map = new Map<string, ResolverOption>()

  // 1. Servidores cadastrados no banco
  for (const s of dnsServersStore.servers) {
    const key = `${s.address}|${s.protocol}`
    map.set(key, {
      key,
      name: s.name,
      server: s.address,
      protocol: s.protocol as DnsProtocol,
      dohUrl: s.protocol === 'doh' ? s.address : undefined,
      isMonitored: checkIsMonitored(s.address, s.protocol),
    })
  }

  // 2. Presets públicos padrão (adiciona se ainda não constar)
  for (const preset of DEFAULT_PRESET_RESOLVERS) {
    const key = `${preset.server}|${preset.protocol}`
    if (!map.has(key)) {
      map.set(key, {
        key,
        name: preset.name,
        server: preset.server,
        protocol: preset.protocol,
        dohUrl: preset.dohUrl,
        isMonitored: checkIsMonitored(preset.server, preset.protocol),
      })
    }
  }

  return Array.from(map.values())
})

const filteredResolvers = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return allResolvers.value
  return allResolvers.value.filter(
    (r) =>
      r.name.toLowerCase().includes(query) ||
      r.server.toLowerCase().includes(query) ||
      r.protocol.toLowerCase().includes(query)
  )
})

const selectableResolvers = computed(() => filteredResolvers.value.filter((r) => !r.isMonitored))

const isAllSelected = computed(() => {
  if (selectableResolvers.value.length === 0) return false
  return selectableResolvers.value.every((r) => selectedKeys.value.includes(r.key))
})

function isSelected(item: ResolverOption): boolean {
  return selectedKeys.value.includes(item.key)
}

function toggleItem(item: ResolverOption) {
  const index = selectedKeys.value.indexOf(item.key)
  if (index === -1) {
    selectedKeys.value.push(item.key)
  } else {
    selectedKeys.value.splice(index, 1)
  }
}

function toggleSelectAll() {
  if (isAllSelected.value) {
    selectedKeys.value = []
  } else {
    selectedKeys.value = selectableResolvers.value.map((r) => r.key)
  }
}

function protocolIcon(protocol: DnsProtocol): string {
  if (protocol === 'doh') return 'mdi-lock-outline'
  if (protocol === 'tcp') return 'mdi-transit-connection-variant'
  return 'mdi-lightning-bolt-outline'
}

function protocolColor(protocol: DnsProtocol): string {
  if (protocol === 'doh') return 'indigo'
  if (protocol === 'tcp') return 'cyan'
  return 'deep-purple'
}

watch(
  () => props.modelValue,
  async (isOpen) => {
    if (!isOpen) return
    if (dnsServersStore.servers.length === 0) {
      await dnsServersStore.fetchServers()
    }
    if (monitorsStore.monitors.length === 0) {
      await monitorsStore.fetchMonitors()
    }
    if (props.initialHostnames && props.initialHostnames.length > 0) {
      const [first, ...rest] = props.initialHostnames
      targetDomain.value = first ?? 'google.com'
      extraHostnames.value = rest
    }
    // Seleciona automaticamente os não monitorados por conveniência
    selectedKeys.value = selectableResolvers.value.map((r) => r.key)
  }
)

onMounted(() => {
  if (props.modelValue) {
    dnsServersStore.fetchServers()
    monitorsStore.fetchMonitors()
  }
})

async function submit() {
  if (selectedKeys.value.length === 0) return

  const selectedResolvers = allResolvers.value.filter((r) => selectedKeys.value.includes(r.key))

  const servers: DnsBatchProvisionServer[] = selectedResolvers.map((r) => ({
    name: r.name,
    server: r.server,
    protocol: r.protocol,
    dohUrl: r.dohUrl,
  }))

  const payload: DnsBatchProvisionRequest = {
    servers,
    domain: targetDomain.value.trim() || 'google.com',
    domains: extraHostnames.value.map((h) => h.trim()).filter(Boolean),
    recordType: recordType.value,
    intervalSeconds: intervalSeconds.value,
    executeNow: executeNow.value,
  }

  const result = await dnsStore.provisionMonitors(payload)
  if (result) {
    snackbar.value = {
      show: true,
      text: `${result.createdCount} monitor(es) DNS provisionado(s) com sucesso!`,
      color: 'success',
    }
    await monitorsStore.fetchMonitors()
    emit('provisioned', result)
    emit('update:modelValue', false)
  }
}
</script>

<style scoped>
.resolver-card {
  transition: all 0.2s ease-in-out;
}
.resolver-card:hover {
  border-color: rgba(103, 58, 183, 0.5) !important;
}
</style>

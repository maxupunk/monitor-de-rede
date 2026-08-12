<template>
  <div>
    <PageHeader
      title="Servidor VPN (WireGuard)"
      subtitle="Túnel para monitorar roteadores MikroTik e OpenWrt fora da rede local"
    >
      <template #actions>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-lan-connect"
          :to="{ name: 'vpn-devices' }"
        >
          <span class="hidden-sm-and-down">Dispositivos VPN</span>
          <span class="hidden-md-and-up">Dispositivos</span>
        </v-btn>
      </template>
    </PageHeader>

    <v-row>
      <!-- Estado do serviço -->
      <v-col cols="12" md="4">
        <v-card elevation="2" class="rounded-lg h-100">
          <v-card-item>
            <v-card-title class="text-subtitle-1 font-weight-bold">Estado do serviço</v-card-title>
          </v-card-item>
          <v-card-text>
            <div class="d-flex align-center mb-4">
              <v-chip :color="serviceColor" variant="flat" class="font-weight-bold">
                <v-icon start size="14">mdi-circle</v-icon>
                {{ serviceLabel }}
              </v-chip>
            </div>

            <v-list density="compact" class="bg-transparent">
              <v-list-item
                prepend-icon="mdi-lan-connect"
                :title="`${vpnStore.state?.peersConnected ?? 0} de ${vpnStore.state?.peersTotal ?? 0} conectados`"
                subtitle="Peers com handshake recente"
              ></v-list-item>
              <v-list-item
                prepend-icon="mdi-swap-vertical"
                :title="`${formatBytes(vpnStore.state?.bytesRx ?? 0)} ↓ / ${formatBytes(vpnStore.state?.bytesTx ?? 0)} ↑`"
                subtitle="Tráfego agregado do túnel"
              ></v-list-item>
              <v-list-item
                prepend-icon="mdi-ip-network"
                :title="vpnStore.state?.serverAddress || '—'"
                subtitle="Endereço do NetMonitor na VPN"
              ></v-list-item>
              <v-list-item
                prepend-icon="mdi-key-chain"
                :title="vpnStore.state?.server?.publicKey || 'Chave ainda não gerada'"
                subtitle="Chave pública do servidor"
                class="text-truncate"
              ></v-list-item>
            </v-list>
          </v-card-text>
        </v-card>
      </v-col>

      <!-- Teste de pré-voo -->
      <v-col cols="12" md="8">
        <v-card elevation="2" class="rounded-lg h-100">
          <v-card-item>
            <v-card-title class="text-subtitle-1 font-weight-bold">
              <v-icon start color="primary">mdi-radar</v-icon>
              Teste de pré-voo
            </v-card-title>
            <v-card-subtitle>
              Antes de configurar roteadores, confirme que o servidor pode receber conexões UDP.
            </v-card-subtitle>
          </v-card-item>

          <v-card-text>
            <v-btn
              color="primary"
              variant="flat"
              prepend-icon="mdi-access-point-check"
              :loading="vpnStore.testing"
              @click="runPreflight"
            >
              Testar acessibilidade externa
            </v-btn>

            <v-alert
              v-if="vpnStore.preflight"
              :type="vpnStore.preflight.level"
              variant="tonal"
              class="mt-4"
              density="comfortable"
            >
              <div class="font-weight-bold mb-1">{{ vpnStore.preflight.message }}</div>
              <div class="text-body-2">{{ vpnStore.preflight.recommendation }}</div>
              <div v-if="vpnStore.preflight.publicIp" class="text-caption mt-2">
                IP público detectado: {{ vpnStore.preflight.publicIp }}
              </div>
              <div v-if="!vpnStore.preflight.verified" class="text-caption mt-1">
                Diagnóstico baseado no endereço público e nas interfaces locais — a confirmação
                definitiva ocorre quando o primeiro roteador fecha o túnel.
              </div>
            </v-alert>
          </v-card-text>
        </v-card>
      </v-col>

      <!-- Configuração -->
      <v-col cols="12">
        <v-card elevation="2" class="rounded-lg">
          <v-card-item>
            <v-card-title class="text-subtitle-1 font-weight-bold">Configuração</v-card-title>
          </v-card-item>

          <v-card-text>
            <v-row>
              <v-col cols="12" md="6">
                <v-text-field
                  v-model="form.publicEndpoint"
                  label="Endereço público (IP ou DDNS)"
                  variant="outlined"
                  density="comfortable"
                  append-inner-icon="mdi-crosshairs-gps"
                  hint="Usado como Endpoint nos scripts dos equipamentos"
                  persistent-hint
                  @click:append-inner="detectEndpoint"
                ></v-text-field>
              </v-col>

              <v-col cols="12" md="3">
                <v-text-field
                  v-model.number="form.listenPort"
                  label="Porta UDP"
                  type="number"
                  variant="outlined"
                  density="comfortable"
                ></v-text-field>
              </v-col>

              <v-col cols="12" md="3">
                <v-text-field
                  v-model="form.cidr"
                  label="Sub-rede da VPN (CIDR)"
                  variant="outlined"
                  density="comfortable"
                  :disabled="vpnStore.isConfigured && (vpnStore.state?.peersTotal ?? 0) > 0"
                  hint="Bloqueado após existirem peers"
                  persistent-hint
                ></v-text-field>
              </v-col>

              <v-col cols="12" md="3">
                <v-text-field
                  v-model.number="form.mtu"
                  label="MTU"
                  type="number"
                  variant="outlined"
                  density="comfortable"
                ></v-text-field>
              </v-col>

              <v-col cols="12" md="9">
                <v-text-field
                  v-model="form.dnsServers"
                  label="Servidores DNS (opcional)"
                  variant="outlined"
                  density="comfortable"
                ></v-text-field>
              </v-col>

              <v-col cols="12">
                <v-switch
                  v-model="form.allowPeerToPeer"
                  color="primary"
                  density="comfortable"
                  hide-details
                  label="Permitir que os dispositivos VPN se enxerguem entre si"
                ></v-switch>
                <div class="text-caption text-grey-darken-1 ml-1">
                  Desligado (recomendado): cada roteador fala apenas com o NetMonitor, então um
                  equipamento comprometido não alcança os demais.
                </div>
              </v-col>
            </v-row>

            <v-alert
              v-if="vpnStore.error"
              type="error"
              variant="tonal"
              class="mt-4"
              density="comfortable"
            >
              {{ vpnStore.error }}
            </v-alert>

            <v-alert
              v-if="savedMessage"
              type="success"
              variant="tonal"
              class="mt-4"
              density="comfortable"
            >
              {{ savedMessage }}
            </v-alert>
          </v-card-text>

          <v-card-actions class="px-4 pb-4">
            <v-spacer></v-spacer>
            <v-btn
              color="primary"
              variant="flat"
              size="large"
              prepend-icon="mdi-content-save-check"
              :loading="vpnStore.saving"
              @click="save"
            >
              Salvar e aplicar
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useVpnStore } from '@/stores/vpn'
import { formatBytes } from '@/utils/formatters'
import PageHeader from '@/components/PageHeader.vue'

const vpnStore = useVpnStore()
const savedMessage = ref<string | null>(null)

const form = reactive({
  publicEndpoint: '',
  listenPort: 51820,
  cidr: '10.8.0.0/24',
  mtu: 1420,
  dnsServers: '',
  allowPeerToPeer: false,
})

const serviceColor = computed(() => {
  if (!vpnStore.isConfigured) return 'grey'
  return (vpnStore.state?.peersConnected ?? 0) > 0 ? 'success' : 'warning'
})

const serviceLabel = computed(() => {
  if (!vpnStore.isConfigured) return 'NÃO CONFIGURADO'
  return (vpnStore.state?.peersConnected ?? 0) > 0 ? 'ATIVO' : 'AGUARDANDO CONEXÕES'
})

watch(
  () => vpnStore.state,
  (state) => {
    if (!state?.server) return
    form.publicEndpoint = state.server.publicEndpoint || ''
    form.listenPort = state.server.listenPort
    form.cidr = state.cidr || form.cidr
    form.mtu = state.server.mtu
    form.dnsServers = state.server.dnsServers || ''
    form.allowPeerToPeer = state.server.allowPeerToPeer
  }
)

onMounted(async () => {
  await vpnStore.fetchServer()
})

async function detectEndpoint() {
  const detected = await vpnStore.detectEndpoint()
  if (detected) form.publicEndpoint = detected
}

async function runPreflight() {
  await vpnStore.runPreflight()
}

async function save() {
  savedMessage.value = null
  const success = await vpnStore.saveServer({
    publicEndpoint: form.publicEndpoint || null,
    listenPort: form.listenPort,
    cidr: form.cidr,
    mtu: form.mtu,
    dnsServers: form.dnsServers || null,
    allowPeerToPeer: form.allowPeerToPeer,
  })

  if (success) {
    savedMessage.value = 'Configuração aplicada sem derrubar os túneis ativos.'
  }
}
</script>

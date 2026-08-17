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
            <!--
              A divergência precisa aparecer aqui, e não virar um detalhe que só
              se descobre no equipamento: este campo é a origem do endereço
              "Internet" da lista de Endereços do servidor, e enquanto os dois
              discordam o syslog aponta para um lado e os peers para o outro.
            -->
            <v-alert
              v-if="enderecoDivergente"
              type="warning"
              variant="tonal"
              density="comfortable"
              class="mb-4"
              border="start"
            >
              <div class="font-weight-bold mb-1">
                Em Endereços do servidor, o endereço de Internet está diferente.
              </div>
              <div class="text-body-2">
                Lá consta <strong>{{ enderecoDivergente }}</strong
                >, corrigido à mão; aqui está
                <strong>{{ form.publicEndpoint || 'em branco' }}</strong
                >. Enquanto os dois discordarem, o envio de log usa um endereço e os túneis usam
                outro.
              </div>
              <template #append>
                <v-btn size="small" variant="flat" color="warning" @click="adotaDaLista">
                  Usar {{ enderecoDivergente }}
                </v-btn>
              </template>
            </v-alert>

            <v-row>
              <v-col cols="12" md="6">
                <!--
                  Combobox e não campo livre: os endereços por onde este servidor
                  é alcançado já estão catalogados em Endereços do servidor, e
                  redigitar um deles aqui é o convite para o erro de digitação
                  que só aparece quando um túnel não fecha. Digitar continua
                  valendo — o endpoint da VPN pode ser um nome que não está na
                  lista.
                -->
                <!--
                  `align-start` e não `align-center`: com a dica fixa embaixo, o
                  centro do bloco fica abaixo do campo e o botão desceria junto.
                  O `mt-1` compensa a diferença entre a altura do controle
                  (48px) e a do botão (40px).
                -->
                <div class="d-flex align-start ga-2">
                  <v-combobox
                    v-model="form.publicEndpoint"
                    :items="enderecoOpcoes"
                    item-title="title"
                    item-value="value"
                    :item-props="enderecoItemProps"
                    :return-object="false"
                    label="Endereço público (IP ou DDNS)"
                    variant="outlined"
                    density="comfortable"
                    append-inner-icon="mdi-crosshairs-gps"
                    hint="Usado como Endpoint nos scripts dos equipamentos"
                    persistent-hint
                    class="flex-grow-1 min-width-0"
                    @click:append-inner="detectEndpoint"
                  ></v-combobox>
                  <ServerAddressesButton class="mt-1" @saved="recarregaEnderecos" />
                </div>
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
                <!--
                  As sugestões são faixas privadas que **não colidem** com
                  nenhuma rede já cadastrada. Uma lista fixa ofereceria
                  `10.8.0.0/24` para quem já usa essa faixa na LAN, e o resultado
                  seria um roteador com dois caminhos para o mesmo endereço.
                -->
                <v-combobox
                  v-model="form.cidr"
                  :items="cidrSugestoes"
                  :return-object="false"
                  label="Sub-rede da VPN (CIDR)"
                  variant="outlined"
                  density="comfortable"
                  :disabled="cidrBloqueado"
                  :error-messages="cidrErro"
                  :hint="cidrHint"
                  persistent-hint
                ></v-combobox>
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

            <!--
              Aviso e não bloqueio: há quem tenha a rede cadastrada errada, e
              travar a configuração por causa disso trocaria um problema por
              outro. O que não pode é a sobreposição passar despercebida — dois
              caminhos para o mesmo endereço não dão erro, dão pacote sumido.
            -->
            <v-alert
              v-if="cidrColide && !cidrBloqueado"
              type="warning"
              variant="tonal"
              class="mt-4"
              density="comfortable"
            >
              A faixa <strong>{{ form.cidr }}</strong> sobrepõe a rede
              <strong>{{ cidrColide.name }}</strong> ({{ cidrColide.cidr }}). Os equipamentos
              ficariam com dois caminhos para o mesmo endereço — escolha uma faixa livre.
            </v-alert>

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

    <!--
      A pergunta só aparece quando trocar o endereço custa alguma coisa: com
      peers já configurados, cada um deles guarda o endpoint antigo no próprio
      arquivo e para de fechar o túnel até receber a configuração nova. Sem peers
      não há o que quebrar, e perguntar ali seria cerimônia.
    -->
    <v-dialog v-model="confirmacaoDeTroca" max-width="560">
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold">Substituir o endereço público?</v-card-title>
        <v-card-text>
          <p class="mb-3">
            O endpoint sai de <strong>{{ endpointGravado || 'em branco' }}</strong> para
            <strong>{{ form.publicEndpoint }}</strong
            >.
          </p>
          <v-alert type="warning" variant="tonal" density="comfortable">
            {{ vpnStore.state?.peersTotal ?? 0 }} dispositivo(s) já configurado(s) continuam com o
            endereço antigo gravado. Eles só voltam a fechar o túnel depois de receberem a
            configuração nova — gere o script de cada um novamente em Dispositivos VPN.
          </v-alert>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="confirmacaoDeTroca = false">Cancelar</v-btn>
          <v-btn color="primary" variant="flat" @click="confirmaTroca">Substituir e aplicar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useVpnStore } from '@/stores/vpn'
import { useNetworksStore } from '@/stores/networks'
import { useServerAddressesStore, addressIcon } from '@/stores/serverAddresses'
import { formatBytes } from '@/utils/formatters'
import { cidrOverlaps, freeVpnCidrs, parseCidr } from '@/utils/cidr'
import PageHeader from '@/components/PageHeader.vue'
import ServerAddressesButton from '@/components/ServerAddressesButton.vue'

const vpnStore = useVpnStore()
const networksStore = useNetworksStore()
const addressesStore = useServerAddressesStore()
const savedMessage = ref<string | null>(null)
const confirmacaoDeTroca = ref(false)

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

/** O que está gravado hoje — a referência de "mudou" e de "vai substituir". */
const endpointGravado = computed(() => vpnStore.state?.server?.publicEndpoint ?? '')

interface EnderecoOpcao {
  value: string
  title: string
  subtitle: string
  icon: string
}

/**
 * Os endereços já catalogados, com o de Internet primeiro.
 *
 * A lista inteira é oferecida, e não só o de Internet: há instalação em que o
 * túnel é fechado por dentro de outra VPN ou pela própria LAN, e esconder essas
 * entradas obrigaria a redigitar o que já está cadastrado.
 */
const enderecoOpcoes = computed<EnderecoOpcao[]>(() => {
  const ordem: Record<string, number> = { public: 0, vpn: 2, lan: 3 }
  return [...addressesStore.usable]
    .sort((a, b) => (ordem[a.kind] ?? 1) - (ordem[b.kind] ?? 1))
    .map((entrada) => ({
      value: entrada.value ?? '',
      title: entrada.value ?? '',
      subtitle: `${entrada.label} — ${entrada.description}`,
      icon: addressIcon(entrada.kind),
    }))
})

function enderecoItemProps(item: EnderecoOpcao): Record<string, unknown> {
  return { title: item.title, subtitle: item.subtitle, prependIcon: item.icon }
}

/**
 * O endereço de Internet corrigido à mão que discorda deste campo.
 *
 * Só a **correção** conta: o valor detectado sai deste próprio campo, então
 * compará-lo consigo mesmo nunca acusaria nada.
 */
const enderecoDivergente = computed(() => {
  const entrada = addressesStore.byId('public')
  if (!entrada?.overridden) return null
  const valor = (entrada.value ?? '').trim()
  if (!valor || valor === form.publicEndpoint.trim()) return null
  return valor
})

/** Depois de mexer na lista, o aviso de divergência precisa ser recalculado. */
async function recarregaEnderecos() {
  await addressesStore.fetchAll(true)
}

function adotaDaLista() {
  const valor = enderecoDivergente.value
  if (valor) form.publicEndpoint = valor
}

const cidrBloqueado = computed(() => vpnStore.isConfigured && (vpnStore.state?.peersTotal ?? 0) > 0)

/** As redes cadastradas que não são a do próprio túnel. */
const faixasOcupadas = computed(() =>
  networksStore.networks
    .filter((rede) => rede.id !== vpnStore.state?.server?.networkId)
    .map((rede) => rede.cidr)
    .filter(Boolean)
)

const cidrSugestoes = computed(() => freeVpnCidrs(faixasOcupadas.value))

/** A rede cadastrada com que a faixa digitada colide, se houver. */
const cidrColide = computed(() => {
  const valor = form.cidr.trim()
  if (!valor || !parseCidr(valor)) return null
  return (
    networksStore.networks.find(
      (rede) =>
        rede.id !== vpnStore.state?.server?.networkId && rede.cidr && cidrOverlaps(valor, rede.cidr)
    ) ?? null
  )
})

const cidrErro = computed(() => {
  const valor = form.cidr.trim()
  if (valor && !parseCidr(valor)) return 'Formato inválido — use algo como 10.8.0.0/24'
  return ''
})

const cidrHint = computed(() => {
  if (cidrBloqueado.value) return 'Bloqueado após existirem peers'
  const colisao = cidrColide.value
  if (colisao) return `Colide com a rede "${colisao.name}" (${colisao.cidr})`
  return 'Faixa privada só para o túnel, sem sobrepor as redes cadastradas'
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
  // Em paralelo: as três respostas alimentam partes diferentes da tela e
  // nenhuma depende da outra.
  await Promise.all([
    vpnStore.fetchServer(),
    addressesStore.fetchAll(true),
    networksStore.fetchNetworks(),
  ])
})

async function detectEndpoint() {
  const detected = await vpnStore.detectEndpoint()
  if (detected) form.publicEndpoint = detected
}

async function runPreflight() {
  await vpnStore.runPreflight()
}

function save() {
  // Trocar o endpoint com peers configurados invalida o arquivo de cada um
  // deles; sem peers não há o que quebrar, e a pergunta seria só um clique a
  // mais entre o operador e o que ele já decidiu.
  const trocou = form.publicEndpoint.trim() !== endpointGravado.value.trim()
  if (trocou && endpointGravado.value.trim() && (vpnStore.state?.peersTotal ?? 0) > 0) {
    confirmacaoDeTroca.value = true
    return
  }
  void persist()
}

function confirmaTroca() {
  confirmacaoDeTroca.value = false
  void persist()
}

async function persist() {
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
    // A lista de endereços deriva daqui: sem recarregar, o aviso de divergência
    // continuaria apontando um conflito que acabou de ser resolvido.
    await addressesStore.fetchAll(true)
  }
}
</script>

<style scoped>
/* Sem isto o campo se recusa a encolher e empurra o botão para fora da coluna. */
.min-width-0 {
  min-width: 0;
}
</style>

<template>
  <v-dialog v-model="open" max-width="760" :persistent="running">
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center">
        <v-icon start>mdi-console-network-outline</v-icon>
        Ativar log automaticamente
      </v-card-title>

      <v-card-subtitle class="pb-3">
        O servidor entra em <strong>{{ deviceName }}</strong
        ><span v-if="host"> ({{ host }})</span> e aplica a configuração de envio de syslog. Nada é
        instalado no equipamento — são os mesmos comandos da aba manual.
      </v-card-subtitle>

      <v-divider></v-divider>

      <v-card-text>
        <!--
          O aviso de credencial vem antes dos campos, e não depois: depois do
          botão ele seria lido só por quem já digitou a senha.
        -->
        <v-alert type="info" variant="tonal" density="comfortable" class="mb-5" border="start">
          <div class="font-weight-bold mb-1">Estes dados são usados uma única vez.</div>
          O usuário e a senha valem só para esta conexão: eles não são gravados no banco, não são
          cifrados em lugar nenhum e não ficam em cache. Para reconfigurar este equipamento depois,
          será preciso digitá-los de novo.
        </v-alert>

        <div v-if="loadingHints" class="d-flex align-center ga-2 mb-4 text-caption text-grey">
          <v-progress-circular indeterminate size="16" width="2"></v-progress-circular>
          Verificando o equipamento — portas de acesso, SNMP e endereço deste servidor...
        </div>

        <v-row density="compact">
          <v-col cols="12" sm="7">
            <!--
              O mesmo catálogo do cadastro do dispositivo e do assistente da
              VPN. Os sistemas sem receita aparecem **desabilitados** em vez de
              omitidos: sumir da lista faria parecer que a lista está incompleta,
              e o subtítulo explica por que não dá para escolher.
            -->
            <v-select
              v-model="operatingSystem"
              :items="systemOptions"
              item-title="title"
              item-value="value"
              :item-props="systemItemProps"
              label="Sistema"
              density="compact"
              variant="outlined"
              :loading="systemsStore.loading"
              :disabled="running"
              :prepend-inner-icon="systemsStore.icon(operatingSystem)"
              :hint="systemHintText"
              persistent-hint
            ></v-select>
          </v-col>
          <v-col cols="12" sm="5">
            <v-select
              v-model="protocol"
              :items="protocolOptions"
              item-title="label"
              item-value="value"
              label="Acesso"
              density="compact"
              variant="outlined"
              :disabled="running"
              :hint="protocolHintText"
              persistent-hint
              :item-props="protocolItemProps"
              @update:model-value="onProtocolChange"
            ></v-select>
          </v-col>

          <v-col cols="12" sm="8">
            <v-text-field
              v-model="username"
              label="Usuário"
              autocomplete="off"
              density="compact"
              variant="outlined"
              :error-messages="fieldErrors.username"
              :disabled="running"
              prepend-inner-icon="mdi-account-outline"
              @update:model-value="fieldErrors.username = ''"
            ></v-text-field>
          </v-col>
          <v-col cols="12" sm="4">
            <v-text-field
              v-model.number="port"
              label="Porta"
              type="number"
              density="compact"
              variant="outlined"
              :disabled="running || protocol === 'mactelnet'"
              :hint="protocol === 'mactelnet' ? 'Fixa no protocolo' : `Padrão ${defaultPort}`"
              persistent-hint
            ></v-text-field>
          </v-col>

          <v-col cols="12">
            <v-text-field
              v-model="password"
              label="Senha"
              autocomplete="new-password"
              :type="showPassword ? 'text' : 'password'"
              :append-inner-icon="showPassword ? 'mdi-eye-off' : 'mdi-eye'"
              density="compact"
              variant="outlined"
              :error-messages="fieldErrors.password"
              :disabled="running"
              prepend-inner-icon="mdi-lock-outline"
              @click:append-inner="showPassword = !showPassword"
              @update:model-value="fieldErrors.password = ''"
            ></v-text-field>
          </v-col>

          <v-col v-if="protocol === 'mactelnet'" cols="12">
            <v-text-field
              v-model="macAddress"
              label="MAC do equipamento"
              placeholder="AA:BB:CC:DD:EE:FF"
              density="compact"
              variant="outlined"
              :error-messages="fieldErrors.macAddress"
              :disabled="running"
              prepend-inner-icon="mdi-ethernet"
              :hint="
                hints?.macAddress
                  ? 'Descoberto pelo inventário — troque se não for a interface certa'
                  : 'Nenhum MAC conhecido para este equipamento; informe manualmente'
              "
              persistent-hint
              @update:model-value="fieldErrors.macAddress = ''"
            ></v-text-field>
          </v-col>

          <!--
            Enquanto a sondagem roda, o campo diz que está carregando —
            aparecer vazio e preencher de repente lia como campo quebrado. Se
            a detecção falha, o seletor abre normalmente, com o aviso do que
            aconteceu em vez de uma lista sem contexto.
          -->
          <v-col v-if="carregandoEndereco" cols="12">
            <v-sheet class="pa-3 rounded-lg border d-flex align-center ga-3">
              <v-progress-circular
                indeterminate
                size="20"
                width="2"
                color="primary"
              ></v-progress-circular>
              <div class="text-body-2 text-medium-emphasis">
                Descobrindo por onde este equipamento alcança o NetMonitor…
              </div>
            </v-sheet>
          </v-col>

          <template v-else>
            <v-col v-if="enderecoFalhou" cols="12" class="pb-0">
              <v-alert type="warning" variant="tonal" density="compact">
                Não foi possível descobrir o endereço automaticamente. Escolha na lista por onde
                este equipamento alcança o NetMonitor.
              </v-alert>
            </v-col>

            <!--
              O endereço já foi decidido: o cadastro diz como este equipamento é
              acessado, e a lista de endereços do servidor diz qual deles serve
              a essa situação. Mostrar a conclusão e o motivo — em vez de um
              seletor — tira do operador uma pergunta que ele responderia por
              eliminação. O seletor continua a um clique, para quando a
              conclusão estiver errada.
            -->
            <v-col v-if="enderecoResolvido && !editandoEndereco" cols="12">
              <v-sheet class="pa-3 rounded-lg border d-flex align-start ga-3">
                <v-icon :color="accessModeMeta(hints?.accessMode).color" class="mt-1">
                  {{ accessModeMeta(hints?.accessMode).icon }}
                </v-icon>
                <div class="flex-grow-1 min-width-0">
                  <div class="text-body-2">
                    O equipamento vai enviar o log para
                    <strong>{{ serverAddress }}:{{ hints?.serverPort ?? 514 }}</strong>
                  </div>
                  <div class="text-caption text-medium-emphasis motivo">
                    {{ motivoDoEndereco }}
                  </div>
                </div>
                <v-btn
                  size="small"
                  variant="text"
                  :disabled="running"
                  @click="editandoEndereco = true"
                >
                  Alterar
                </v-btn>
              </v-sheet>
            </v-col>

            <v-col v-else cols="12">
              <!--
                Seletor e não campo livre: o endereço certo depende de onde o
                equipamento está, e essa lista é exatamente o catálogo dessas
                situações. Digitar continua possível pela última opção.
              -->
              <div class="d-flex align-start ga-2">
                <v-select
                  v-model="addressChoice"
                  :items="addressOptions"
                  item-title="title"
                  item-value="value"
                  label="Endereço que o equipamento vai usar"
                  density="compact"
                  variant="outlined"
                  :error-messages="fieldErrors.serverAddress"
                  :disabled="running"
                  prepend-inner-icon="mdi-server-network"
                  :item-props="addressItemProps"
                  class="flex-grow-1 min-width-0"
                  @update:model-value="onAddressChange"
                ></v-select>
                <!--
                  Campo `compact` (40px) e botão `comfortable` (40px): a mesma
                  altura, então nenhum deslocamento é necessário aqui.
                -->
                <ServerAddressesButton :disabled="running" @saved="onAddressesSaved" />
              </div>

              <div class="text-caption text-medium-emphasis px-1">
                {{ addressHintText }}
              </div>
            </v-col>

            <v-col v-if="editandoEndereco && addressChoice === CUSTOM_CHOICE" cols="12">
              <v-text-field
                v-model="serverAddress"
                label="Endereço deste servidor"
                placeholder="IP ou nome pelo qual o equipamento alcança o NetMonitor"
                density="compact"
                variant="outlined"
                :disabled="running"
                prepend-inner-icon="mdi-pencil-outline"
                autofocus
                @update:model-value="fieldErrors.serverAddress = ''"
              ></v-text-field>
            </v-col>
          </template>
        </v-row>

        <!--
          O `localhost` é o erro que este campo existe para evitar: é aceito por
          todo comando de configuração e faz o roteador mandar o log para si
          mesmo, sem erro visível em lugar nenhum.
        -->
        <v-alert
          v-if="serverAddressLooksLocal"
          type="error"
          variant="tonal"
          density="compact"
          class="mt-3"
        >
          <strong>Este endereço aponta o roteador para ele mesmo.</strong> Informe o IP pelo qual o
          equipamento alcança este servidor na rede — não o endereço que você usa no navegador.
        </v-alert>

        <v-alert
          v-if="protocol === 'telnet'"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-3"
        >
          O Telnet trafega a senha em texto claro pela rede. Use SSH sempre que o equipamento
          oferecer.
        </v-alert>

        <v-alert
          v-if="protocol === 'mactelnet' && hints && !hints.layer2Reachable"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-3"
        >
          <strong>Este servidor está num container em rede bridge.</strong> O MAC-Telnet encontra o
          equipamento por difusão na rede local, e difusão não atravessa a ponte do Docker — a
          tentativa deve falhar por falta de resposta. Use <code>network_mode: host</code> no
          <code>docker-compose.yml</code> para que este meio funcione.
        </v-alert>

        <v-alert v-if="error" type="error" variant="tonal" density="comfortable" class="mt-4">
          <div class="font-weight-bold mb-1">{{ error }}</div>
          <div v-if="errorHint" class="text-body-2">{{ errorHint }}</div>
        </v-alert>

        <template v-if="result">
          <v-divider class="my-4"></v-divider>

          <v-alert
            :type="result.confirmed === true ? 'success' : 'warning'"
            variant="tonal"
            density="comfortable"
            border="start"
          >
            <template v-if="result.confirmed === true">
              <div class="font-weight-bold">Pronto — o log já está chegando.</div>
              A linha de teste enviada pelo equipamento foi recebida por este servidor.
            </template>
            <template v-else>
              <div class="font-weight-bold mb-1">
                Comandos aceitos, mas a linha de teste não chegou.
              </div>
              O equipamento aplicou a configuração e emitiu uma mensagem de teste; ela não apareceu
              aqui em {{ CONFIRMATION_SECONDS }} segundos. Como a mensagem foi provocada de
              propósito, o silêncio aponta para o caminho, não para o equipamento: normalmente é
              firewall bloqueando a porta {{ result.serverPort }}, ou o endereço
              <strong>{{ result.serverAddress }}</strong> não ser alcançável a partir do roteador.
            </template>
          </v-alert>

          <div class="text-caption text-grey mt-4 mb-1">
            Enviado para {{ result.serverAddress }}:{{ result.serverPort }}
          </div>
          <v-sheet class="pa-3 rounded-lg border" color="surface-variant">
            <pre class="transcript">{{ result.transcript }}</pre>
          </v-sheet>
        </template>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions>
        <v-spacer></v-spacer>
        <!--
          Depois de um resultado o botão de destaque fecha o diálogo. "Aplicar
          de novo" continua ali, mas em segundo plano: deixá-lo em destaque
          fazia o desfecho parecer uma falha que precisa ser refeita.
        -->
        <template v-if="result">
          <v-btn variant="text" :loading="running" @click="submit">Aplicar de novo</v-btn>
          <v-btn color="primary" variant="flat" @click="open = false">Concluir</v-btn>
        </template>
        <template v-else>
          <v-btn variant="text" :disabled="running" @click="open = false">Cancelar</v-btn>
          <v-btn color="primary" variant="flat" :loading="running" @click="submit">
            <v-icon start>mdi-flash</v-icon>
            Ativar agora
          </v-btn>
        </template>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import {
  useLogsStore,
  type ProvisionHintsResponse,
  type ProvisionLoggingResponse,
} from '@/stores/logs'
import { useServerAddressesStore } from '@/stores/serverAddresses'
import ServerAddressesButton from '@/components/ServerAddressesButton.vue'
import { accessModeMeta } from '@/utils/accessMode'
import { operatingSystemSourceLabel, useOperatingSystemsStore } from '@/stores/operatingSystems'

/** Igual ao `TETO_DA_CONFIRMACAO` do backend — só para o texto do aviso. */
const CONFIRMATION_SECONDS = 12

const props = defineProps<{
  deviceId: number
  deviceName: string
  host: string | null | undefined
}>()

const open = defineModel<boolean>({ required: true })

const logsStore = useLogsStore()
const addressesStore = useServerAddressesStore()

const systemsStore = useOperatingSystemsStore()

const operatingSystem = ref('routeros')
const protocol = ref('ssh')
const port = ref<number | null>(22)
const username = ref('')
const password = ref('')
const macAddress = ref('')
const serverAddress = ref('')
const addressChoice = ref<string>('')
/** O seletor só aparece quando a conclusão do servidor não serve. */
const editandoEndereco = ref(false)
/** A sondagem de palpites falhou — o seletor abre com o aviso, não em silêncio. */
const hintsFalhou = ref(false)
const showPassword = ref(false)
const running = ref(false)
const loadingHints = ref(false)
const error = ref('')
const errorHint = ref('')
const result = ref<ProvisionLoggingResponse | null>(null)
const hints = ref<ProvisionHintsResponse | null>(null)

const fieldErrors = reactive({
  username: '',
  password: '',
  macAddress: '',
  serverAddress: '',
})

const DEFAULT_PORTS: Record<string, number> = { ssh: 22, telnet: 23, mactelnet: 20561 }

const defaultPort = computed(() => DEFAULT_PORTS[protocol.value] ?? 22)

interface SystemOption {
  value: string
  title: string
  subtitle: string
  icon: string
  disabled: boolean
}

/**
 * O catálogo do servidor, com os sem receita desabilitados.
 *
 * Quem decide o que é possível é o backend — `supportsSyslog` sai da mesma
 * tabela que guarda os comandos. Uma lista escrita aqui divergiria dela na
 * primeira entrada nova, e a divergência só apareceria quando alguém escolhesse
 * a opção que não funciona. Desabilitar em vez de omitir: sumir da lista faria
 * parecer que ela está incompleta, e o subtítulo explica o motivo.
 */
const systemOptions = computed<SystemOption[]>(() =>
  systemsStore.systems.map((sistema) => ({
    value: sistema.id,
    title: sistema.label,
    subtitle: sistema.supportsSyslog
      ? 'Comandos prontos para este sistema'
      : 'Este sistema não tem ativação automática de log',
    icon: sistema.icon,
    disabled: !sistema.supportsSyslog,
  }))
)

function systemItemProps(item: SystemOption): Record<string, unknown> {
  return { subtitle: item.subtitle, prependIcon: item.icon, disabled: item.disabled }
}

/**
 * A dica embaixo do seletor.
 *
 * O **motivo** vem primeiro, e a origem só quando ele falta: "Identificado pelo
 * SNMP" não deixava conferir nada — um OpenWrt cujo agente responde só o `uname`
 * também é "identificado pelo SNMP", e ficava como Linux sem ninguém entender.
 */
const systemHintText = computed(
  () =>
    hints.value?.operatingSystemReason ||
    operatingSystemSourceLabel(hints.value?.operatingSystemSource)
)

/**
 * Se o sistema escolhido alcança o equipamento por MAC-Telnet.
 *
 * Vem do catálogo, e não de uma lista aqui: oferecer o meio fora dele seria
 * oferecer uma tentativa que não tem como dar certo.
 */
const aceitaMacTelnet = computed(
  () => systemsStore.byId(operatingSystem.value)?.supportsMacTelnet ?? false
)

interface ProtocolOption {
  value: string
  label: string
  subtitle: string
}

const protocolOptions = computed<ProtocolOption[]>(() => {
  const opcoes: ProtocolOption[] = [
    { value: 'ssh', label: 'SSH', subtitle: portaLabel(hints.value?.sshOpen, 22) },
    { value: 'telnet', label: 'Telnet', subtitle: portaLabel(hints.value?.telnetOpen, 23) },
  ]
  if (aceitaMacTelnet.value) {
    opcoes.push({
      value: 'mactelnet',
      label: 'MAC-Telnet',
      subtitle: 'Por endereço MAC, sem IP — só na mesma rede local',
    })
  }
  return opcoes
})

/**
 * O resultado da sondagem vira o subtítulo de cada opção da lista — é ali que
 * ele responde à pergunta que o operador está fazendo no momento de escolher.
 */
function protocolItemProps(item: ProtocolOption): Record<string, unknown> {
  return { subtitle: item.subtitle }
}

function portaLabel(aberta: boolean | undefined, porta: number): string {
  if (aberta === undefined) return `Porta ${porta}`
  return aberta ? `Porta ${porta} — respondeu` : `Porta ${porta} — sem resposta`
}

const protocolHintText = computed(() => {
  if (protocol.value === 'mactelnet') return 'Endereça o equipamento pelo MAC'
  const aberta = protocol.value === 'ssh' ? hints.value?.sshOpen : hints.value?.telnetOpen
  if (aberta === false) return 'A porta não respondeu à sondagem'
  return ''
})

/** Valor sentinela da opção "digitar outro endereço". */
const CUSTOM_CHOICE = '__custom__'

interface AddressOption {
  value: string
  title: string
  subtitle: string
}

const addressOptions = computed<AddressOption[]>(() => {
  const opcoes: AddressOption[] = addressesStore.usable.map((entrada) => ({
    value: entrada.id,
    title: `${entrada.label} — ${entrada.value}`,
    subtitle: entrada.description,
  }))
  opcoes.push({
    value: CUSTOM_CHOICE,
    title: 'Outro endereço…',
    subtitle: 'Digitar um endereço só para este equipamento',
  })
  return opcoes
})

function addressItemProps(item: AddressOption): Record<string, unknown> {
  return { subtitle: item.subtitle }
}

/**
 * A frase abaixo do seletor. Quando o servidor sugeriu uma entrada, ela diz
 * **por quê** — e é esse motivo que dispensa o operador de entender a lista
 * antes de escolher.
 */
const addressHintText = computed(() => {
  if (addressChoice.value === CUSTOM_CHOICE) {
    return 'Use o IP pelo qual o roteador alcança este servidor, não o da barra do navegador.'
  }
  const motivo = hints.value?.suggestedAddressReason
  if (motivo && addressChoice.value === hints.value?.suggestedAddressId) {
    return `Sugerido: ${motivo}.`
  }
  const entrada = addressesStore.byId(addressChoice.value)
  if (entrada) return entrada.description
  // Sem escolha e sem sugestão, o que ainda orienta o operador é o que o
  // servidor sabe sobre como o equipamento é alcançado.
  const acesso = hints.value?.accessModeReason
  return acesso
    ? `Sem sugestão automática — ${acesso}. Escolha por onde ele alcança o NetMonitor.`
    : 'Escolha por onde este equipamento alcança o NetMonitor.'
})

/**
 * Se há endereço decidido — e portanto nada a perguntar.
 *
 * Cair na opção "digitar outro" significa que a lista não tinha resposta: ali o
 * campo precisa aparecer, porque a alternativa seria um resumo dizendo que o
 * equipamento vai enviar para lugar nenhum.
 */
const enderecoResolvido = computed(
  () => Boolean(serverAddress.value.trim()) && addressChoice.value !== CUSTOM_CHOICE
)

/**
 * A sondagem de palpites e a lista de endereços chegam juntas; enquanto
 * qualquer uma está em trânsito o campo mostra carregamento em vez de um
 * seletor vazio que se preenche sozinho depois.
 */
const carregandoEndereco = computed(() => loadingHints.value || addressesStore.loading)

/**
 * Falhou quem falhou — os palpites (`null` do store) ou a lista (erro do
 * store). Só vale depois do carregamento: durante ele, ainda não há falha.
 */
const enderecoFalhou = computed(
  () => !carregandoEndereco.value && (hintsFalhou.value || Boolean(addressesStore.error))
)

/**
 * A frase que substitui o seletor.
 *
 * Carrega o rótulo do endereço **e** o motivo — "Túnel VPN — o cadastro diz que
 * este equipamento acessa por túnel vpn". Só o endereço deixaria o operador sem
 * como julgar se está certo, que é exatamente o que o seletor pedia dele.
 */
const motivoDoEndereco = computed(() => {
  const entrada = addressesStore.byId(addressChoice.value)
  const rotulo = entrada?.label ?? 'Endereço deste servidor'
  const motivo =
    addressChoice.value === hints.value?.suggestedAddressId
      ? hints.value?.suggestedAddressReason
      : null
  const complemento = motivo ?? entrada?.description
  return complemento ? `${rotulo} — ${complemento}` : rotulo
})

function onAddressChange(escolha: unknown): void {
  fieldErrors.serverAddress = ''
  const id = typeof escolha === 'string' ? escolha : null
  if (!id || id === CUSTOM_CHOICE) {
    serverAddress.value = ''
    return
  }
  serverAddress.value = addressesStore.byId(id)?.value ?? ''
}

/** Depois de gerenciar a lista, reaproveita a sugestão se ela passou a existir. */
function onAddressesSaved(): void {
  const sugerido = hints.value?.suggestedAddressId
  if (sugerido && addressesStore.usable.some((entrada) => entrada.id === sugerido)) {
    addressChoice.value = sugerido
  } else if (addressChoice.value !== CUSTOM_CHOICE) {
    addressChoice.value = addressesStore.usable[0]?.id ?? CUSTOM_CHOICE
  }
  onAddressChange(addressChoice.value)
}

/**
 * Endereços que o roteador resolveria para si mesmo. Detectado aqui só para
 * avisar cedo — o backend recusa de qualquer forma.
 */
const serverAddressLooksLocal = computed(() => {
  const valor = serverAddress.value.trim().toLowerCase()
  if (!valor) return false
  return valor === 'localhost' || valor === '::1' || valor.startsWith('127.')
})

/**
 * Ajusta a porta ao trocar de meio de acesso.
 *
 * O valor novo chega por parâmetro em vez de ser lido de `protocol.value`: a
 * ordem entre o `v-model` e este ouvinte não é garantida, e ler o ref daria a
 * porta do protocolo **anterior** — SSH na 23, exatamente o erro que este
 * comportamento existe para evitar.
 */
function onProtocolChange(escolhido: unknown): void {
  const chave = typeof escolhido === 'string' ? escolhido : protocol.value
  // A porta padrão é **setada**, não deixada em branco: um campo vazio com
  // placeholder obriga a saber qual é o padrão para conferir se está certo.
  port.value = DEFAULT_PORTS[chave] ?? 22
  fieldErrors.macAddress = ''
}

// Trocar de sistema pode tirar o MAC-Telnet da lista; deixar a seleção
// apontando para uma opção que sumiu quebraria o envio em silêncio.
watch(operatingSystem, () => {
  if (protocol.value === 'mactelnet' && !aceitaMacTelnet.value) {
    protocol.value = 'ssh'
    onProtocolChange('ssh')
  }
})

function valida(): boolean {
  fieldErrors.username = username.value.trim() ? '' : 'Informe o usuário de acesso'
  fieldErrors.password = password.value ? '' : 'Informe a senha de acesso'
  fieldErrors.serverAddress = serverAddress.value.trim()
    ? serverAddressLooksLocal.value
      ? 'Este endereço aponta o roteador para ele mesmo'
      : ''
    : addressChoice.value === CUSTOM_CHOICE
      ? 'Digite o endereço que o equipamento deve usar'
      : 'Escolha por onde este equipamento alcança o NetMonitor'
  fieldErrors.macAddress =
    protocol.value === 'mactelnet' && !macAddress.value.trim()
      ? 'O MAC-Telnet precisa do MAC do equipamento'
      : ''
  return !Object.values(fieldErrors).some(Boolean)
}

async function submit(): Promise<void> {
  if (running.value) return
  if (!valida()) {
    error.value = 'Corrija os campos destacados antes de continuar.'
    errorHint.value = ''
    // Um erro no endereço enquanto o resumo está fechado ficaria marcado num
    // campo que ninguém vê — a mensagem apontaria para o nada.
    if (fieldErrors.serverAddress) editandoEndereco.value = true
    return
  }

  running.value = true
  error.value = ''
  errorHint.value = ''
  result.value = null
  try {
    result.value = await logsStore.provisionDevice(props.deviceId, {
      protocol: protocol.value,
      port: port.value || null,
      username: username.value,
      password: password.value,
      operatingSystem: operatingSystem.value,
      serverAddress: serverAddress.value.trim(),
      macAddress: macAddress.value.trim() || null,
    })
  } catch (erro) {
    error.value = mensagemDeErro(erro)
    errorHint.value = pista(error.value)
  } finally {
    running.value = false
  }
}

function mensagemDeErro(erro: unknown): string {
  if (erro instanceof Error && erro.message.trim()) return erro.message.trim()
  return 'Não foi possível concluir a ativação.'
}

/**
 * Acrescenta o próximo passo à mensagem do servidor.
 *
 * O backend já explica **o que** aconteceu; o que faltava era o que fazer a
 * respeito, e é isso que separa um erro útil de um erro que só interrompe.
 */
function pista(mensagem: string): string {
  const texto = mensagem.toLowerCase()
  if (texto.includes('recusad') || texto.includes('senha')) {
    return 'Confirme se o usuário tem permissão de administrador no equipamento. No RouterOS, o grupo precisa incluir as políticas "write" e "policy".'
  }
  if (texto.includes('tempo esgotado') || texto.includes('acessar o equipamento')) {
    return `Verifique se o serviço está ligado no equipamento e se a porta ${port.value ?? defaultPort.value} aceita conexão a partir deste servidor.`
  }
  if (texto.includes('mac-telnet') || texto.includes('difusão')) {
    return 'O MAC-Telnet só funciona na mesma rede de camada 2. Num container em rede bridge ele não alcança o equipamento.'
  }
  if (texto.includes('endereço deste servidor') || texto.includes('localhost')) {
    return 'Preencha o campo "Endereço deste servidor" com o IP que o roteador enxerga — o endereço da barra do navegador raramente serve.'
  }
  return ''
}

async function carregaPalpites(): Promise<void> {
  loadingHints.value = true
  try {
    // A lista de endereços e os palpites do equipamento são independentes; em
    // série somariam a sondagem de portas à leitura da lista sem motivo.
    const [dicas] = await Promise.all([
      logsStore.fetchProvisionHints(props.deviceId),
      addressesStore.fetchAll(true),
      systemsStore.fetchAll(),
    ])
    hints.value = dicas
    // `null` é falha da sondagem, não "nada a sugerir" — a tela precisa saber
    // a diferença para avisar em vez de só mostrar o seletor.
    hintsFalhou.value = dicas === null
    aplicaEndereco(dicas)
    if (!dicas) return

    operatingSystem.value = dicas.operatingSystem
    macAddress.value = dicas.macAddress ?? ''

    // A sondagem decide o meio de acesso: oferecer SSH quando só a 23 responde
    // faria o operador esperar um timeout para descobrir sozinho.
    if (dicas.sshOpen) protocol.value = 'ssh'
    else if (dicas.telnetOpen) protocol.value = 'telnet'
    port.value = DEFAULT_PORTS[protocol.value] ?? 22
  } finally {
    loadingHints.value = false
  }
}

/**
 * Escolhe a entrada da lista que serve a este equipamento.
 *
 * Só a sugestão do servidor (que sabe por qual rota alcança o aparelho)
 * preenche o campo: sem ela, pré-selecionar o primeiro endereço da lista
 * apresentaria como padrão o que é chute — e o primeiro costuma ser o do
 * túnel VPN, errado para qualquer equipamento fora dele. Nesse caso o seletor
 * abre sem escolha, e a pergunta volta a ser do operador.
 */
function aplicaEndereco(dicas: ProvisionHintsResponse | null): void {
  const sugerido = dicas?.suggestedAddressId
  if (sugerido && addressesStore.usable.some((entrada) => entrada.id === sugerido)) {
    addressChoice.value = sugerido
    onAddressChange(sugerido)
    editandoEndereco.value = false
    return
  }
  if (addressesStore.usable.length === 0) {
    addressChoice.value = CUSTOM_CHOICE
    // Sem lista, o palpite solto do servidor ainda vale como ponto de partida.
    serverAddress.value = dicas?.serverAddress ?? ''
    editandoEndereco.value = true
    return
  }
  addressChoice.value = ''
  serverAddress.value = ''
  editandoEndereco.value = true
}

watch(open, (aberto) => {
  if (aberto) {
    operatingSystem.value = 'routeros'
    protocol.value = 'ssh'
    port.value = 22
    username.value = ''
    password.value = ''
    macAddress.value = ''
    serverAddress.value = ''
    addressChoice.value = ''
    editandoEndereco.value = false
    showPassword.value = false
    error.value = ''
    errorHint.value = ''
    result.value = null
    hints.value = null
    hintsFalhou.value = false
    Object.keys(fieldErrors).forEach((campo) => {
      fieldErrors[campo as keyof typeof fieldErrors] = ''
    })
    void carregaPalpites()
  } else {
    // A senha só é descartada **ao fechar**. Limpá-la logo depois de aplicar
    // deixava o campo vazio marcado como obrigatório ao lado de uma mensagem de
    // sucesso, o que lia como falha — e obrigava a redigitar para reaplicar.
    password.value = ''
    showPassword.value = false
  }
})
</script>

<style scoped>
/* Sem isto o filho do flex se recusa a encolher e a frase transborda o cartão. */
.min-width-0 {
  min-width: 0;
}

.motivo {
  line-height: 1.35;
  overflow-wrap: anywhere;
}

/*
  Sem `max-height` nem `overflow` aqui de propósito: um scroll interno dentro
  do scroll do diálogo prende a roda do mouse no meio do caminho e esconde o
  fim do transcript. Quem rola é o `v-card-text` do diálogo.
*/
.transcript {
  font-family: 'Roboto Mono', 'Courier New', monospace;
  font-size: 0.75rem;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
}
</style>

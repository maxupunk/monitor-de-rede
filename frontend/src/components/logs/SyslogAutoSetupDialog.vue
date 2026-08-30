<template>
  <v-dialog v-model="open" max-width="760" :persistent="running">
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center">
        <v-icon start>mdi-console-network-outline</v-icon>
        Ativar log automaticamente
      </v-card-title>

      <v-card-subtitle class="pb-3">
        O servidor entra em <strong>{{ target.name }}</strong
        ><span v-if="target.host"> ({{ target.host }})</span> e aplica a configuração de envio de
        syslog. Nada é instalado no equipamento — são os mesmos comandos da aba manual.
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

          <v-col v-if="enderecoFalhou" cols="12" class="pb-0">
            <v-alert type="warning" variant="tonal" density="compact">
              Não foi possível descobrir o endereço automaticamente. Selecione uma opção conhecida
              ou digite o IP/hostname pelo qual o equipamento alcança o NetMonitor.
            </v-alert>
          </v-col>

          <!-- Um único valor serve para sugestão, seleção conhecida e texto livre. -->
          <v-col cols="12">
            <div class="d-flex align-start ga-2">
              <v-combobox
                :model-value="serverAddress"
                :items="addressOptions"
                item-title="title"
                item-value="value"
                :return-object="false"
                label="Endereço que o equipamento vai usar"
                placeholder="Selecione ou digite um IP/hostname"
                density="compact"
                variant="outlined"
                clearable
                persistent-hint
                :hint="addressHintText"
                :error-messages="fieldErrors.serverAddress"
                :loading="carregandoEndereco"
                :disabled="running"
                prepend-inner-icon="mdi-server-network"
                :item-props="addressItemProps"
                class="flex-grow-1 min-width-0"
                @update:model-value="onAddressChange"
              >
                <template #no-data>
                  <v-list-item
                    title="Usar o endereço digitado"
                    subtitle="Pressione Enter ou saia do campo para confirmar"
                  ></v-list-item>
                </template>
              </v-combobox>
              <ServerAddressesButton :disabled="running" @saved="onAddressesSaved" />
            </div>
          </v-col>
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
            :type="result.confirmed === true && result.addressSaved ? 'success' : 'warning'"
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

          <v-alert
            v-if="result.identifiedHostname"
            type="info"
            variant="tonal"
            density="compact"
            class="mt-3"
            icon="mdi-tag-check-outline"
          >
            <strong>Identidade reconhecida: {{ result.identifiedHostname }}.</strong>
            O NetMonitor lembrará este nome para associar os próximos logs ao dispositivo, mesmo
            quando a rede do container mostrar o mesmo IP para vários equipamentos.
          </v-alert>

          <v-alert
            v-if="result.persistenceWarning"
            type="warning"
            variant="tonal"
            density="compact"
            class="mt-3"
          >
            {{ result.persistenceWarning }}
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
import { operatingSystemSourceLabel, useOperatingSystemsStore } from '@/stores/operatingSystems'
import {
  buildProvisionAddressOptions,
  isProvisionSessionCurrent,
  normalizeComboboxAddress,
  resolveProvisionOperatingSystem,
  sameProvisionAddress,
  type LogSetupTarget,
} from '@/utils/syslogProvision'

/** Igual ao `TETO_DA_CONFIRMACAO` do backend — só para o texto do aviso. */
const CONFIRMATION_SECONDS = 12

const props = defineProps<{ target: Readonly<LogSetupTarget> }>()

const open = defineModel<boolean>({ required: true })

const logsStore = useLogsStore()
const addressesStore = useServerAddressesStore()

const systemsStore = useOperatingSystemsStore()

const operatingSystem = ref('')
const protocol = ref('ssh')
const port = ref<number | null>(22)
const username = ref('')
const password = ref('')
const macAddress = ref('')
const serverAddress = ref('')
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

const addressOptions = computed(() => {
  const suggestedEntry = addressesStore.byId(hints.value?.suggestedAddressId)
  return buildProvisionAddressOptions(
    addressesStore.usable,
    hints.value?.suggestedAddressId,
    hints.value?.serverAddress
      ? {
          value: hints.value.serverAddress,
          label: suggestedEntry?.label,
          description:
            hints.value.suggestedAddressReason ||
            `Sugerido automaticamente — ${hints.value.serverAddressSource}`,
        }
      : null
  )
})

function addressItemProps(item: { subtitle: string; suggested: boolean }): Record<string, unknown> {
  return {
    subtitle: item.subtitle,
    prependIcon: item.suggested ? 'mdi-auto-fix' : 'mdi-server-network',
  }
}

/**
 * A frase abaixo do seletor. Quando o servidor sugeriu uma entrada, ela diz
 * **por quê** — e é esse motivo que dispensa o operador de entender a lista
 * antes de escolher.
 */
const addressHintText = computed(() => {
  if (carregandoEndereco.value) return 'Descobrindo a melhor rota para este equipamento…'
  const current = addressesStore.usable.find((entry) =>
    sameProvisionAddress(entry.value, serverAddress.value)
  )
  if (current?.id === hints.value?.suggestedAddressId) {
    const reason = hints.value?.suggestedAddressReason
    return reason ? `Sugerido automaticamente — ${reason}.` : 'Sugerido automaticamente.'
  }
  if (
    hints.value?.serverAddressSource === 'último endereço aplicado' &&
    sameProvisionAddress(hints.value.serverAddress, serverAddress.value)
  ) {
    return 'Último endereço aplicado neste dispositivo.'
  }
  if (sameProvisionAddress(hints.value?.serverAddress, serverAddress.value)) {
    const reason = hints.value?.suggestedAddressReason || hints.value?.serverAddressSource
    return reason ? `Sugerido automaticamente — ${reason}.` : 'Sugerido automaticamente.'
  }
  if (current) return current.description
  if (serverAddress.value.trim()) {
    return 'Endereço personalizado — será adicionado ao catálogo global depois da aplicação.'
  }
  return 'Selecione uma opção conhecida ou digite o IP/hostname alcançável pelo equipamento.'
})

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
 * Adapta tanto uma opção conhecida quanto o texto livre emitido pelo combobox.
 */
function onAddressChange(value: unknown): void {
  fieldErrors.serverAddress = ''
  serverAddress.value = normalizeComboboxAddress(value)
}

/** Gerenciar o catálogo não deve apagar o texto que o operador já digitou. */
function onAddressesSaved(): void {
  if (!serverAddress.value.trim() && hints.value?.serverAddress) {
    serverAddress.value = hints.value.serverAddress
  }
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
    : 'Digite ou selecione o endereço que o equipamento deve usar'
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
    return
  }

  running.value = true
  error.value = ''
  errorHint.value = ''
  result.value = null
  const sessionId = props.target.sessionId
  const deviceId = props.target.id
  try {
    const provisioned = await logsStore.provisionDevice(deviceId, {
      protocol: protocol.value,
      port: port.value || null,
      username: username.value,
      password: password.value,
      operatingSystem: operatingSystem.value,
      serverAddress: serverAddress.value.trim(),
      macAddress: macAddress.value.trim() || null,
    })
    if (
      !isProvisionSessionCurrent(
        open.value,
        sessionId,
        props.target.sessionId,
        deviceId,
        props.target.id
      )
    )
      return
    result.value = provisioned
    void addressesStore.fetchAll(true)
  } catch (erro) {
    if (
      !isProvisionSessionCurrent(
        open.value,
        sessionId,
        props.target.sessionId,
        deviceId,
        props.target.id
      )
    )
      return
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
    return 'Confirme se o usuário tem permissão de administrador para alterar a configuração de logs do equipamento.'
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

let hintsSequence = 0

async function carregaPalpites(): Promise<void> {
  const sequence = ++hintsSequence
  const deviceId = props.target.id
  loadingHints.value = true
  try {
    // A lista de endereços e os palpites do equipamento são independentes; em
    // série somariam a sondagem de portas à leitura da lista sem motivo.
    const [dicas] = await Promise.all([
      logsStore.fetchProvisionHints(deviceId),
      addressesStore.fetchAll(true),
      systemsStore.fetchAll(),
    ])
    if (!isProvisionSessionCurrent(open.value, sequence, hintsSequence, deviceId, props.target.id))
      return
    hints.value = dicas
    // `null` é falha da sondagem, não "nada a sugerir" — a tela precisa saber
    // a diferença para avisar em vez de só mostrar o seletor.
    hintsFalhou.value = dicas === null
    serverAddress.value = dicas?.serverAddress?.trim() ?? serverAddress.value
    if (!dicas) return

    operatingSystem.value = resolveProvisionOperatingSystem(
      dicas.operatingSystem,
      props.target.operatingSystem
    )
    macAddress.value = dicas.macAddress ?? ''

    // A sondagem decide o meio de acesso: oferecer SSH quando só a 23 responde
    // faria o operador esperar um timeout para descobrir sozinho.
    if (dicas.sshOpen) protocol.value = 'ssh'
    else if (dicas.telnetOpen) protocol.value = 'telnet'
    port.value = DEFAULT_PORTS[protocol.value] ?? 22
  } finally {
    if (sequence === hintsSequence) loadingHints.value = false
  }
}

watch(
  [open, () => props.target.sessionId],
  ([aberto]) => {
    if (aberto) {
      operatingSystem.value = resolveProvisionOperatingSystem(null, props.target.operatingSystem)
      protocol.value = 'ssh'
      port.value = 22
      username.value = ''
      password.value = ''
      macAddress.value = ''
      serverAddress.value = ''
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
      hintsSequence += 1
      loadingHints.value = false
      // A senha só é descartada **ao fechar**. Limpá-la logo depois de aplicar
      // deixava o campo vazio marcado como obrigatório ao lado de uma mensagem de
      // sucesso, o que lia como falha — e obrigava a redigitar para reaplicar.
      password.value = ''
      showPassword.value = false
    }
  },
  // O componente nasce via `v-if` com `open=true`; sem execução imediata o
  // snapshot e os palpites nunca eram aplicados na primeira abertura.
  { immediate: true }
)
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

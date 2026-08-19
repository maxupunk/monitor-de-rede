<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 620"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg pa-4">
      <v-card-title class="font-weight-bold">
        {{ editando ? 'Editar Regra de Alerta' : 'Cadastrar Regra de Alerta' }}
      </v-card-title>
      <v-card-subtitle class="pb-2">
        Monte a regra em linguagem simples: escolha o que medir, como comparar e a partir de qual
        valor o alerta deve disparar.
      </v-card-subtitle>

      <v-card-text>
        <v-form ref="formRef" @submit.prevent="salvar">
          <v-text-field
            v-model="form.name"
            label="Nome da Regra"
            placeholder="Ex.: Latência alta no link principal"
            variant="outlined"
            :rules="[(v: string) => !!v || 'Informe um nome para a regra']"
          />

          <!--
            O escopo é a **primeira** decisão, e não um detalhe escondido: é ele
            que separa "a CPU deste servidor" de "a CPU de qualquer equipamento".
            `null` significa todos — e é uma opção legítima, não a ausência de
            escolha.

            Aberto de dentro de um dispositivo, o seletor **restringe** em vez
            de travar: trocar o dono da regra por engano continua impedido
            (os outros equipamentos não aparecem na lista), mas a decisão
            legítima — "isto vale para o parque inteiro" — deixa de exigir sair
            da tela. Quem está olhando um equipamento é justamente quem percebe
            que a condição é de parque.
          -->
          <v-select
            v-model="form.deviceId"
            :items="opcoesDeDispositivo"
            item-title="title"
            item-value="value"
            label="Aplicar a"
            :hint="dicaDeEscopo"
            persistent-hint
            variant="outlined"
            class="mb-4"
          />

          <v-select
            v-model="form.field"
            :items="metricasOferecidas"
            item-title="title"
            item-value="field"
            label="O que monitorar (métrica alvo)"
            :hint="metricaSelecionada?.hint"
            persistent-hint
            variant="outlined"
            class="mb-4"
            @update:model-value="aoTrocarMetrica"
          />

          <v-row dense>
            <v-col cols="12" sm="6">
              <v-select
                v-model="form.operator"
                :items="operadoresDisponiveis"
                item-title="title"
                item-value="value"
                label="Quando o valor..."
                variant="outlined"
              />
            </v-col>

            <v-col cols="12" sm="6">
              <v-select
                v-if="metricaSelecionada?.kind === 'enum'"
                v-model="form.value"
                :items="metricaSelecionada.options"
                item-title="title"
                item-value="value"
                label="Valor de referência"
                variant="outlined"
              />
              <v-text-field
                v-else-if="metricaSelecionada?.kind === 'text'"
                v-model="form.value"
                label="Valor de referência"
                placeholder="Ex.: uplink"
                variant="outlined"
              />
              <DataRateInput
                v-else-if="metricaSelecionada?.unit === 'bps'"
                v-model="valorEmBps"
                label="Valor de referência"
              />
              <v-text-field
                v-else
                v-model.number="form.value"
                label="Valor de referência"
                type="number"
                :suffix="metricaSelecionada?.unit"
                variant="outlined"
              />
            </v-col>
          </v-row>

          <v-select
            v-model="form.durationSeconds"
            :items="ALERT_DURATIONS"
            item-title="title"
            item-value="value"
            label="Tolerância antes de disparar"
            hint="Evita alertas por oscilações momentâneas da rede."
            persistent-hint
            variant="outlined"
            class="mb-4"
          />

          <v-select
            v-model="form.recoveryWindowSeconds"
            :items="RECOVERY_WINDOWS"
            item-title="title"
            item-value="value"
            label="Estabilização antes de resolver"
            hint="Só resolve depois que o alvo se mantém estável por esse período; cada recaída reinicia a contagem."
            persistent-hint
            variant="outlined"
            class="mb-4"
          />

          <v-row dense class="mb-2">
            <v-col cols="12" md="6">
              <v-select
                v-model="form.flapThreshold"
                :items="FLAP_THRESHOLDS"
                item-title="title"
                item-value="value"
                label="Detecção de oscilação"
                hint="Alvo que recai demais é marcado como “oscilando” e para de notificar até estabilizar."
                persistent-hint
                variant="outlined"
              />
            </v-col>
            <v-col cols="12" md="6">
              <v-select
                v-model="form.flapWindowSeconds"
                :items="FLAP_WINDOWS"
                item-title="title"
                item-value="value"
                label="Janela de contagem das recaídas"
                :disabled="!form.flapThreshold"
                variant="outlined"
              />
            </v-col>
          </v-row>

          <v-alert
            v-if="flapNeedsRecoveryWindow(form.flapThreshold, form.recoveryWindowSeconds)"
            type="warning"
            variant="tonal"
            density="compact"
            class="mb-4"
          >
            A detecção de oscilação é medida sobre o episódio do alerta, que só sobrevive à
            oscilação quando há estabilização. Com “sem estabilização” o alerta resolve na primeira
            checagem ok e nunca chega a recair — escolha uma janela acima.
          </v-alert>

          <v-select
            v-model="form.notificationCooldownSeconds"
            :items="NOTIFICATION_COOLDOWNS"
            item-title="title"
            item-value="value"
            label="Intervalo entre notificações"
            hint="Vale mesmo quando o alerta fecha e um novo abre. A resolução acompanha o disparo: se ele foi engolido, ela também é."
            persistent-hint
            variant="outlined"
            class="mb-4"
          />

          <v-switch
            v-model="form.inhibitWhenParentDown"
            color="primary"
            label="Silenciar quando o equipamento-pai já está em alerta"
            hint="Um roteador que cai leva junto tudo que está atrás dele. Com esta opção, só o alerta do pai chega ao canal."
            persistent-hint
            density="compact"
            class="mb-4"
          />

          <v-select
            v-model="form.severity"
            :items="ALERT_SEVERITIES"
            item-title="title"
            item-value="value"
            label="Nível de severidade"
            variant="outlined"
          />

          <v-alert type="info" variant="tonal" density="comfortable" class="mt-2">
            <div class="text-caption font-weight-bold mb-1">Resumo da regra</div>
            <div class="text-body-2">{{ resumo }}</div>
          </v-alert>
        </v-form>
      </v-card-text>

      <v-card-actions class="justify-end">
        <v-btn variant="text" @click="fechar">Cancelar</v-btn>
        <v-btn color="primary" :loading="salvando" @click="salvar">
          {{ editando ? 'Salvar Alterações' : 'Salvar Regra' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * O formulário de regra — **um só**, nas duas telas.
 *
 * Antes ele vivia inline em `AlertsPage.vue`, e a aba Regras do dispositivo
 * só sabia mandar o operador para lá por um link. Duas consequências: criar
 * uma regra a partir do equipamento perdia o contexto (o dispositivo tinha de
 * ser escolhido de novo, do zero) e qualquer campo novo precisaria nascer duas
 * vezes. É o item da Fase 6 do roadmap — "criar ou editar uma regra usa um
 * único componente compartilhado nas duas páginas".
 *
 * O escopo é um campo do formulário, e `null` ("Todos os dispositivos") é uma
 * escolha válida: existem regras genuinamente globais, como indisponibilidade.
 * Quando o diálogo é aberto de dentro de um dispositivo, o escopo vem
 * preenchido e **restrito a duas opções** — aquele equipamento e o parque
 * inteiro. Trocar o dono da regra por engano continua impedido (os outros
 * equipamentos não aparecem na lista); a decisão legítima de generalizar, não.
 */
import { computed, reactive, ref, watch } from 'vue'
import DataRateInput from '@/components/DataRateInput.vue'
import { useAlertsStore, type AlertRule } from '@/stores/alerts'
import { useDevicesStore } from '@/stores/devices'
import {
  ALERT_DURATIONS,
  ALERT_METRICS,
  ALERT_SEVERITIES,
  FLAP_THRESHOLDS,
  FLAP_WINDOWS,
  NOTIFICATION_COOLDOWNS,
  RECOVERY_WINDOWS,
  describeRule,
  findMetric,
  flapNeedsRecoveryWindow,
  operatorsForMetric,
  type AlertOperator,
} from '@/utils/alertPresentation'

const props = defineProps<{
  modelValue: boolean
  /** Regra a editar. Ausente = cadastro novo. */
  rule?: AlertRule | null
  /**
   * Dispositivo de origem: o diálogo foi aberto de dentro dele.
   *
   * Pré-seleciona o escopo e **restringe** as opções a duas — este
   * equipamento e "Todos os dispositivos". Não trava: uma regra genuinamente
   * de parque continua podendo nascer daqui.
   */
  fixedDeviceId?: number | null
  /** Nome do dispositivo de origem, para rotular a opção antes de a lista chegar. */
  fixedDeviceName?: string | null
  /** Dispositivo apenas pré-selecionado; o operador ainda pode trocar. */
  defaultDeviceId?: number | null
  /**
   * Campos que o dispositivo do escopo publica. Quando informado, o seletor de
   * métrica só oferece o que pode de fato disparar ali — a mesma regra de
   * aplicabilidade do catálogo, aplicada à criação personalizada.
   *
   * A restrição acompanha o escopo escolhido: ao trocar para "Todos os
   * dispositivos", o vocabulário volta a ser o completo, porque o escopo
   * deixou de ser aquele equipamento.
   */
  availableFields?: string[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', rule: AlertRule | null): void
}>()

const alertsStore = useAlertsStore()
const devicesStore = useDevicesStore()

const formRef = ref()
const salvando = ref(false)

const form = reactive({
  name: '',
  deviceId: null as number | null,
  field: 'latencyMs',
  operator: 'gt' as AlertOperator,
  value: 150 as number | string,
  durationSeconds: 0,
  recoveryWindowSeconds: 0,
  flapThreshold: 0,
  flapWindowSeconds: 900,
  notificationCooldownSeconds: 0,
  inhibitWhenParentDown: false,
  severity: 'warning' as AlertRule['severity'],
})

const editando = computed(() => props.rule != null)

/** O diálogo nasceu de dentro de um equipamento: duas opções, não todas. */
const escopoRestrito = computed(() => props.fixedDeviceId != null)

const TODOS_OS_DISPOSITIVOS = { title: 'Todos os dispositivos', value: null }

const opcoesDeDispositivo = computed(() => {
  const inventario = devicesStore.devices.map((device) => ({
    title: device.name,
    value: device.id as number | null,
  }))
  if (!escopoRestrito.value) return [TODOS_OS_DISPOSITIVOS, ...inventario]
  // A origem entra sempre, inclusive antes de a lista de dispositivos chegar:
  // um seletor sem a opção selecionada apareceria vazio.
  const origem = inventario.find((opcao) => opcao.value === props.fixedDeviceId) ?? {
    title: props.fixedDeviceName ?? 'Este dispositivo',
    value: props.fixedDeviceId as number | null,
  }
  return [origem, TODOS_OS_DISPOSITIVOS]
})

const dicaDeEscopo = computed(() => {
  if (form.deviceId == null) {
    return escopoRestrito.value
      ? 'A regra passa a valer para todo o inventário, e não só para este equipamento.'
      : 'A regra vale para todo o inventário. Use para condições genéricas, como indisponibilidade.'
  }
  return escopoRestrito.value
    ? 'A regra fica vinculada a este equipamento. Escolha "Todos os dispositivos" para valer no parque inteiro.'
    : 'A regra fica vinculada a este equipamento e só é avaliada nas checagens dele.'
})

/**
 * As métricas oferecidas.
 *
 * Sem `availableFields` (a Central de Alertas, sem dispositivo escolhido) o
 * vocabulário inteiro aparece. Com ele, oferecer uma métrica que o equipamento
 * não publica produziria uma regra que nunca dispara — o mesmo motivo pelo qual
 * o catálogo desabilita templates inaplicáveis.
 */
const metricasOferecidas = computed(() => {
  const permitidos = props.availableFields
  // Escopo global: o vocabulário é o completo. Restringir pelo que **um**
  // equipamento publica não faz sentido quando a regra vale para todos.
  if (form.deviceId == null) return ALERT_METRICS
  if (!permitidos || permitidos.length === 0) return ALERT_METRICS
  return ALERT_METRICS.filter((metrica) => permitidos.includes(metrica.field))
})

const metricaSelecionada = computed(() => findMetric(form.field))
const operadoresDisponiveis = computed(() => operatorsForMetric(form.field))

/** Ponte tipada para o `DataRateInput`, que trabalha só em bps. */
const valorEmBps = computed<number | null>({
  get: () => (typeof form.value === 'number' ? form.value : Number(form.value) || null),
  set: (value) => {
    form.value = value ?? 0
  },
})

const resumo = computed(() =>
  describeRule(
    { field: form.field, operator: form.operator, value: form.value },
    form.durationSeconds,
    form.recoveryWindowSeconds,
    form.flapThreshold,
    form.flapWindowSeconds,
    {
      notificationCooldownSeconds: form.notificationCooldownSeconds,
      inhibitWhenParentDown: form.inhibitWhenParentDown,
    }
  )
)

/**
 * Trocar o escopo pode invalidar a métrica escolhida.
 *
 * Vindo de "Todos os dispositivos" para o equipamento, o vocabulário encolhe
 * para o que ele publica — e um campo fora dessa lista ficaria selecionado num
 * seletor que nem o oferece, produzindo uma regra que nunca dispara. Só o
 * cadastro novo é reajustado: uma regra existente em edição mantém a condição
 * que o operador gravou.
 */
watch(metricasOferecidas, (oferecidas) => {
  if (props.rule) return
  if (oferecidas.some((metrica) => metrica.field === form.field)) return
  const primeira = oferecidas[0] ?? ALERT_METRICS[0]
  form.field = primeira.field
  form.operator = primeira.defaultOperator
  form.value = primeira.defaultValue
})

function aoTrocarMetrica(field: string) {
  const metrica = findMetric(field)
  if (!metrica) return
  form.operator = metrica.defaultOperator
  form.value = metrica.defaultValue
}

/** Preenche o formulário ao abrir — com a regra em edição, ou do zero. */
function preencher() {
  const regra = props.rule
  if (regra) {
    form.name = regra.name
    form.deviceId = regra.deviceId ?? null
    form.field = regra.condition?.field ?? 'latencyMs'
    form.operator = (regra.condition?.operator ?? 'gt') as AlertOperator
    form.value = regra.condition?.value ?? 0
    form.durationSeconds = regra.durationSeconds ?? 0
    form.recoveryWindowSeconds = regra.recoveryWindowSeconds ?? 0
    form.flapThreshold = regra.flapThreshold ?? 0
    form.flapWindowSeconds = regra.flapWindowSeconds ?? 900
    form.notificationCooldownSeconds = regra.notificationCooldownSeconds ?? 0
    form.inhibitWhenParentDown = regra.inhibitWhenParentDown ?? false
    form.severity = regra.severity ?? 'warning'
    return
  }

  // Cadastro novo: a primeira métrica **oferecida**, e não uma fixa. Num
  // dispositivo que não mede latência, abrir o formulário já em `latencyMs`
  // mostraria um campo que o seletor nem lista.
  const primeira = metricasOferecidas.value[0] ?? ALERT_METRICS[0]
  form.name = ''
  form.deviceId = props.fixedDeviceId ?? props.defaultDeviceId ?? null
  form.field = primeira.field
  form.operator = primeira.defaultOperator
  form.value = primeira.defaultValue
  form.durationSeconds = 0
  form.recoveryWindowSeconds = 0
  form.flapThreshold = 0
  form.flapWindowSeconds = 900
  form.notificationCooldownSeconds = 0
  form.inhibitWhenParentDown = false
  form.severity = 'warning'
}

watch(
  () => props.modelValue,
  (aberto) => {
    if (!aberto) return
    preencher()
    // O seletor de escopo mostra **nome**, não id.
    if (devicesStore.devices.length === 0) void devicesStore.fetchDevices()
  }
)

function montarPayload(): Partial<AlertRule> {
  const metrica = metricaSelecionada.value
  // Métrica numérica precisa chegar como número para o comparador do backend.
  const valor =
    metrica?.kind === 'number' && form.value !== '' ? Number(form.value) : String(form.value)

  return {
    name: form.name,
    type: 'custom',
    deviceId: form.deviceId,
    condition: { field: form.field, operator: form.operator, value: valor },
    durationSeconds: form.durationSeconds,
    recoveryWindowSeconds: form.recoveryWindowSeconds,
    flapThreshold: form.flapThreshold,
    flapWindowSeconds: form.flapWindowSeconds,
    notificationCooldownSeconds: form.notificationCooldownSeconds,
    inhibitWhenParentDown: form.inhibitWhenParentDown,
    severity: form.severity,
    enabled: true,
  }
}

function fechar() {
  emit('update:modelValue', false)
}

async function salvar() {
  const validacao = await formRef.value?.validate()
  if (validacao && validacao.valid === false) return
  if (!form.name) return

  salvando.value = true
  const ok = props.rule
    ? await alertsStore.updateAlertRule(props.rule.id, montarPayload())
    : await alertsStore.createAlertRule(montarPayload())
  salvando.value = false

  if (!ok) return
  emit('saved', props.rule ?? null)
  fechar()
}
</script>

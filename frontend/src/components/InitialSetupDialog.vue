<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 860"
    :fullscreen="$vuetify.display.xs"
    persistent
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg initial-setup-card d-flex flex-column">
      <!-- Cabeçalho do Diálogo -->
      <v-card-item class="pa-5 pb-4 border-b bg-surface">
        <template #prepend>
          <v-avatar color="primary" size="48" rounded="lg" class="elevation-2">
            <v-icon size="28" color="white">mdi-rocket-launch-outline</v-icon>
          </v-avatar>
        </template>
        <div class="d-flex align-center ga-2 flex-wrap">
          <v-card-title class="font-weight-bold text-h6 pa-0">
            Assistente de Configuração Inicial
          </v-card-title>
          <v-chip size="x-small" color="primary" variant="tonal" class="font-weight-bold">
            Primeiro Acesso
          </v-chip>
        </div>
        <v-card-subtitle class="mt-1 text-wrap text-body-2">
          Configure os parâmetros essenciais do NetMonitor para iniciar o monitoramento do seu
          ambiente.
        </v-card-subtitle>
        <template #append>
          <v-btn
            v-if="!reviewing && !applying"
            variant="text"
            size="small"
            color="primary"
            prepend-icon="mdi-close"
            class="text-none"
            @click="handleSkip"
          >
            Pular
          </v-btn>
        </template>
      </v-card-item>

      <!-- Indicador Visual de Etapas -->
      <div v-if="!reviewing && currentStep" class="px-5 pt-3 pb-2 border-b">
        <div class="d-flex align-center justify-space-between mb-2">
          <span class="text-caption font-weight-bold text-primary">
            Etapa {{ stepIndex + 1 }} de {{ steps.length }}: {{ currentStep.title }}
          </span>
          <span class="text-caption font-weight-medium">{{ progress }}% concluído</span>
        </div>
        <v-progress-linear
          :model-value="progress"
          color="primary"
          height="6"
          rounded
        ></v-progress-linear>

        <!-- Barra de ícones das etapas -->
        <div class="d-none d-sm-flex justify-space-between align-center mt-3 px-1">
          <div
            v-for="(step, idx) in steps"
            :key="step.id"
            class="d-flex flex-column align-center cursor-pointer step-pill"
            :class="{
              'text-primary font-weight-bold': stepIndex === idx,
              'text-success': stepIndex > idx,
            }"
            :data-step="step.id"
            @click="goToStep(idx)"
          >
            <v-avatar
              :color="
                stepIndex === idx ? 'primary' : stepIndex > idx ? 'success' : 'grey-lighten-2'
              "
              size="26"
              class="mb-1"
              :variant="stepIndex > idx ? 'tonal' : 'flat'"
            >
              <v-icon
                size="14"
                :color="stepIndex === idx ? 'white' : stepIndex > idx ? 'success' : 'grey-darken-2'"
              >
                {{ stepIndex > idx ? 'mdi-check' : step.icon }}
              </v-icon>
            </v-avatar>
            <span class="text-caption font-weight-medium" style="font-size: 0.68rem !important">
              {{ step.title }}
            </span>
          </div>
        </div>
      </div>

      <!-- Conteúdo das Etapas -->
      <v-card-text class="pa-5 flex-grow-1 overflow-y-auto">
        <div v-if="reviewing" class="py-2">
          <SetupDoneStep
            v-if="appliedSuccess"
            :network="visibleIds.has('network')"
            :scan-started="form.network.enabled && form.network.scanNow"
            :servers="visibleIds.has('agents')"
            @navigate="finishAndNavigate"
          />
          <SetupReviewStep
            v-else
            :form="form"
            :steps="visibleIds"
            :show-vpn-address="areasStore.isEnabled('vpn')"
            :apps-summary="fleetApps.length > 0 ? appsSummary : null"
            :applying="applying"
            :status-message="applyStatusMessage"
            :error="applyError"
            @clear-error="applyError = null"
          />
        </div>
        <div v-else-if="currentStep" class="py-2">
          <SetupIntentStep
            v-if="currentStep.id === 'intent'"
            v-model="intent"
            :error="intentError"
          />
          <SetupSiteStep v-else-if="currentStep.id === 'site'" v-model="form.site" />
          <SetupAddressesStep
            v-else-if="currentStep.id === 'addresses'"
            v-model="form.addresses"
            :detecting="detectingPublicIp"
            :show-vpn="areasStore.isEnabled('vpn')"
            @detect="handleDetectPublicIp"
          />
          <SetupNetworkStep v-else-if="currentStep.id === 'network'" v-model="form.network" />
          <SetupDnsStep v-else-if="currentStep.id === 'dns'" v-model="form.dns" />
          <SetupVpnStep v-else-if="currentStep.id === 'vpn'" v-model="form.vpn" />
          <SetupAppsStep
            v-else-if="currentStep.id === 'apps'"
            v-model="form.apps"
            :apps="fleetApps"
          />
          <SetupAgentsStep v-else-if="currentStep.id === 'agents'" />
          <SetupGeneralStep v-else-if="currentStep.id === 'general'" v-model="form.preferences" />
          <SetupNotificationsStep v-else-if="currentStep.id === 'notifications'" />
        </div>
      </v-card-text>

      <!-- Ações do Rodapé -->
      <v-divider></v-divider>
      <v-card-actions class="pa-4 bg-surface justify-space-between align-center">
        <div>
          <v-btn
            v-if="canGoBack"
            variant="text"
            color="primary"
            prepend-icon="mdi-arrow-left"
            @click="goBack"
          >
            Voltar
          </v-btn>
        </div>

        <div class="d-flex align-center ga-2">
          <v-btn
            v-if="currentStep?.id === 'intent' && !reviewing"
            color="primary"
            variant="flat"
            append-icon="mdi-arrow-right"
            class="font-weight-bold px-5"
            :loading="submittingIntent"
            @click="submitIntent"
          >
            Continuar
          </v-btn>
          <v-btn
            v-else-if="!reviewing && !isLastStep"
            color="primary"
            variant="flat"
            append-icon="mdi-arrow-right"
            class="font-weight-bold px-5"
            @click="stepIndex++"
          >
            Avançar
          </v-btn>
          <v-btn
            v-else-if="!reviewing"
            color="primary"
            variant="flat"
            append-icon="mdi-check-decagram-outline"
            class="font-weight-bold px-5"
            @click="reviewing = true"
          >
            Revisar Configurações
          </v-btn>
          <v-btn
            v-else-if="!appliedSuccess"
            color="success"
            variant="flat"
            size="large"
            prepend-icon="mdi-content-save-check"
            class="font-weight-bold px-6"
            :loading="applying"
            @click="handleApplyAll"
          >
            Salvar e Iniciar
          </v-btn>
        </div>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRouter, type RouteLocationRaw } from 'vue-router'
import type { OnboardingIntentInput } from '@/bindings/OnboardingIntentInput'
import { useAreasStore } from '@/stores/areas'
import { useAuthStore } from '@/stores/auth'
import { useOnboardingStore } from '@/stores/onboarding'
import { usePluginsStore } from '@/stores/plugins'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { useSitesStore } from '@/stores/sites'
import { useNetworksStore } from '@/stores/networks'
import { useDnsServersStore } from '@/stores/dnsServers'
import { useServerAddressesStore } from '@/stores/serverAddresses'
import { useVpnStore } from '@/stores/vpn'
import { usePreferencesStore } from '@/stores/preferences'
import SetupAddressesStep from '@/components/setup/SetupAddressesStep.vue'
import SetupAgentsStep from '@/components/setup/SetupAgentsStep.vue'
import SetupAppsStep from '@/components/setup/SetupAppsStep.vue'
import SetupDnsStep from '@/components/setup/SetupDnsStep.vue'
import SetupDoneStep from '@/components/setup/SetupDoneStep.vue'
import SetupGeneralStep from '@/components/setup/SetupGeneralStep.vue'
import SetupIntentStep from '@/components/setup/SetupIntentStep.vue'
import SetupNetworkStep from '@/components/setup/SetupNetworkStep.vue'
import SetupNotificationsStep from '@/components/setup/SetupNotificationsStep.vue'
import SetupReviewStep from '@/components/setup/SetupReviewStep.vue'
import SetupSiteStep from '@/components/setup/SetupSiteStep.vue'
import SetupVpnStep from '@/components/setup/SetupVpnStep.vue'
import {
  createSetupForm,
  POPULAR_DNS_PRESETS,
  suggestNetworkFromIp,
} from '@/components/setup/setupForm'
import { isIntentValid, setupSteps, type SetupStepId } from '@/components/setup/steps'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'completed'): void
  (e: 'skipped'): void
}>()

const router = useRouter()
const auth = useAuthStore()
const areasStore = useAreasStore()
const onboardingStore = useOnboardingStore()
const sitesStore = useSitesStore()
const networksStore = useNetworksStore()
const dnsServersStore = useDnsServersStore()
const serverAddressesStore = useServerAddressesStore()
const vpnStore = useVpnStore()
const prefsStore = usePreferencesStore()
const pluginsStore = usePluginsStore()
const pluginAppsStore = usePluginAppsStore()

/** As etapas valem para as áreas ligadas; a escolha das áreas é só do administrador. */
const steps = computed(() => setupSteps((area) => areasStore.isEnabled(area), auth.isAdmin))
const visibleIds = computed<ReadonlySet<SetupStepId>>(
  () => new Set(steps.value.map((step) => step.id))
)
const stepIndex = ref(0)
/** Depois da última etapa: revisão e, aplicada, a tela de conclusão. */
const reviewing = ref(false)
const currentStep = computed(() => steps.value[stepIndex.value] ?? null)
const isLastStep = computed(() => stepIndex.value >= steps.value.length - 1)
const progress = computed(() =>
  steps.value.length > 1 ? Math.round((stepIndex.value / (steps.value.length - 1)) * 100) : 100
)

const fleetApps = computed(() =>
  pluginsStore.library.filter(
    (plugin) => plugin.surfaces.includes('fleet') && plugin.status !== 'quarantine'
  )
)

const form = reactive(createSetupForm())

const appsSummary = computed(() => {
  const on = fleetApps.value.filter((app) => form.apps[app.id])
  return on.length > 0
    ? on.map((app) => app.fleet?.title ?? app.name).join(', ')
    : 'Nenhum ligado agora (ligam ao cadastrar um equipamento compatível)'
})

/** Primeira etapa: o que a instalação vai gerenciar, a partir do estado atual. */
const intent = ref<OnboardingIntentInput>({ network: true, servers: true, vpn: true })
const intentError = ref<string | null>(null)
const submittingIntent = ref(false)

const detectingPublicIp = ref(false)
const applying = ref(false)
const applyStatusMessage = ref('')
const applyError = ref<string | null>(null)
const appliedSuccess = ref(false)

const canGoBack = computed(
  () => !applying.value && !appliedSuccess.value && (reviewing.value || stepIndex.value > 0)
)

function currentIntent(): OnboardingIntentInput {
  return {
    network: areasStore.isEnabled('network'),
    servers: areasStore.isEnabled('servers'),
    vpn: areasStore.isEnabled('vpn'),
  }
}

function sameIntent(a: OnboardingIntentInput, b: OnboardingIntentInput): boolean {
  return a.network === b.network && a.servers === b.servers && a.vpn === b.vpn
}

/** Grava a escolha das áreas (se mudou) e segue para a próxima etapa. */
async function submitIntent() {
  intentError.value = null
  if (!isIntentValid(intent.value)) return
  const unchanged =
    onboardingStore.status?.intentChosen === true && sameIntent(intent.value, currentIntent())
  if (!unchanged) {
    submittingIntent.value = true
    try {
      await areasStore.applyIntent({ ...intent.value })
      onboardingStore.markIntentChosen()
    } catch (err: unknown) {
      intentError.value =
        err instanceof Error ? err.message : 'Não foi possível gravar a escolha das áreas.'
      return
    } finally {
      submittingIntent.value = false
    }
  }
  stepIndex.value = 1
}

function goToStep(index: number) {
  const leavingIntent = currentStep.value?.id === 'intent'
  if (index <= stepIndex.value || (index === stepIndex.value + 1 && !leavingIntent)) {
    stepIndex.value = index
  }
}

function goBack() {
  if (reviewing.value) {
    reviewing.value = false
    stepIndex.value = steps.value.length - 1
  } else if (stepIndex.value > 0) {
    stepIndex.value--
  }
}

function applyLanSuggestion(ip: string) {
  form.addresses.lan = ip
  const suggestion = suggestNetworkFromIp(ip)
  if (suggestion) {
    form.network.cidr = suggestion.cidr
    form.network.gateway = suggestion.gateway
  }
}

/** Preenche valores sugeridos inteligentes a partir dos dados detectados */
async function initializeDefaults() {
  // Preseleciona Cloudflare e Google por padrão
  if (form.dns.length === 0) {
    form.dns = POPULAR_DNS_PRESETS.slice(0, 2).map((preset) => ({ ...preset }))
  }

  // Carrega status de onboarding e endereços do servidor
  const st = await onboardingStore.fetchStatus()
  await serverAddressesStore.fetchAll()

  if (st?.detectedLanIp) {
    applyLanSuggestion(st.detectedLanIp)
  } else {
    const lanEntry = serverAddressesStore.entries.find((e) => e.kind === 'lan')
    const ip = lanEntry?.value || lanEntry?.detected
    if (ip) applyLanSuggestion(ip)
  }

  if (st?.detectedPublicIp) {
    form.addresses.public = st.detectedPublicIp
    form.vpn.publicEndpoint = st.detectedPublicIp
  }

  const vpnEntry = serverAddressesStore.entries.find((e) => e.kind === 'vpn')
  const vpnIp = vpnEntry?.value || vpnEntry?.detected
  if (vpnIp) form.addresses.vpn = vpnIp

  // Carrega preferências atuais
  await prefsStore.fetchAll()
  form.preferences = { ...prefsStore.preferences }

  // Aplicativos (plugins com página de frota) e se já estão ligados
  try {
    await pluginsStore.fetchLibrary()
  } catch {
    // Sem permissão de leitura da biblioteca: a etapa só fica vazia.
  }
  form.apps = Object.fromEntries(fleetApps.value.map((app) => [app.id, app.status !== 'disabled']))
}

async function handleDetectPublicIp() {
  detectingPublicIp.value = true
  try {
    const ip = await vpnStore.detectEndpoint()
    if (ip) {
      form.addresses.public = ip
      form.vpn.publicEndpoint = ip
    }
  } finally {
    detectingPublicIp.value = false
  }
}

function handleSkip() {
  onboardingStore.dismissWizard(true)
  emit('update:modelValue', false)
  emit('skipped')
}

/** Cria o Site (ou reaproveita um de mesmo nome) e devolve o id. */
async function applySite(): Promise<number | null> {
  if (!form.site.enabled || !form.site.name.trim()) return null
  applyStatusMessage.value = 'Criando Site de identificação...'
  await sitesStore.fetchSites()
  const name = form.site.name.trim()
  const existing = sitesStore.sites.find((s) => s.name.trim().toLowerCase() === name.toLowerCase())
  if (existing) return existing.id
  const ok = await sitesStore.createSite({
    name,
    location: form.site.location.trim() || undefined,
    description: form.site.description.trim() || undefined,
  })
  if (!ok) return null
  await sitesStore.fetchSites()
  return sitesStore.sites.find((s) => s.name === name)?.id ?? null
}

async function applyAddresses() {
  applyStatusMessage.value = 'Configurando endereços deste servidor...'
  const overrides: Record<string, string> = {}
  if (form.addresses.lan.trim()) overrides.lan = form.addresses.lan.trim()
  if (form.addresses.public.trim()) overrides.public = form.addresses.public.trim()
  if (areasStore.isEnabled('vpn') && form.addresses.vpn.trim()) {
    overrides.vpn = form.addresses.vpn.trim()
  }
  const domain = form.addresses.domain.trim()
  if (domain) overrides.domain = domain

  // Com domínio, ele é o padrão (sem marcação explícita); sem, a rede local.
  await serverAddressesStore.save({
    overrides,
    custom: [],
    preferredId: domain ? null : 'lan',
    domainHttps: Boolean(domain) && form.addresses.domainHttps,
  })
}

async function applyNetwork(siteId: number | null) {
  if (!form.network.enabled || !form.network.name.trim() || !form.network.cidr.trim()) return
  applyStatusMessage.value = 'Cadastrando sub-rede local...'
  const cidr = form.network.cidr.trim()
  await networksStore.fetchNetworks()
  const existingNet = networksStore.networks.find((n) => n.cidr.trim() === cidr)

  let targetNetId = existingNet?.id ?? null
  if (!existingNet) {
    const createdNet = await networksStore.createNetwork({
      name: form.network.name.trim(),
      cidr,
      gateway: form.network.gateway.trim() || undefined,
      vlanId: form.network.vlanId || undefined,
      siteId,
      scanEnabled: form.network.scanEnabled,
    })
    if (createdNet) {
      await networksStore.fetchNetworks()
      targetNetId = networksStore.networks.find((n) => n.cidr.trim() === cidr)?.id ?? null
    }
  }

  // Disparar varredura imediata se solicitado
  if (form.network.scanNow && targetNetId) {
    applyStatusMessage.value = 'Iniciando varredura da rede...'
    void networksStore.scanNetwork(targetNetId)
  }
}

async function applyDns() {
  if (form.dns.length === 0) return
  applyStatusMessage.value = 'Cadastrando servidores DNS...'
  await dnsServersStore.fetchServers(true)
  for (const dns of form.dns) {
    if (!dnsServersStore.findByAddress(dns.address, dns.protocol)) {
      await dnsServersStore.createServer({ ...dns })
    }
  }
}

async function applyVpn() {
  if (!form.vpn.enabled) return
  applyStatusMessage.value = 'Salvando parâmetros do servidor WireGuard...'
  await vpnStore.saveServer({
    publicEndpoint: form.vpn.publicEndpoint.trim() || null,
    listenPort: form.vpn.listenPort || 51820,
    cidr: form.vpn.cidr.trim() || '10.8.0.0/24',
  })
}

/** Liga/desliga aplicativos (só o que mudou). */
async function applyApps() {
  const changedApps = fleetApps.value.filter(
    (app) => (form.apps[app.id] ?? false) !== (app.status !== 'disabled')
  )
  if (changedApps.length === 0) return
  applyStatusMessage.value = 'Ligando os aplicativos escolhidos...'
  for (const app of changedApps) {
    await pluginsStore.lifecycle(app.id, form.apps[app.id] ? 'enable' : 'disable')
  }
  await pluginAppsStore.fetchApps()
}

/** Grava o que as etapas percorridas configuraram — nada de uma área desligada. */
async function handleApplyAll() {
  applying.value = true
  applyError.value = null
  const visible = visibleIds.value

  try {
    const siteId = visible.has('site') ? await applySite() : null
    await applyAddresses()
    if (visible.has('network')) await applyNetwork(siteId)
    if (visible.has('dns')) await applyDns()
    if (visible.has('vpn')) await applyVpn()
    if (visible.has('apps')) await applyApps()
    if (visible.has('general')) {
      applyStatusMessage.value = 'Gravando preferências globais...'
      await prefsStore.save({ ...form.preferences })
    }

    applyStatusMessage.value = 'Finalizando assistente...'
    await onboardingStore.completeOnboarding()

    appliedSuccess.value = true
    emit('completed')
  } catch (err: unknown) {
    applyError.value =
      err instanceof Error ? err.message : 'Ocorreu um erro ao aplicar as configurações.'
  } finally {
    applying.value = false
  }
}

function finishAndNavigate(to: RouteLocationRaw) {
  emit('update:modelValue', false)
  void router.push(to)
}

function reset() {
  stepIndex.value = 0
  reviewing.value = false
  appliedSuccess.value = false
  applyError.value = null
  intentError.value = null
  intent.value = currentIntent()
  void initializeDefaults()
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) reset()
  }
)

onMounted(() => {
  if (props.modelValue) reset()
})
</script>

<style scoped>
.initial-setup-card {
  max-height: 90vh;
}
.step-pill {
  transition: all 0.2s ease;
  user-select: none;
}
.cursor-pointer {
  cursor: pointer;
}
.step-pill:hover {
  opacity: 0.85;
}
</style>

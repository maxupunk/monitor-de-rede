<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 760"
    :fullscreen="$vuetify.display.xs"
    persistent
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center">
        <v-icon start color="primary">mdi-shield-plus-outline</v-icon>
        Adicionar Dispositivo VPN
      </v-card-title>

      <v-card-text>
        <v-stepper v-model="step" :items="stepLabels" flat hide-actions>
          <!-- Passo 1: perfil do equipamento -->
          <template #item.1>
            <div class="text-subtitle-1 font-weight-medium mb-1">Qual equipamento?</div>
            <div class="text-caption text-grey-darken-1 mb-4">
              A escolha define o formato do artefato gerado — roteadores recebem script para colar
              no terminal.
            </div>

            <v-row>
              <v-col v-for="option in profiles" :key="option.profile" cols="12" sm="6" md="4">
                <v-card
                  :color="form.profile === option.profile ? 'primary' : undefined"
                  :variant="form.profile === option.profile ? 'flat' : 'outlined'"
                  class="pa-4 text-center h-100 cursor-pointer"
                  @click="form.profile = option.profile"
                >
                  <v-icon size="40" class="mb-2">{{ option.icon }}</v-icon>
                  <div class="text-body-2 font-weight-medium">{{ option.label }}</div>
                  <v-chip
                    size="x-small"
                    class="mt-2"
                    :color="form.profile === option.profile ? 'white' : 'grey'"
                    variant="tonal"
                  >
                    {{ option.supportsQrCode ? 'QR Code' : 'Script / arquivo' }}
                  </v-chip>
                </v-card>
              </v-col>
            </v-row>
          </template>

          <!-- Passo 2: identificação -->
          <template #item.2>
            <v-row>
              <v-col cols="12" md="6">
                <v-text-field
                  v-model="form.name"
                  label="Nome do dispositivo *"
                  variant="outlined"
                  density="comfortable"
                  hint="Ex.: MikroTik Filial Centro"
                  persistent-hint
                ></v-text-field>
              </v-col>
              <v-col cols="12" md="6">
                <v-text-field
                  v-model="form.ipAddress"
                  label="IP fixo na VPN"
                  variant="outlined"
                  density="comfortable"
                  :hint="`Sugerido automaticamente dentro de ${cidr || 'faixa da VPN'}`"
                  persistent-hint
                  append-inner-icon="mdi-refresh"
                  @click:append-inner="suggestIp"
                ></v-text-field>
              </v-col>

              <v-col cols="12" md="6">
                <v-select
                  v-model="form.siteId"
                  :items="siteOptions"
                  item-title="name"
                  item-value="id"
                  label="Site"
                  variant="outlined"
                  density="comfortable"
                  clearable
                ></v-select>
              </v-col>

              <v-col cols="12" md="6" class="d-flex align-center">
                <v-switch
                  v-model="form.snmpEnabled"
                  color="primary"
                  label="Monitorar via SNMP"
                  density="comfortable"
                  hide-details
                ></v-switch>
              </v-col>

              <v-col v-if="form.snmpEnabled" cols="12" md="6">
                <v-text-field
                  v-model="form.snmpCommunity"
                  label="Community SNMP"
                  variant="outlined"
                  density="comfortable"
                ></v-text-field>
              </v-col>
            </v-row>

            <v-alert type="info" variant="tonal" density="comfortable" class="mt-2">
              O ping e o SNMP são criados automaticamente e atribuídos ao probe da VPN. Você nunca
              precisa digitar chaves — elas são geradas pelo sistema.
            </v-alert>
          </template>
        </v-stepper>

        <v-alert
          v-if="vpnStore.error"
          type="error"
          variant="tonal"
          class="mt-4"
          density="comfortable"
        >
          {{ vpnStore.error }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="px-4 pb-4">
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-spacer></v-spacer>
        <v-btn v-if="step > 1" variant="text" @click="step--">Voltar</v-btn>
        <v-btn
          v-if="step === 1"
          color="primary"
          variant="flat"
          :disabled="!form.profile"
          @click="goToIdentification"
        >
          Continuar
        </v-btn>
        <v-btn
          v-else
          color="primary"
          variant="flat"
          :loading="vpnStore.saving"
          :disabled="!form.name.trim()"
          @click="submit"
        >
          Gerar configuração
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useVpnStore, type VpnArtifact } from '@/stores/vpn'
import { useSitesStore } from '@/stores/sites'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'created', artifact: VpnArtifact): void
}>()

const vpnStore = useVpnStore()
const sitesStore = useSitesStore()

const step = ref(1)
const stepLabels = ['Equipamento', 'Identificação']

const form = reactive<{
  // O perfil vem do registro do backend (`vpnStore.profiles`), não de uma união
  // redigitada aqui — quem valida a chave é o controller Rust.
  profile: string | null
  name: string
  ipAddress: string
  siteId: number | null
  snmpEnabled: boolean
  snmpCommunity: string
}>({
  profile: null,
  name: '',
  ipAddress: '',
  siteId: null,
  snmpEnabled: false,
  snmpCommunity: 'public',
})

const profiles = computed(() => vpnStore.profiles)
const cidr = computed(() => vpnStore.state?.cidr ?? null)
const siteOptions = computed(() => sitesStore.sites)

watch(
  () => props.modelValue,
  async (isOpen) => {
    if (!isOpen) return

    step.value = 1
    form.profile = null
    form.name = ''
    form.ipAddress = ''
    form.siteId = null
    form.snmpEnabled = false
    form.snmpCommunity = 'public'

    if (sitesStore.sites.length === 0) {
      await sitesStore.fetchSites()
    }
  }
)

async function suggestIp() {
  const suggestion = await vpnStore.suggestNextIp()
  if (suggestion) form.ipAddress = suggestion
}

async function goToIdentification() {
  step.value = 2
  if (!form.ipAddress) {
    await suggestIp()
  }
}

function close() {
  emit('update:modelValue', false)
}

async function submit() {
  if (!form.profile || !form.name.trim()) return

  const artifact = await vpnStore.createPeer({
    name: form.name.trim(),
    profile: form.profile,
    ipAddress: form.ipAddress || null,
    siteId: form.siteId,
    snmpEnabled: form.snmpEnabled,
    snmpCommunity: form.snmpEnabled ? form.snmpCommunity : null,
  })

  if (artifact) {
    emit('created', artifact)
    close()
  }
}
</script>

<style scoped>
.cursor-pointer {
  cursor: pointer;
}

/*
 * O v-stepper-window é um v-window (overflow: hidden) e o espaçamento padrão
 * vem de margin, que fica fora da área de recorte. Com a margem negativa do
 * v-row o primeiro campo encosta no topo e a label flutuante dos campos
 * outlined (translateY(-50%) sobre a borda) era cortada pela metade.
 * Trocamos parte da margem por padding para o recorte acontecer acima dela.
 */
:deep(.v-stepper-window) {
  margin-top: 0.75rem;
  padding-top: 0.75rem;
}
</style>

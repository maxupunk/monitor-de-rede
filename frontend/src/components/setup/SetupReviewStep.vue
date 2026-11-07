<template>
  <div>
    <div class="text-center mb-4">
      <v-avatar color="primary" size="56" variant="tonal" class="mb-2">
        <v-icon size="32" color="primary">mdi-check-decagram-outline</v-icon>
      </v-avatar>
      <div class="text-h6 font-weight-bold">Revisão das Configurações</div>
      <div class="text-body-2">Confira o resumo das ações que serão aplicadas no servidor.</div>
    </div>

    <v-sheet border rounded="lg" class="pa-4 mb-4 bg-surface">
      <v-list density="compact" class="pa-0 bg-transparent">
        <template v-for="(row, index) in rows" :key="row.title">
          <v-divider v-if="index > 0" class="my-2"></v-divider>
          <v-list-item class="px-0" :title="row.title" :subtitle="row.subtitle">
            <template #prepend>
              <v-avatar :color="row.color" size="32" variant="tonal" class="mr-3">
                <v-icon size="18">{{ row.icon }}</v-icon>
              </v-avatar>
            </template>
          </v-list-item>
        </template>
      </v-list>
    </v-sheet>

    <v-alert
      v-if="error"
      type="error"
      variant="tonal"
      density="compact"
      class="mb-3"
      closable
      @click:close="emit('clear-error')"
    >
      {{ error }}
    </v-alert>

    <div v-if="applying" class="pa-4 text-center">
      <v-progress-circular indeterminate color="primary" class="mb-2"></v-progress-circular>
      <div class="text-caption font-weight-bold">{{ statusMessage }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { SetupForm } from './setupForm'
import type { SetupStepId } from './steps'

interface ReviewRow {
  title: string
  subtitle: string
  icon: string
  color: string
}

const props = defineProps<{
  form: SetupForm
  /** As etapas percorridas: só elas entram no resumo (e no que será gravado). */
  steps: ReadonlySet<SetupStepId>
  /** O endereço no túnel só aparece com a área VPN ligada. */
  showVpnAddress: boolean
  /** Resumo dos aplicativos; `null` quando não há aplicativo para escolher. */
  appsSummary: string | null
  applying: boolean
  statusMessage: string
  error: string | null
}>()

const emit = defineEmits<{ 'clear-error': [] }>()

function addressesSummary(): string {
  const { addresses } = props.form
  const parts = [`LAN: ${addresses.lan || '—'}`, `Internet: ${addresses.public || '—'}`]
  if (props.showVpnAddress) parts.push(`VPN: ${addresses.vpn || '—'}`)
  const domain = addresses.domain.trim()
  if (domain) parts.push(`Domínio: ${domain}`)
  return parts.join(' | ')
}

const rows = computed<ReviewRow[]>(() => {
  const { form, steps } = props
  const list: ReviewRow[] = []
  if (steps.has('site')) {
    list.push({
      title: 'Local / Site',
      subtitle: form.site.enabled
        ? `${form.site.name} (${form.site.location || 'Sem local'})`
        : 'Não configurado',
      icon: 'mdi-domain',
      color: 'info',
    })
  }
  list.push({
    title: 'Endereços do Servidor',
    subtitle: addressesSummary(),
    icon: 'mdi-server-network',
    color: 'primary',
  })
  if (steps.has('network')) {
    list.push({
      title: 'Sub-rede',
      subtitle: form.network.enabled
        ? `${form.network.name} (${form.network.cidr}) — ${form.network.scanNow ? 'Varredura imediata ativa' : 'Sem varredura imediata'}`
        : 'Não configurada',
      icon: 'mdi-lan',
      color: 'teal',
    })
  }
  if (steps.has('dns')) {
    list.push({
      title: 'Servidores DNS',
      subtitle: `${form.dns.length} servidor(es) selecionado(s): ${form.dns.map((d) => d.name).join(', ') || 'Nenhum'}`,
      icon: 'mdi-dns-outline',
      color: 'deep-purple',
    })
  }
  if (steps.has('vpn')) {
    list.push({
      title: 'WireGuard VPN',
      subtitle: form.vpn.enabled
        ? `Porta ${form.vpn.listenPort} | CIDR ${form.vpn.cidr} | Endpoint ${form.vpn.publicEndpoint || 'Auto'}`
        : 'Desativado',
      icon: 'mdi-shield-lock-outline',
      color: 'deep-orange',
    })
  }
  if (steps.has('agents')) {
    list.push({
      title: 'Servidores e Docker',
      subtitle: 'Agente local e servidores conectados pelo comando de instalação',
      icon: 'mdi-server',
      color: 'info',
    })
  }
  if (steps.has('general')) {
    list.push({
      title: 'Preferências Globais',
      subtitle: `Ping: ${form.preferences.defaultPingIntervalSeconds}s | SNMP: ${form.preferences.defaultSnmpCommunity} | Descoberta: ${form.preferences.autoDiscoveryEnabled ? 'Ligada' : 'Desligada'}`,
      icon: 'mdi-cog-outline',
      color: 'warning',
    })
  }
  if (steps.has('apps') && props.appsSummary !== null) {
    list.push({
      title: 'Aplicativos',
      subtitle: props.appsSummary,
      icon: 'mdi-apps',
      color: 'primary',
    })
  }
  return list
})
</script>

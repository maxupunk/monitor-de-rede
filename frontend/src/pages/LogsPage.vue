<template>
  <div>
    <PageHeader
      title="Logs dos Dispositivos"
      subtitle="Mensagens de syslog recebidas dos roteadores e vinculadas ao inventário"
    >
      <template #actions>
        <v-btn
          :color="logsStore.tailing ? 'success' : 'primary'"
          :variant="logsStore.tailing ? 'flat' : 'tonal'"
          class="mr-2"
          @click="logsStore.toggleTail()"
        >
          <v-icon start>
            {{ logsStore.tailing ? 'mdi-radiobox-marked' : 'mdi-play-circle-outline' }}
          </v-icon>
          <span class="hidden-xs">{{ logsStore.tailing ? 'Ao vivo' : 'Acompanhar' }}</span>
        </v-btn>
        <v-btn color="primary" variant="tonal" @click="openSetup">
          <v-icon start>mdi-cog-outline</v-icon>
          <span class="hidden-xs">Configurar envio</span>
        </v-btn>
      </template>
    </PageHeader>

    <!--
      O aviso do NAT vem antes do de origem desconhecida, e não depois: quando
      o Docker mascara a origem, "vincule cada endereço" é conselho errado — o
      endereço é um só para o parque inteiro. Explicar a causa primeiro evita o
      operador seguir a instrução que contamina todos os dispositivos.
    -->
    <v-alert v-if="logsStore.natMasking" type="warning" variant="tonal" class="mb-4" border="start">
      <div class="d-flex align-center flex-wrap ga-2">
        <div class="flex-grow-1">
          <strong>O Docker está reescrevendo o endereço de origem das mensagens.</strong>
          Os equipamentos chegam como
          {{ (logsStore.nat?.gateways ?? []).join(', ') || 'o gateway da bridge' }}, e não pelo
          próprio IP. O vínculo passa a ser pelo nome que cada um envia no syslog; quem não envia
          nome não pode ser separado dos demais. Para receber o endereço real, publique o servidor
          com <code>network_mode: host</code> no <code>docker-compose.yml</code>.
        </div>
        <v-btn color="warning" variant="flat" size="small" @click="openSources">
          Ver origens
        </v-btn>
      </div>
    </v-alert>

    <v-alert
      v-if="logsStore.unknownCount > 0"
      type="warning"
      variant="tonal"
      class="mb-4"
      border="start"
    >
      <div class="d-flex align-center flex-wrap ga-2">
        <div class="flex-grow-1">
          <strong>
            {{ logsStore.unknownCount }}
            {{ logsStore.unknownCount === 1 ? 'origem está enviando' : 'origens estão enviando' }}
            log e {{ logsStore.unknownCount === 1 ? 'não é reconhecida' : 'não são reconhecidas' }}.
          </strong>
          Essas mensagens estão sendo descartadas para não encher o disco. Vincule cada origem a um
          dispositivo para começar a guardá-las.
        </div>
        <v-btn color="warning" variant="flat" size="small" @click="openSources">
          Ver origens
        </v-btn>
      </div>
    </v-alert>

    <v-card elevation="2" class="rounded-lg mb-6 pa-4">
      <v-row density="compact">
        <v-col cols="12" md="4">
          <v-text-field
            v-model="search"
            placeholder="Buscar no texto da mensagem..."
            prepend-inner-icon="mdi-magnify"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            @keyup.enter="applySearch"
            @click:clear="applySearch"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6" md="3">
          <v-select
            v-model="deviceId"
            :items="deviceOptions"
            item-title="title"
            item-value="value"
            label="Dispositivo"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ deviceId })"
          ></v-select>
        </v-col>
        <v-col cols="12" sm="6" md="3">
          <v-select
            v-model="severity"
            :items="severityOptions"
            item-title="label"
            item-value="value"
            label="Severidade"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ severity })"
          ></v-select>
        </v-col>
        <v-col cols="12" sm="6" md="2">
          <v-select
            v-model="hours"
            :items="windowOptions"
            item-title="label"
            item-value="value"
            label="Período"
            hide-details
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ hours })"
          ></v-select>
        </v-col>
      </v-row>
      <div v-if="windowLabel" class="text-caption text-grey mt-2">
        Mostrando registros {{ windowLabel }}.
      </div>
    </v-card>

    <LogTable
      :entries="logsStore.entries"
      :scroll-key="logsStore.scrollKey"
      :load="logsStore.load"
      :error="logsStore.error"
    />

    <LogSourcesDialog v-model="sourcesDialog" />
    <SyslogSetupDialog v-model="setupDialog" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import LogTable from '@/components/logs/LogTable.vue'
import LogSourcesDialog from '@/components/logs/LogSourcesDialog.vue'
import SyslogSetupDialog from '@/components/logs/SyslogSetupDialog.vue'
import { useLogsStore, SEVERITY_OPTIONS, WINDOW_OPTIONS, type LogFilters } from '@/stores/logs'
import { useDevicesStore } from '@/stores/devices'

const logsStore = useLogsStore()
const devicesStore = useDevicesStore()

const search = ref('')
const deviceId = ref<number | null>(null)
const severity = ref<number | null>(null)
const hours = ref<number | null>(24)
const sourcesDialog = ref(false)
const setupDialog = ref(false)

const severityOptions = SEVERITY_OPTIONS
const windowOptions = WINDOW_OPTIONS

const deviceOptions = computed(() =>
  devicesStore.devices.map((device) => ({ title: device.name, value: device.id }))
)

/**
 * A janela devolvida pelo backend, não a pedida: quem escolhe 7 dias e o
 * servidor guarda 5 precisa ver 5, senão conclui que o log sumiu.
 */
const windowLabel = computed(() => {
  const janela = logsStore.window
  if (!janela) return ''
  const inicio = new Date(janela.from)
  return `desde ${inicio.toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'short' })}`
})

/**
 * Mudar o filtro reinicia o tail junto com a lista: o stream carrega os
 * mesmos filtros, e mantê-lo com os antigos empilharia no topo linhas que a
 * tabela abaixo não mostraria.
 */
function onFilterChange(next: Partial<LogFilters>): void {
  const estavaAoVivo = logsStore.tailing
  if (estavaAoVivo) logsStore.stopTail()
  logsStore.applyFilters(next)
  if (estavaAoVivo) logsStore.startTail()
}

function applySearch(): void {
  onFilterChange({ search: search.value ?? '' })
}

function openSources(): void {
  sourcesDialog.value = true
}

function openSetup(): void {
  setupDialog.value = true
}

onMounted(() => {
  void devicesStore.fetchDevices()
  void logsStore.fetchSources()
})

// O `EventSource` sobrevive à troca de rota se ninguém o fechar — e ficaria
// empilhando linhas numa lista que não está mais na tela.
onUnmounted(() => logsStore.stopTail())
</script>

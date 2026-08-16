<template>
  <div>
    <PageHeader
      title="Logs dos Dispositivos"
      subtitle="Mensagens de syslog recebidas dos roteadores e vinculadas ao inventário"
    >
      <template #actions>
        <v-chip v-if="logsStore.window" color="primary" size="large" variant="tonal">
          <v-icon start>mdi-clock-outline</v-icon>
          <span class="hidden-xs">{{ windowLabel }}</span>
        </v-chip>
      </template>
    </PageHeader>

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
            @update:model-value="logsStore.applyFilters({ deviceId })"
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
            @update:model-value="logsStore.applyFilters({ severity })"
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
            @update:model-value="logsStore.applyFilters({ hours })"
          ></v-select>
        </v-col>
      </v-row>
    </v-card>

    <v-alert v-if="logsStore.error" type="error" variant="tonal" class="mb-4" border="start">
      {{ logsStore.error }}
    </v-alert>

    <v-infinite-scroll :key="logsStore.scrollKey" @load="logsStore.load">
      <v-table density="compact" class="rounded-lg border">
        <thead>
          <tr>
            <th class="text-left" style="width: 170px">Recebido</th>
            <th class="text-left" style="width: 110px">Severidade</th>
            <th class="text-left" style="width: 200px">Origem</th>
            <th class="text-left">Mensagem</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="entry in logsStore.entries" :key="entry.id">
            <td class="text-caption text-no-wrap">{{ formatReceivedAt(entry.receivedAt) }}</td>
            <td>
              <v-chip
                :color="severityColor(entry.severity)"
                size="x-small"
                variant="tonal"
                class="text-capitalize"
              >
                {{ entry.severityLabel ?? 'sem nível' }}
              </v-chip>
            </td>
            <td class="text-caption">
              <RouterLink
                v-if="entry.deviceId"
                :to="{ name: 'device-detail', params: { id: entry.deviceId } }"
                class="text-primary font-weight-medium text-decoration-none"
              >
                {{ entry.deviceName ?? `Dispositivo ${entry.deviceId}` }}
              </RouterLink>
              <span v-else class="text-grey">{{ entry.hostname ?? entry.sourceIp }}</span>
              <div class="text-grey text-caption">{{ entry.sourceIp }}</div>
            </td>
            <td class="text-body-2">
              <span class="log-message">{{ entry.message }}</span>
              <div v-if="entry.topics.length > 0" class="mt-1">
                <v-chip
                  v-for="topic in entry.topics"
                  :key="topic"
                  size="x-small"
                  variant="outlined"
                  class="mr-1"
                >
                  {{ topic }}
                </v-chip>
              </div>
              <div v-else-if="entry.appName" class="text-caption text-grey mt-1">
                {{ entry.appName }}<template v-if="entry.pid">[{{ entry.pid }}]</template>
              </div>
            </td>
          </tr>
        </tbody>
      </v-table>
      <template #empty>
        <div class="text-caption text-grey text-center py-4">
          Não há mais registros no período consultado.
        </div>
      </template>
    </v-infinite-scroll>

    <div v-if="logsStore.isEmpty" class="pa-8 text-center text-grey">
      <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-text-box-search-outline</v-icon>
      <div class="text-subtitle-2 font-weight-medium">Nenhum registro encontrado</div>
      <div class="text-caption">
        Ajuste os filtros ou verifique se os roteadores estão enviando syslog para este servidor.
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import PageHeader from '@/components/PageHeader.vue'
import { useLogsStore, severityColor, SEVERITY_OPTIONS, WINDOW_OPTIONS } from '@/stores/logs'
import { useDevicesStore } from '@/stores/devices'

const logsStore = useLogsStore()
const devicesStore = useDevicesStore()

const search = ref('')
const deviceId = ref<number | null>(null)
const severity = ref<number | null>(null)
const hours = ref<number | null>(24)

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

function applySearch(): void {
  logsStore.applyFilters({ search: search.value ?? '' })
}

function formatReceivedAt(value: string): string {
  return new Date(value).toLocaleString('pt-BR', {
    dateStyle: 'short',
    timeStyle: 'medium',
  })
}

onMounted(() => {
  void devicesStore.fetchDevices()
})
</script>

<style scoped>
/* Mensagem de log é texto pré-formatado: quebrar palavra longa é melhor do que
   estourar a largura da tabela com uma linha de firewall. */
.log-message {
  font-family: 'Roboto Mono', 'Courier New', monospace;
  font-size: 0.8125rem;
  word-break: break-word;
}
</style>

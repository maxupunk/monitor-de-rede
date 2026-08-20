<template>
  <v-card elevation="2" class="rounded-lg pa-4 pa-md-6 mb-6">
    <div
      class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between ga-4"
    >
      <div class="d-flex align-center ga-3">
        <v-avatar :color="headerChip.color" size="48" size-md="56" class="text-white mr-2">
          <v-icon size="28" size-md="32">{{ typeIcon }}</v-icon>
        </v-avatar>
        <div>
          <div class="d-flex align-center ga-2 flex-wrap">
            <h1 class="text-h6 text-md-h4 font-weight-bold">{{ monitor.name }}</h1>
            <v-chip
              :color="headerChip.color"
              size="small"
              variant="flat"
              class="font-weight-bold px-3"
            >
              <v-icon start size="14">{{ headerChip.icon }}</v-icon>
              {{ headerChip.label }}
            </v-chip>
            <v-chip size="small" color="info" variant="tonal" class="px-3">
              {{ typeText }}
            </v-chip>
          </div>
          <div class="text-caption text-md-subtitle-1 text-grey-darken-1 mt-1 text-break">
            Alvo: <strong class="text-high-emphasis">{{ formattedTarget }}</strong> ·
            {{
              monitor.type === 'snmp' && monitor.device
                ? 'Intervalo de coleta SNMP:'
                : 'Intervalo de coleta:'
            }}
            {{ monitor.intervalSeconds }}s
            <span v-if="monitor.device">
              · Dispositivo: <strong>{{ monitor.device.name }}</strong></span
            >
          </div>
        </div>
      </div>

      <div class="d-flex flex-wrap align-center ga-2 w-100 w-md-auto">
        <v-btn
          v-if="monitor.device"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-router-network"
          size="small"
          class="flex-grow-1 flex-md-grow-0"
          :to="{ name: 'device-detail', params: { id: monitor.device.id } }"
        >
          <span class="hidden-sm-and-down">Ver dispositivo</span>
          <span class="hidden-md-and-up">Dispositivo</span>
        </v-btn>
        <v-btn
          color="primary"
          prepend-icon="mdi-play"
          size="small"
          class="flex-grow-1 flex-md-grow-0"
          :loading="running"
          @click="emit('test')"
        >
          <span class="hidden-sm-and-down">Testar Agora</span>
          <span class="hidden-md-and-up">Testar</span>
        </v-btn>
        <v-btn
          variant="outlined"
          color="primary"
          prepend-icon="mdi-pencil"
          size="small"
          class="flex-grow-1 flex-md-grow-0"
          @click="emit('edit')"
        >
          Editar
        </v-btn>
        <v-btn
          :color="monitor.isEnabled ? 'warning' : 'success'"
          variant="outlined"
          size="small"
          class="flex-grow-1 flex-md-grow-0"
          :prepend-icon="monitor.isEnabled ? 'mdi-pause' : 'mdi-play-outline'"
          @click="emit('toggleEnabled')"
        >
          {{ monitor.isEnabled ? 'Pausar' : 'Ativar' }}
        </v-btn>
        <v-btn icon color="error" variant="text" size="small" @click="emit('delete')">
          <v-icon>mdi-delete</v-icon>
          <v-tooltip activator="parent" location="top">Excluir Monitor</v-tooltip>
        </v-btn>
      </div>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import type { Monitor } from '@/stores/monitors'

defineProps<{
  monitor: Monitor
  headerChip: { label: string; color: string; icon: string }
  typeIcon: string
  typeText: string
  formattedTarget: string
  running: boolean
}>()

const emit = defineEmits<{
  (e: 'test'): void
  (e: 'edit'): void
  (e: 'toggleEnabled'): void
  (e: 'delete'): void
}>()
</script>

<template>
  <div>
    <div v-if="available.length === 0" class="text-body-2 mb-4">
      Nenhum plugin compatível ainda. Crie um com a IA ou importe um pacote.
    </div>
    <v-row dense>
      <v-col v-for="item in available" :key="item.plugin.id" cols="12" md="6">
        <v-card border flat class="rounded-lg h-100 d-flex flex-column">
          <v-card-item>
            <template #prepend>
              <v-avatar color="primary" variant="tonal" rounded="lg">
                <v-icon>{{ item.plugin.panel?.icon ?? 'mdi-puzzle-outline' }}</v-icon>
              </v-avatar>
            </template>
            <v-card-title class="text-wrap">{{ item.plugin.name }}</v-card-title>
            <v-card-subtitle>v{{ item.plugin.version }} · {{ transports(item) }}</v-card-subtitle>
          </v-card-item>
          <v-card-text class="flex-grow-1">
            <p v-if="item.plugin.description" class="text-body-2 mb-2">
              {{ item.plugin.description }}
            </p>
            <div class="d-flex flex-wrap ga-1 mb-2">
              <v-chip size="x-small" :color="compatPresentation(item.compat).color" variant="flat">
                <v-icon start size="12">{{ compatPresentation(item.compat).icon }}</v-icon>
                {{ compatPresentation(item.compat).label }}
              </v-chip>
              <v-chip
                size="x-small"
                :color="statusPresentation(item.plugin.status).color"
                variant="tonal"
              >
                {{ statusPresentation(item.plugin.status).label }}
              </v-chip>
              <v-chip
                size="x-small"
                :color="sourcePresentation(item.plugin.source).color"
                variant="tonal"
              >
                {{ sourcePresentation(item.plugin.source).label }}
              </v-chip>
              <v-chip v-if="item.plugin.deviceId" size="x-small" color="secondary" variant="tonal">
                Exclusivo
              </v-chip>
            </div>
            <div class="text-body-small">{{ item.reasons.join(' · ') }}</div>
          </v-card-text>
          <v-card-actions class="flex-wrap ga-1">
            <v-btn
              v-if="item.plugin.status === 'quarantine'"
              size="small"
              color="error"
              variant="flat"
              prepend-icon="mdi-shield-alert-outline"
              @click="emit('review', item.plugin.id)"
            >
              Ver revisão de segurança
            </v-btn>
            <v-btn
              v-else-if="item.installed"
              size="small"
              color="success"
              variant="flat"
              prepend-icon="mdi-open-in-app"
              @click="emit('open', item.plugin.id)"
            >
              Instalado — usar
            </v-btn>
            <v-btn
              v-else
              size="small"
              color="primary"
              variant="flat"
              prepend-icon="mdi-puzzle-plus-outline"
              :loading="installing === item.plugin.id"
              @click="emit('install', item.plugin.id)"
            >
              {{ item.plugin.status === 'disabled' ? 'Ativar e instalar' : 'Instalar' }}
            </v-btn>
            <v-spacer></v-spacer>
            <v-btn
              size="small"
              color="primary"
              variant="text"
              prepend-icon="mdi-code-braces"
              @click="emit('edit', item.plugin.id)"
            >
              {{ item.plugin.source === 'builtin' ? 'Ver código' : 'Editar' }}
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-col>
    </v-row>

    <v-expansion-panels v-if="incompatible.length" class="mt-4">
      <v-expansion-panel>
        <v-expansion-panel-title>
          <v-icon color="error" class="mr-2">mdi-close-circle-outline</v-icon>
          Incompatíveis com este equipamento ({{ incompatible.length }})
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <v-list density="compact">
            <v-list-item
              v-for="item in incompatible"
              :key="item.plugin.id"
              :title="`${item.plugin.name} v${item.plugin.version}`"
              :subtitle="item.reasons.join(' · ')"
            ></v-list-item>
          </v-list>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { DevicePluginItem } from '@/bindings/DevicePluginItem'
import {
  compatPresentation,
  sourcePresentation,
  statusPresentation,
} from '@/utils/pluginPresentation'

const props = defineProps<{
  plugins: DevicePluginItem[]
  installing: number | null
}>()

const emit = defineEmits<{
  install: [pluginId: number]
  open: [pluginId: number]
  review: [pluginId: number]
  edit: [pluginId: number]
}>()

const available = computed(() => props.plugins.filter((item) => item.compat !== 'incompatible'))
const incompatible = computed(() => props.plugins.filter((item) => item.compat === 'incompatible'))

function transports(item: DevicePluginItem): string {
  return item.plugin.transports.map((transport) => transport.toUpperCase()).join(', ')
}
</script>

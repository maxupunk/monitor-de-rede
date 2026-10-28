<template>
  <v-card border flat class="rounded-lg h-100 d-flex flex-column">
    <v-card-item>
      <template #prepend>
        <v-avatar :color="status.color" variant="tonal" rounded="lg">
          <v-icon>{{ pluginIcon(plugin) }}</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-wrap text-body-1 font-weight-bold">{{ plugin.name }}</v-card-title>
      <v-card-subtitle class="opacity-100">
        v{{ plugin.version }} · {{ transports }} · {{ source.label }}
      </v-card-subtitle>
      <template #append>
        <v-chip size="small" :color="status.color" variant="flat" :prepend-icon="status.icon">
          {{ status.label }}
        </v-chip>
      </template>
    </v-card-item>

    <v-card-text class="flex-grow-1 pt-0">
      <p v-if="plugin.description" class="text-body-2 description mb-3" :title="plugin.description">
        {{ plugin.description }}
      </p>
      <div v-if="hasBadges" class="d-flex flex-wrap ga-1">
        <v-chip v-if="plugin.deviceId" size="small" color="secondary" variant="tonal">
          Exclusivo de um equipamento
        </v-chip>
        <v-chip
          v-if="riskWorthShowing"
          size="small"
          :color="risk.color"
          variant="tonal"
          :prepend-icon="risk.icon"
        >
          Risco {{ risk.label.toLowerCase() }}
        </v-chip>
        <v-chip
          v-if="plugin.validatedCount > 0"
          size="small"
          color="success"
          variant="tonal"
          prepend-icon="mdi-check-decagram"
        >
          Validado em {{ plugin.validatedCount }}
        </v-chip>
      </div>
      <v-alert
        v-if="nextStep"
        :color="status.color"
        variant="tonal"
        density="compact"
        class="mt-3 text-body-2"
        :icon="status.icon"
      >
        {{ nextStep }}
      </v-alert>
    </v-card-text>

    <v-card-actions class="flex-wrap ga-1 px-4 pb-3">
      <v-btn
        v-if="plugin.status === 'quarantine'"
        color="error"
        variant="flat"
        prepend-icon="mdi-shield-search"
        @click="emit('review', plugin)"
      >
        Revisar segurança
      </v-btn>
      <v-btn
        v-else-if="plugin.status === 'tested'"
        color="success"
        variant="flat"
        prepend-icon="mdi-power-plug"
        @click="emit('lifecycle', plugin, 'promote')"
      >
        Ativar
      </v-btn>
      <v-btn
        v-else-if="plugin.status === 'draft'"
        color="warning"
        variant="flat"
        prepend-icon="mdi-test-tube"
        @click="emit('edit', plugin)"
      >
        Abrir e testar
      </v-btn>
      <v-btn
        v-else-if="plugin.status === 'disabled'"
        color="success"
        variant="tonal"
        prepend-icon="mdi-power-plug-outline"
        @click="emit('lifecycle', plugin, 'enable')"
      >
        Reativar
      </v-btn>
      <v-btn
        v-if="opensApp(plugin)"
        color="primary"
        variant="flat"
        :prepend-icon="plugin.fleet?.icon ?? 'mdi-apps'"
        :to="'/apps/' + plugin.id"
      >
        Abrir aplicativo
      </v-btn>
      <v-btn
        v-if="plugin.status !== 'draft'"
        color="primary"
        variant="text"
        prepend-icon="mdi-code-braces"
        @click="emit('edit', plugin)"
      >
        {{ plugin.source === 'builtin' ? 'Ver código' : 'Editar' }}
      </v-btn>
      <v-spacer></v-spacer>
      <v-menu location="bottom end">
        <template #activator="{ props: menu }">
          <v-btn
            v-bind="menu"
            icon="mdi-dots-vertical"
            variant="text"
            color="primary"
            size="small"
            :aria-label="`Mais ações de ${plugin.name}`"
          ></v-btn>
        </template>
        <v-list density="compact">
          <v-list-item
            prepend-icon="mdi-download-outline"
            title="Exportar pacote"
            @click="emit('export', plugin)"
          ></v-list-item>
          <v-list-item
            v-if="plugin.status !== 'disabled'"
            prepend-icon="mdi-power-plug-off-outline"
            title="Desativar"
            base-color="warning"
            @click="emit('lifecycle', plugin, 'disable')"
          ></v-list-item>
          <v-list-item
            v-if="plugin.source !== 'builtin'"
            prepend-icon="mdi-delete-outline"
            title="Excluir"
            base-color="error"
            @click="emit('remove', plugin)"
          ></v-list-item>
        </v-list>
      </v-menu>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { PluginSummary } from '@/bindings/PluginSummary'
import {
  nextStepOf,
  opensApp,
  pluginIcon,
  severityPresentation,
  sourcePresentation,
  statusPresentation,
} from '@/utils/pluginPresentation'

const props = defineProps<{
  plugin: PluginSummary
}>()

const emit = defineEmits<{
  edit: [plugin: PluginSummary]
  review: [plugin: PluginSummary]
  export: [plugin: PluginSummary]
  remove: [plugin: PluginSummary]
  lifecycle: [plugin: PluginSummary, action: 'promote' | 'enable' | 'disable']
}>()

const status = computed(() => statusPresentation(props.plugin.status))
const source = computed(() => sourcePresentation(props.plugin.source))
const nextStep = computed(() => nextStepOf(props.plugin.status))
const transports = computed(() =>
  props.plugin.transports.map((transport) => transport.toUpperCase()).join(', ')
)
const risk = computed(() => severityPresentation(props.plugin.risk ?? 'info'))
/** Risco baixo ou informativo não muda a decisão de ninguém: só o que pede atenção. */
const riskWorthShowing = computed(() =>
  ['medium', 'high', 'critical'].includes(props.plugin.risk ?? '')
)
const hasBadges = computed(
  () => props.plugin.deviceId !== null || riskWorthShowing.value || props.plugin.validatedCount > 0
)
</script>

<style scoped>
.description {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>

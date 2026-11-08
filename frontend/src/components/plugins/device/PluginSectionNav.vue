<template>
  <!-- Computador: lista lateral, com os instalados separados do gerenciamento -->
  <v-list
    v-if="mdAndUp"
    :selected="[current]"
    mandatory
    nav
    density="compact"
    color="primary"
    class="plugin-nav rounded-lg border pa-2"
    aria-label="Seções de plugins"
    @update:selected="select"
  >
    <v-list-subheader class="font-weight-bold text-primary">
      Instalados neste equipamento
    </v-list-subheader>
    <v-list-item
      v-for="plugin in installed"
      :key="plugin.id"
      :value="pluginTab(plugin.id)"
      :prepend-icon="plugin.icon"
      :title="plugin.title"
      :base-color="PLUGIN_COLOR"
      :color="PLUGIN_COLOR"
      rounded="lg"
    >
      <template v-if="blockedStatus(plugin)" #append>
        <v-chip size="x-small" :color="blockedStatus(plugin)?.color" variant="flat">
          {{ blockedStatus(plugin)?.label }}
        </v-chip>
      </template>
    </v-list-item>
    <div v-if="!installed.length" class="text-body-2 px-4 pb-2">
      Nenhum ainda. Instale um pelo catálogo.
    </div>

    <v-divider class="my-2" />
    <v-list-subheader class="font-weight-bold text-primary">Gerenciar</v-list-subheader>
    <v-list-item
      v-for="section in manageSections"
      :key="section.value"
      :value="section.value"
      :prepend-icon="section.icon"
      :title="section.title"
      rounded="lg"
    />
  </v-list>

  <!-- Celular: fileira de chips que rola de lado -->
  <v-chip-group
    v-else
    :model-value="current"
    mandatory
    selected-class="plugin-nav__chip--active"
    color="primary"
    class="mb-2"
    aria-label="Seções de plugins"
    @update:model-value="select"
  >
    <v-chip
      v-for="plugin in installed"
      :key="plugin.id"
      :value="pluginTab(plugin.id)"
      :prepend-icon="plugin.icon"
      :color="blockedStatus(plugin)?.color ?? PLUGIN_COLOR"
      :base-color="blockedStatus(plugin)?.color ?? PLUGIN_COLOR"
      variant="outlined"
      filter
    >
      {{ plugin.title }}
      <template v-if="blockedStatus(plugin)"> · {{ blockedStatus(plugin)?.label }}</template>
    </v-chip>
    <v-divider v-if="installed.length" vertical class="mx-1 my-2" />
    <v-chip
      v-for="section in manageSections"
      :key="section.value"
      :value="section.value"
      :prepend-icon="section.icon"
      color="secondary"
      base-color="secondary"
      variant="outlined"
      filter
    >
      {{ section.title }}
    </v-chip>
  </v-chip-group>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDisplay } from 'vuetify'
import {
  PLUGIN_COLOR,
  PLUGIN_MANAGE_SECTIONS,
  pluginTab,
  statusPresentation,
  type InstalledPluginTab,
  type Presentation,
} from '@/utils/pluginPresentation'

/**
 * Navegação da aba Plugins: o que já está instalado (o que se usa) separado do
 * que se gerencia (catálogo, credenciais, histórico). Lista ao lado no
 * computador, chips no celular.
 */
const props = defineProps<{
  current: string
  installed: InstalledPluginTab[]
  credentialsCount: number
}>()

const emit = defineEmits<{ select: [section: string] }>()

const { mdAndUp } = useDisplay()

const manageSections = computed(() =>
  PLUGIN_MANAGE_SECTIONS.map((section) =>
    section.value === 'credentials'
      ? { ...section, title: `${section.title} (${props.credentialsCount})` }
      : section
  )
)

/** Selo do instalado que não roda (desativado ou em quarentena); `null` quando roda. */
function blockedStatus(plugin: InstalledPluginTab): Presentation | null {
  const status = plugin.item.plugin.status
  return status === 'disabled' || status === 'quarantine' ? statusPresentation(status) : null
}

function select(value: unknown) {
  const section = Array.isArray(value) ? value[0] : value
  if (typeof section === 'string' && section !== props.current) emit('select', section)
}
</script>

<style scoped>
.plugin-nav {
  position: sticky;
  top: 72px;
}

/* Nome de plugin inteiro, quebrando linha, em vez de cortado com "…". */
.plugin-nav :deep(.v-list-item-title) {
  white-space: normal;
  line-height: 1.3;
}

.plugin-nav__chip--active {
  font-weight: 700;
}
</style>

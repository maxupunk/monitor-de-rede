<template>
  <section class="topology-inspector" :class="{ sheet }" aria-label="Detalhes do item selecionado">
    <header class="inspector-header px-4 pt-3 pb-2">
      <div class="d-flex align-center ga-3">
        <v-avatar :color="header.color" size="42" rounded="lg">
          <v-icon color="white" size="24">{{ header.icon }}</v-icon>
        </v-avatar>
        <div class="flex-grow-1 overflow-hidden">
          <div class="text-subtitle-1 font-weight-bold text-truncate" :title="header.title">
            {{ header.title }}
          </div>
          <div class="d-flex align-center flex-wrap ga-1 mt-1">
            <v-chip :color="header.color" size="x-small" variant="flat" label>
              {{ header.status }}
            </v-chip>
            <span class="text-caption subtitle text-truncate">{{ header.subtitle }}</span>
          </div>
        </div>
        <v-btn
          icon="mdi-close"
          variant="text"
          size="small"
          aria-label="Fechar painel (Esc)"
          @click="$emit('close')"
        ></v-btn>
      </div>
    </header>

    <v-tabs v-model="tab" color="primary" density="compact" show-arrows class="px-2">
      <v-tab v-for="item in tabs" :key="item.id" :value="item.id" :prepend-icon="item.icon">
        {{ item.title }}
      </v-tab>
    </v-tabs>
    <v-divider></v-divider>

    <div class="inspector-body">
      <component :is="current.component" v-if="current" :target="target" :host="host" />
    </div>
  </section>
</template>

<script setup lang="ts">
/**
 * Painel lateral (folha inferior no celular) do nó ou enlace selecionado.
 * As abas vêm do registro (`inspectorRegistry`) — extensões entram sem
 * editar este componente.
 */
import { computed, ref } from 'vue'
import {
  resolveTabs,
  type TopologyHost,
  type TopologyTarget,
} from '@/utils/topology/inspectorRegistry'
import {
  EDGE_STATUS,
  edgeStatusKey,
  linkKindPresentation,
  nodeIcon,
  nodeIp,
  nodeStatusPresentation,
  nodeTypeLabel,
} from '@/utils/topology/presentation'
import { registerBuiltinInspectorTabs } from './inspector/builtinTabs'

const props = defineProps<{
  target: TopologyTarget
  host: TopologyHost
  /** Exibido como folha inferior (celular). */
  sheet?: boolean
}>()

defineEmits<{ close: [] }>()

registerBuiltinInspectorTabs()

const tabs = computed(() => resolveTabs(props.target))
/** Última aba escolhida por tipo de alvo (nó, enlace). */
const chosen = ref<Record<string, string>>({})

/**
 * Aba ativa derivada (nunca um id que não existe para o alvo atual): trocar
 * de nó para enlace cai na aba equivalente ou na primeira.
 */
const tab = computed<string>({
  get: () => {
    const wanted = chosen.value[props.target.kind]
    return tabs.value.some((item) => item.id === wanted) ? wanted : (tabs.value[0]?.id ?? '')
  },
  set: (value) => {
    if (!value || !tabs.value.some((item) => item.id === value)) return
    chosen.value = { ...chosen.value, [props.target.kind]: value }
  },
})

const current = computed(() => tabs.value.find((item) => item.id === tab.value) ?? null)

const header = computed(() => {
  const target = props.target
  if (target.kind === 'node') {
    const status = nodeStatusPresentation(target.node.status)
    return {
      title: target.node.name,
      subtitle: [nodeTypeLabel(target.node), nodeIp(target.node)].filter(Boolean).join(' · '),
      icon: nodeIcon(target.node),
      color: status.color,
      status: status.label,
    }
  }
  if (target.kind === 'edge') {
    const status = EDGE_STATUS[edgeStatusKey(target.edge)]
    const kind = linkKindPresentation(target.edge.linkType)
    return {
      title: `${target.edge.sourceDeviceName || 'Origem'} ↔ ${target.edge.targetDeviceName || 'Destino'}`,
      subtitle: kind.label,
      icon: kind.icon,
      color: status.color,
      status: status.label,
    }
  }
  return { title: 'Mapa', subtitle: '', icon: 'mdi-sitemap', color: 'primary', status: '' }
})
</script>

<style scoped>
.topology-inspector {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 384px;
  max-width: 100%;
  background: rgb(var(--v-theme-surface));
  border-left: 1px solid rgba(var(--v-theme-on-surface), 0.14);
  color: rgb(var(--v-theme-on-surface));
}
.topology-inspector.sheet {
  width: 100%;
  height: auto;
  max-height: 78vh;
  border-left: none;
  border-radius: 16px 16px 0 0;
}
.subtitle {
  color: rgb(var(--v-theme-on-surface));
}
.inspector-body {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
</style>

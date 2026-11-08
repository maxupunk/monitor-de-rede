<template>
  <div class="d-flex align-center justify-end ga-1" @click.stop>
    <v-chip
      v-if="container.protected"
      size="small"
      color="info"
      variant="tonal"
      prepend-icon="mdi-shield-lock"
    >
      Protegido
      <v-tooltip activator="parent" location="top" max-width="280">
        Container da instalação NetMonitor: parar, recriar ou remover derrubaria a central.
      </v-tooltip>
    </v-chip>

    <!-- A ação principal fica à vista: ligar ou desligar. -->
    <template v-if="primary">
      <v-btn
        v-if="labeled"
        :color="primary.color"
        variant="tonal"
        :prepend-icon="primary.icon"
        :loading="busy === primary.key"
        @click="trigger(primary.key)"
      >
        {{ primary.label }}
      </v-btn>
      <v-btn
        v-else
        :icon="primary.icon"
        :color="primary.color"
        variant="tonal"
        size="small"
        :aria-label="`${primary.label} ${name}`"
        :loading="busy === primary.key"
        @click="trigger(primary.key)"
      >
        <v-icon>{{ primary.icon }}</v-icon>
        <v-tooltip activator="parent" location="top">{{ primary.label }}</v-tooltip>
      </v-btn>
    </template>

    <!-- O resto, com nome e explicação, num menu. -->
    <v-menu v-if="others.length" location="bottom end">
      <template #activator="{ props: menuProps }">
        <v-btn
          v-bind="menuProps"
          :icon="!labeled"
          :size="labeled ? undefined : 'small'"
          color="primary"
          variant="tonal"
          :append-icon="labeled ? 'mdi-chevron-down' : undefined"
          :aria-label="`Mais ações de ${name}`"
          :loading="busy !== null && busy !== primary?.key"
        >
          <v-icon v-if="!labeled">mdi-dots-vertical</v-icon>
          <template v-else>Mais ações</template>
        </v-btn>
      </template>
      <v-list density="compact" min-width="260">
        <v-list-item
          v-for="action in others"
          :key="action.key"
          :title="action.label"
          :subtitle="action.hint"
          :base-color="action.color"
          lines="two"
          @click="trigger(action.key)"
        >
          <template #prepend>
            <v-icon :color="action.color">{{ action.icon }}</v-icon>
          </template>
        </v-list-item>
      </v-list>
    </v-menu>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import { useContainerActions, type ActionOutcome } from '@/composables/useContainerActions'
import { containerName, type ContainerActionKey } from '@/utils/dockerPresentation'

/**
 * Ações de um container: a principal à vista, as demais num menu com o que
 * cada uma faz. `labeled` desenha botões com texto (detalhe e celular).
 */
const props = defineProps<{
  container: DockerContainerSummary
  labeled?: boolean
}>()

const emit = defineEmits<{ done: [outcome: ActionOutcome] }>()

const actions = useContainerActions()
const busy = ref<ContainerActionKey | null>(null)

const name = computed(() => containerName(props.container))
const list = computed(() => actions.available(props.container))
const primary = computed(() => {
  const first = list.value[0]
  return first && (first.key === 'start' || first.key === 'stop') ? first : null
})
const others = computed(() => list.value.filter((action) => action !== primary.value))

async function trigger(key: ContainerActionKey) {
  busy.value = key
  try {
    const outcome = await actions.run(props.container, key)
    if (outcome) emit('done', outcome)
  } finally {
    busy.value = null
  }
}
</script>

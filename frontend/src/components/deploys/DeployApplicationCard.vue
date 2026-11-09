<template>
  <v-card elevation="2" class="h-100" data-test="application-card">
    <v-card-item>
      <template #prepend>
        <v-avatar color="primary" variant="tonal" rounded="lg">
          <v-icon>mdi-github</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="font-weight-bold text-wrap">{{ application.name }}</v-card-title>
      <v-card-subtitle class="text-high-emphasis">
        <a
          :href="`https://github.com/${application.repository}`"
          target="_blank"
          rel="noopener"
          class="text-primary"
          >{{ application.repository }}</a
        >
        · {{ application.rollout === 'sequential' ? 'um alvo de cada vez' : 'todos juntos' }}
        <template v-if="!application.credentialId"> · sem credencial</template>
      </v-card-subtitle>
      <template v-if="canAdmin" #append>
        <v-btn
          color="primary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-github"
          class="mr-1"
          @click="emit('workflow', application)"
        >
          Workflow
        </v-btn>
        <v-menu>
          <template #activator="{ props: menu }">
            <v-btn
              v-bind="menu"
              icon="mdi-dots-vertical"
              variant="text"
              color="primary"
              size="small"
              :aria-label="`Mais ações de ${application.name}`"
            ></v-btn>
          </template>
          <v-list density="compact">
            <v-list-item
              prepend-icon="mdi-pencil"
              title="Editar aplicação"
              @click="emit('edit', application)"
            ></v-list-item>
            <v-list-item
              prepend-icon="mdi-delete"
              title="Remover aplicação"
              base-color="error"
              @click="emit('remove', application)"
            ></v-list-item>
          </v-list>
        </v-menu>
      </template>
    </v-card-item>

    <v-card-text class="d-flex flex-column ga-2">
      <DeployTargetRow
        v-for="target in application.targets"
        :key="target.id"
        :target="target"
        :repository="application.repository"
        :can-admin="canAdmin"
        @open-run="(id) => emit('open-run', id)"
        @publish="(item) => emit('publish', item)"
        @env="(item) => emit('env', item)"
        @history="(item) => emit('history', item)"
        @edit="(item) => emit('edit-target', application, item)"
        @remove="(item) => emit('remove-target', item)"
      />
      <div v-if="!application.targets.length" class="text-body-2 text-high-emphasis">
        Nenhum alvo: diga em qual servidor esta aplicação roda.
      </div>
      <div v-if="canAdmin">
        <v-btn
          color="primary"
          variant="text"
          size="small"
          prepend-icon="mdi-plus"
          @click="emit('add-target', application)"
        >
          Adicionar alvo (outro servidor ou ambiente)
        </v-btn>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import DeployTargetRow from '@/components/deploys/DeployTargetRow.vue'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'

defineProps<{ application: DeployApplicationView; canAdmin: boolean }>()
const emit = defineEmits<{
  'open-run': [id: number]
  publish: [target: DeployTargetView]
  env: [target: DeployTargetView]
  history: [target: DeployTargetView]
  'edit-target': [application: DeployApplicationView, target: DeployTargetView]
  'remove-target': [target: DeployTargetView]
  'add-target': [application: DeployApplicationView]
  workflow: [application: DeployApplicationView]
  edit: [application: DeployApplicationView]
  remove: [application: DeployApplicationView]
}>()
</script>

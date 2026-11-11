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
        <template v-if="compiles"> · compila no NetMonitor</template>
        <template v-if="application.githubAccount"> · via GitHub App</template>
        <template v-else-if="!application.credentialId"> · sem acesso a privados</template>
      </v-card-subtitle>
      <template v-if="canAdmin" #append>
        <v-tooltip v-if="compiles" :text="noBuildHost ?? ''" :disabled="!noBuildHost">
          <template #activator="{ props: tooltip }">
            <span v-bind="tooltip" class="mr-1">
              <v-btn
                color="primary"
                variant="flat"
                size="small"
                prepend-icon="mdi-hammer-wrench"
                data-test="build-now"
                :disabled="noBuildHost !== null"
                @click="emit('build', application)"
              >
                Compilar agora
              </v-btn>
            </span>
          </template>
        </v-tooltip>
        <v-btn
          v-else
          color="primary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-github"
          class="mr-1"
          @click="emit('workflow', application)"
        >
          Ligar o GitHub
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
              v-if="compiles"
              prepend-icon="mdi-key-chain"
              title="Segredos de build"
              @click="emit('secrets', application)"
            ></v-list-item>
            <v-list-item
              v-if="compiles"
              prepend-icon="mdi-github"
              title="Workflow do GitHub Actions"
              subtitle="Só se for voltar a compilar no GitHub"
              @click="emit('workflow', application)"
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
      <ChoiceCard
        v-if="compiles && latestBuild"
        class="pa-2 d-flex align-center ga-2"
        :aria-label="`Abrir a compilação #${latestBuild.id}`"
        data-test="latest-build"
        @click="emit('open-build', latestBuild.id)"
      >
        <StatusMetaChip :meta="BUILD_STATUS[latestBuild.status]" />
        <span class="text-body-2">
          Última compilação: <code>{{ latestBuild.shortSha }}</code>
          <template v-if="latestBuild.reference"> em {{ refName(latestBuild.reference) }}</template>
          · {{ formatRelativeTime(latestBuild.createdAt) }}
        </span>
        <v-spacer></v-spacer>
        <v-icon color="primary">mdi-chevron-right</v-icon>
      </ChoiceCard>
      <v-alert
        v-else-if="compiles && application.targets.length"
        type="info"
        variant="tonal"
        density="compact"
        icon="mdi-hammer-wrench"
      >
        Nenhuma compilação ainda: o próximo push na branch do alvo compila e publica — ou use
        Compilar agora.
      </v-alert>
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
import { computed } from 'vue'
import ChoiceCard from '@/components/ChoiceCard.vue'
import DeployTargetRow from '@/components/deploys/DeployTargetRow.vue'
import StatusMetaChip from '@/components/deploys/StatusMetaChip.vue'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployServerBuildView } from '@/bindings/DeployServerBuildView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import { BUILD_STATUS, refName } from '@/utils/deployPresentation'
import { formatRelativeTime } from '@/utils/formatters'

const props = withDefaults(
  defineProps<{
    application: DeployApplicationView
    canAdmin: boolean
    latestBuild?: DeployServerBuildView | null
  }>(),
  { latestBuild: null }
)

/** Por que não dá para compilar agora (sem alvo e sem servidor escolhido). */
const noBuildHost = computed(() =>
  props.application.targets.length === 0 && props.application.buildProbeId === null
    ? 'Cadastre um alvo ou escolha onde compilar: não há servidor para compilar'
    : null
)

/** O servidor compila esta aplicação (ADR 023). */
const compiles = computed(() => props.application.builder === 'netmonitor')

const emit = defineEmits<{
  'open-run': [id: number]
  publish: [target: DeployTargetView]
  env: [target: DeployTargetView]
  history: [target: DeployTargetView]
  'edit-target': [application: DeployApplicationView, target: DeployTargetView]
  'remove-target': [target: DeployTargetView]
  'add-target': [application: DeployApplicationView]
  workflow: [application: DeployApplicationView]
  build: [application: DeployApplicationView]
  'open-build': [id: number]
  secrets: [application: DeployApplicationView]
  edit: [application: DeployApplicationView]
  remove: [application: DeployApplicationView]
}>()
</script>

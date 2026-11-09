<template>
  <div>
    <DeployGithubConnectForm v-if="connecting" :return-to="returnTo" @cancel="connecting = false" />

    <template v-else>
      <v-list
        v-if="deploys.githubConnections.length"
        density="compact"
        class="bg-transparent pa-0 mb-2"
      >
        <v-list-item
          v-for="connection in deploys.githubConnections"
          :key="connection.id"
          :title="connection.account"
          class="px-0"
        >
          <template #prepend>
            <v-icon color="primary" class="mr-3">
              {{ connection.organization ? 'mdi-domain' : 'mdi-github' }}
            </v-icon>
          </template>
          <template #subtitle>
            <div class="d-flex flex-wrap align-center ga-1 mt-1">
              <v-chip
                size="x-small"
                variant="flat"
                :color="connection.installed ? 'success' : 'warning'"
              >
                {{ connection.installed ? 'Instalado' : 'Falta instalar' }}
              </v-chip>
              <v-chip
                size="x-small"
                variant="flat"
                :color="connection.canWrite ? 'primary' : 'info'"
              >
                {{ connection.canWrite ? 'Abre o PR do workflow' : 'Só leitura' }}
              </v-chip>
            </div>
          </template>
          <template #append>
            <v-btn
              v-if="!connection.installed"
              color="warning"
              variant="flat"
              size="small"
              prepend-icon="mdi-download"
              @click="install(connection)"
            >
              Instalar
            </v-btn>
            <IconAction
              v-else
              icon="mdi-source-repository-multiple"
              label="Escolher os repositórios que o app enxerga"
              @click="install(connection)"
            />
            <IconAction
              icon="mdi-refresh"
              label="Conferir no GitHub (instalação e permissões)"
              :loading="refreshing === connection.id"
              @click="refresh(connection)"
            />
            <IconAction
              icon="mdi-open-in-new"
              label="Configurações do app no GitHub"
              @click="openSettings(connection)"
            />
            <IconAction
              icon="mdi-delete"
              label="Remover a conexão"
              color="error"
              @click="remove(connection)"
            />
          </template>
        </v-list-item>
      </v-list>
      <div v-else class="text-body-2 text-high-emphasis mb-3">
        Conecte o GitHub para escolher o repositório da lista, preencher as imagens a partir do
        compose e abrir o PR do workflow sem sair daqui.
      </div>

      <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">
        {{ error }}
      </v-alert>

      <v-btn color="primary" variant="flat" prepend-icon="mdi-github" @click="connecting = true">
        {{ deploys.githubConnections.length ? 'Conectar outra conta' : 'Conectar GitHub' }}
      </v-btn>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import IconAction from '@/components/IconAction.vue'
import DeployGithubConnectForm from '@/components/deploys/DeployGithubConnectForm.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { DeployGithubConnectionView } from '@/bindings/DeployGithubConnectionView'
import { useDeploysStore } from '@/stores/deploys'
import { rememberGithubReturn, type GithubReturn } from '@/utils/githubConnect'

const props = withDefaults(defineProps<{ returnTo?: GithubReturn; startConnecting?: boolean }>(), {
  returnTo: 'connections',
  startConnecting: false,
})

const deploys = useDeploysStore()
const { confirm } = useConfirm()
const connecting = ref(props.startConnecting)
const refreshing = ref<number | null>(null)
const error = ref<string | null>(null)

/** Instalar (ou trocar os repositórios): o GitHub volta para cá sozinho. */
function install(connection: DeployGithubConnectionView) {
  rememberGithubReturn(props.returnTo)
  window.location.assign(connection.installUrl)
}

function openSettings(connection: DeployGithubConnectionView) {
  window.open(connection.settingsUrl, '_blank', 'noopener')
}

async function refresh(connection: DeployGithubConnectionView) {
  refreshing.value = connection.id
  error.value = null
  try {
    await deploys.refreshGithub(connection.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível conferir no GitHub'
  } finally {
    refreshing.value = null
  }
}

async function remove(connection: DeployGithubConnectionView) {
  const ok = await confirm({
    title: `Remover a conexão com ${connection.account}?`,
    message:
      'A chave do app é apagada desta central. O app continua no GitHub: para apagá-lo de vez, abra as configurações dele (Advanced → Delete GitHub App).',
    confirmText: 'Remover',
    confirmColor: 'error',
    icon: 'mdi-delete',
  })
  if (!ok) return
  error.value = null
  try {
    await deploys.removeGithub(connection.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível remover'
  }
}
</script>

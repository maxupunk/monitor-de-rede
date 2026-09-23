<template>
  <div>
    <PageHeader
      title="Projetos compose"
      subtitle="Stacks descobertas pelas labels dos containers; as ações rodam no próprio servidor"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-refresh" :loading="loading" @click="load">
          Atualizar
        </v-btn>
      </template>
    </PageHeader>

    <v-alert
      v-if="error"
      type="error"
      variant="tonal"
      closable
      class="mb-4"
      @click:close="error = null"
    >
      {{ error }}
    </v-alert>
    <v-alert
      v-if="!composeReady"
      type="info"
      variant="tonal"
      class="mb-4"
      icon="mdi-information-outline"
    >
      {{ composeUnavailableReason }}
    </v-alert>

    <v-row v-if="projects.length > 0" dense>
      <v-col v-for="project in projects" :key="project.name" cols="12" lg="6">
        <v-card rounded="xl" variant="outlined" class="h-100">
          <v-card-title class="d-flex align-center ga-2">
            <v-icon color="primary">mdi-file-cabinet</v-icon>
            <span class="text-truncate">{{ project.name }}</span>
            <v-spacer></v-spacer>
            <v-chip size="small" variant="tonal" :color="runningColor(project)">
              {{ project.running }}/{{ project.containers }} rodando
            </v-chip>
          </v-card-title>
          <v-card-subtitle class="text-truncate">{{ project.workingDir }}</v-card-subtitle>

          <v-card-text class="pb-2">
            <template v-if="canAct">
              <div class="text-overline text-medium-emphasis">Projeto inteiro</div>
              <div class="d-flex flex-wrap ga-2 mb-4">
                <v-btn
                  v-for="action in projectActions"
                  :key="action.value"
                  :color="action.color"
                  :prepend-icon="action.icon"
                  variant="tonal"
                  size="small"
                  @click="ask(project.name, action.value)"
                >
                  {{ action.title }}
                  <v-tooltip activator="parent" location="top" max-width="280">
                    {{ action.hint }}
                  </v-tooltip>
                </v-btn>
              </div>
            </template>

            <div class="text-overline text-medium-emphasis">
              Serviços ({{ project.services.length }})
            </div>
            <v-list density="compact" class="bg-transparent pa-0">
              <v-list-item
                v-for="service in project.services"
                :key="service.name"
                :title="service.name"
                :subtitle="serviceSubtitle(service)"
                class="px-0"
              >
                <template #prepend>
                  <v-icon :color="serviceColor(service)" size="small" class="mr-2">
                    mdi-circle
                  </v-icon>
                </template>
                <template v-if="canAct" #append>
                  <div class="d-flex ga-1">
                    <v-btn
                      v-for="action in serviceActions(service)"
                      :key="action.value"
                      :icon="action.icon"
                      :color="action.color"
                      :aria-label="`${action.title} ${service.name}`"
                      size="small"
                      variant="text"
                      @click="ask(project.name, action.value, service.name)"
                    >
                      <v-icon>{{ action.icon }}</v-icon>
                      <v-tooltip activator="parent" location="top">
                        {{ action.title }} {{ service.name }}
                      </v-tooltip>
                    </v-btn>
                  </div>
                </template>
              </v-list-item>
            </v-list>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
    <v-card v-else-if="!loading" rounded="xl" variant="outlined" class="pa-8 text-center">
      <v-icon size="40" color="medium-emphasis">mdi-file-cabinet</v-icon>
      <div class="text-body-1 mt-2">Nenhum projeto compose neste host.</div>
    </v-card>

    <v-dialog v-model="confirm.open" max-width="460">
      <v-card rounded="xl">
        <v-card-title>{{ confirmTitle }}</v-card-title>
        <v-card-text>
          <p class="mb-2">{{ confirmHint }}</p>
          <p class="text-medium-emphasis mb-0">
            Alvo: <strong>{{ confirm.target }}</strong
            >. O progresso aparece no topo da tela.
          </p>
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="confirm.open = false">Cancelar</v-btn>
          <v-btn
            :color="confirmColor"
            variant="flat"
            :loading="docker.actionLoading"
            @click="runAction"
          >
            {{ confirmActionTitle }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import type { ComposeAction } from '@/bindings/ComposeAction'
import type { ComposeProject } from '@/bindings/ComposeProject'
import type { ComposeService } from '@/bindings/ComposeService'

interface ActionMeta {
  value: ComposeAction
  title: string
  icon: string
  color: string
  /** O que o comando faz, em linguagem de quem opera — não o nome do CLI. */
  hint: string
}

const docker = useDockerStore()
const auth = useAuthStore()

const projects = ref<ComposeProject[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const confirm = reactive({
  open: false,
  project: '',
  service: null as string | null,
  action: 'up' as ComposeAction,
  target: '',
})

const ACTIONS: Record<ComposeAction, ActionMeta> = {
  pull: {
    value: 'pull',
    title: 'Baixar imagens',
    icon: 'mdi-download',
    color: 'primary',
    hint: 'Baixa a versão mais recente das imagens. Os containers só passam a usá-las depois de "Aplicar".',
  },
  up: {
    value: 'up',
    title: 'Aplicar',
    icon: 'mdi-play',
    color: 'success',
    hint: 'Cria ou recria os containers conforme o arquivo compose e as imagens atuais, e os deixa rodando.',
  },
  restart: {
    value: 'restart',
    title: 'Reiniciar',
    icon: 'mdi-restart',
    color: 'info',
    hint: 'Reinicia os containers sem recriá-los.',
  },
  stop: {
    value: 'stop',
    title: 'Parar',
    icon: 'mdi-stop',
    color: 'warning',
    hint: 'Para os containers sem removê-los. "Aplicar" ou "Reiniciar" os traz de volta.',
  },
  down: {
    value: 'down',
    title: 'Derrubar',
    icon: 'mdi-arrow-collapse-down',
    color: 'error',
    hint: 'Para e remove os containers e as redes do projeto. Os volumes (dados) são mantidos.',
  },
}

const projectActions = [ACTIONS.pull, ACTIONS.up, ACTIONS.restart, ACTIONS.stop, ACTIONS.down]

/** `down` não existe para um serviço só; parar/iniciar depende do estado. */
function serviceActions(service: ComposeService): ActionMeta[] {
  const toggle = service.running > 0 ? ACTIONS.stop : { ...ACTIONS.up, title: 'Iniciar' }
  return [ACTIONS.restart, toggle, { ...ACTIONS.pull, title: 'Baixar imagem de' }]
}

const composeReady = computed(
  () => Boolean(docker.selectedHost?.composeAvailable) && docker.allows('compose')
)
const canAct = computed(() => composeReady.value && auth.isAdmin)

const composeUnavailableReason = computed(() => {
  if (!docker.selectedHost || docker.selectedHost.kind === 'local') {
    return 'Ações de compose rodam em servidores remotos. Nesta central os projetos são apenas listados.'
  }
  if (!docker.selectedHost.composeAvailable) {
    return 'Este servidor não tem o plugin docker compose instalado.'
  }
  return 'A política local deste servidor não libera ações de compose (AGENT_ALLOW).'
})

function runningColor(item: { running: number; containers: number }): string {
  if (item.running === 0) return 'error'
  return item.running === item.containers ? 'success' : 'warning'
}

function serviceColor(service: ComposeService): string {
  return runningColor(service)
}

function serviceSubtitle(service: ComposeService): string {
  if (service.running === 0) return 'parado'
  if (service.containers === 1) return 'rodando'
  return `${service.running} de ${service.containers} réplicas rodando`
}

const confirmMeta = computed(() => ACTIONS[confirm.action])
const confirmActionTitle = computed(() =>
  confirm.service && confirm.action === 'up' ? 'Iniciar' : confirmMeta.value.title
)
const confirmTitle = computed(() => `${confirmActionTitle.value} — ${confirm.target}`)
const confirmHint = computed(() => confirmMeta.value.hint)
const confirmColor = computed(() => confirmMeta.value.color)

async function load() {
  loading.value = true
  error.value = null
  try {
    projects.value = (await docker.api.composeProjects()).data
  } catch (reason: unknown) {
    error.value = reason instanceof Error ? reason.message : 'Erro ao listar os projetos'
  } finally {
    loading.value = false
  }
}

function ask(project: string, action: ComposeAction, service: string | null = null) {
  confirm.project = project
  confirm.service = service
  confirm.action = action
  confirm.target = service ? `${project}/${service}` : `projeto ${project}`
  confirm.open = true
}

async function runAction() {
  const ok = await docker.runAction(() =>
    docker.api.composeAction(confirm.project, confirm.action, confirm.service ?? undefined)
  )
  if (ok) confirm.open = false
}

watch(() => docker.selectedHostKey, load, { immediate: true })
</script>

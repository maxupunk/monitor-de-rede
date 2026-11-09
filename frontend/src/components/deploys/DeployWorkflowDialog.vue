<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 900"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-item class="pt-4 px-6">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-github</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold"
          >Ligar o GitHub — {{ application?.name }}</v-card-title
        >
        <v-card-subtitle class="text-wrap text-high-emphasis">
          O GitHub compila e publica as imagens; depois avisa esta central, que decide quando e onde
          aplicar. Nenhum segredo vai para o GitHub: a central confere o token OIDC do próprio
          Actions.
        </v-card-subtitle>
      </v-card-item>

      <v-card-text v-if="application" class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" class="mb-4">{{ error }}</v-alert>

        <!-- O endereço que o workflow chama -->
        <v-text-field
          v-model="centralUrl"
          label="Endereço público desta central"
          hint="É para onde o GitHub Actions manda o aviso de versão nova. Vai escrito no workflow."
          persistent-hint
          variant="outlined"
          prepend-inner-icon="mdi-web"
          :rules="[centralUrlRule]"
          class="mb-2"
          @change="applyCentralUrl"
          @keyup.enter="applyCentralUrl"
        ></v-text-field>
        <v-alert
          v-if="looksPrivate(centralUrl)"
          type="warning"
          variant="tonal"
          density="compact"
          class="mb-4"
        >
          O GitHub não alcança endereços da rede local. Use o endereço público da central (o do
          domínio, túnel ou proxy).
        </v-alert>

        <!-- O repositório, conferido pela conexão -->
        <v-card v-if="hasAccess" border flat class="pa-4 mb-4" data-test="github-status">
          <div class="d-flex align-center ga-2 mb-3">
            <span class="text-subtitle-1 font-weight-bold flex-grow-1">
              Em {{ application.repository }}
            </span>
            <v-btn
              color="primary"
              variant="text"
              size="small"
              prepend-icon="mdi-refresh"
              :loading="statusLoading"
              @click="loadStatus"
            >
              Conferir de novo
            </v-btn>
          </div>
          <v-skeleton-loader v-if="statusLoading && !status" type="list-item@2" />
          <v-alert
            v-else-if="status?.unavailableReason"
            type="warning"
            variant="tonal"
            density="compact"
          >
            {{ status.unavailableReason }}
          </v-alert>
          <template v-else-if="status">
            <div class="status-row">
              <v-icon :color="workflowMeta.color">{{ workflowMeta.icon }}</v-icon>
              <div class="flex-grow-1">
                <div class="font-weight-bold">Workflow: {{ workflowMeta.label }}</div>
                <div v-if="status.workflow === 'different'" class="text-body-2 text-high-emphasis">
                  Foi editado à mão ou gerado antes de mudar as imagens ou os alvos.
                </div>
              </div>
              <v-btn
                v-if="status.workflowUrl"
                :href="status.workflowUrl"
                target="_blank"
                rel="noopener"
                color="primary"
                variant="text"
                size="small"
                append-icon="mdi-open-in-new"
              >
                Ver
              </v-btn>
            </div>
            <div v-if="status.pullRequest" class="status-row">
              <v-icon color="info">mdi-source-pull</v-icon>
              <div class="flex-grow-1 font-weight-bold">
                PR #{{ status.pullRequest.number }} aguardando o merge
              </div>
              <v-btn
                :href="status.pullRequest.url"
                target="_blank"
                rel="noopener"
                color="primary"
                variant="flat"
                size="small"
                append-icon="mdi-open-in-new"
              >
                Revisar no GitHub
              </v-btn>
            </div>
            <div v-if="status.lastBuild && buildStatus" class="status-row">
              <v-icon :color="buildStatus.color">{{ buildStatus.icon }}</v-icon>
              <div class="flex-grow-1">
                <div class="font-weight-bold">Último build: {{ buildStatus.label }}</div>
                <div class="text-body-2 text-high-emphasis">
                  {{ status.lastBuild.branch }} · {{ status.lastBuild.shortSha }} ·
                  {{ formatRelativeTime(status.lastBuild.createdAt) }}
                </div>
              </div>
              <v-btn
                :href="status.lastBuild.url"
                target="_blank"
                rel="noopener"
                color="primary"
                variant="text"
                size="small"
                append-icon="mdi-open-in-new"
              >
                Ver
              </v-btn>
            </div>

            <div v-if="status.workflow !== 'current'" class="mt-3">
              <template v-if="status.canWrite">
                <!-- Um workflow próprio (com testes, build args) não deve ser trocado sem querer. -->
                <div
                  v-if="status.workflow === 'different'"
                  class="text-body-2 text-high-emphasis mb-2"
                >
                  Se o workflow do repositório é próprio (com testes ou build args), mantenha-o: ele
                  só precisa terminar avisando a central. O PR abaixo propõe trocar pelo gerado.
                </div>
                <v-btn
                  color="primary"
                  :variant="status.workflow === 'different' ? 'tonal' : 'flat'"
                  prepend-icon="mdi-source-pull"
                  :loading="opening"
                  :disabled="centralUrlRule(centralUrl) !== true"
                  @click="openPullRequest"
                >
                  {{ pullRequestLabel }}
                </v-btn>
              </template>
              <div v-else class="text-body-2 text-high-emphasis">
                A conexão não pode escrever no repositório: crie o arquivo à mão (abaixo) ou conecte
                o GitHub de novo marcando "abrir o PR do workflow".
              </div>
            </div>
          </template>
        </v-card>

        <!-- À mão -->
        <v-expansion-panels v-model="manualPanel" variant="accordion" class="mb-4">
          <v-expansion-panel value="manual" elevation="0" class="border">
            <v-expansion-panel-title class="font-weight-bold">
              {{ status?.canWrite ? 'Fazer à mão' : 'Criar o workflow' }}
            </v-expansion-panel-title>
            <v-expansion-panel-text>
              <v-skeleton-loader v-if="loading" type="paragraph@3"></v-skeleton-loader>
              <template v-else-if="workflow">
                <div class="text-body-2 text-high-emphasis mb-2">
                  Salve em <code>{{ workflow.path }}</code
                  >. Ele roda a cada push nas branches dos alvos ({{ references }}) e só avisa a
                  central depois que todas as imagens foram publicadas.
                </div>
                <CodeTextarea
                  :model-value="workflow.yaml"
                  readonly
                  height="min(42vh, 420px)"
                  hide-details
                  class="mb-2"
                />
                <div class="d-flex flex-wrap ga-2">
                  <v-btn
                    color="primary"
                    variant="flat"
                    :prepend-icon="copied ? 'mdi-check' : 'mdi-content-copy'"
                    @click="copyYaml"
                  >
                    {{ copied ? 'Copiado' : 'Copiar o YAML' }}
                  </v-btn>
                  <v-btn
                    :href="createUrl"
                    target="_blank"
                    rel="noopener"
                    color="primary"
                    variant="tonal"
                    prepend-icon="mdi-file-plus-outline"
                  >
                    Criar no GitHub
                  </v-btn>
                </div>
                <div class="text-body-2 text-high-emphasis mt-3">
                  Opcional: para trocar o endereço da central sem editar o arquivo, crie a variável
                  <code>{{ workflow.urlVariable }}</code> em
                  <a
                    :href="`https://github.com/${application.repository}/settings/variables/actions`"
                    target="_blank"
                    rel="noopener"
                    class="font-weight-bold"
                    >Settings → Variables</a
                  >.
                </div>
              </template>
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>

        <!-- O compose -->
        <div class="text-subtitle-1 font-weight-bold mb-1">
          Confira o compose: as imagens publicadas
        </div>
        <div class="text-body-2 text-high-emphasis mb-2">
          No servidor nada é compilado. Cada serviço com <code>build:</code> precisa de uma
          <code>image:</code> com a tag em <code>{{ application.imageTagVariable }}</code> — o
          <code>build:</code> pode ficar para o desenvolvimento local.
        </div>
        <template v-if="application.images.length">
          <CopyableCommand
            v-for="image in application.images"
            :key="image.name"
            :label="image.name"
            :command="imageLine(image.name)"
            class="mb-2"
          />
        </template>
        <v-alert v-else type="info" variant="tonal" density="compact">
          Esta aplicação não lista imagens para compilar: o workflow só avisa a central. Cadastre as
          imagens na aplicação para o GitHub compilá-las.
        </v-alert>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="flat" @click="emit('update:modelValue', false)">
          Concluir
        </v-btn>
      </v-card-actions>
    </v-card>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="6000">
      {{ feedback.message }}
      <template v-if="feedback.url" #actions>
        <v-btn :href="feedback.url" target="_blank" rel="noopener" variant="text">Abrir</v-btn>
      </template>
    </v-snackbar>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import CodeTextarea from '@/components/CodeTextarea.vue'
import CopyableCommand from '@/components/CopyableCommand.vue'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployGithubStatusView } from '@/bindings/DeployGithubStatusView'
import type { DeployWorkflowView } from '@/bindings/DeployWorkflowView'
import { useDeploysStore } from '@/stores/deploys'
import { buildMeta, WORKFLOW_STATE } from '@/utils/deployPresentation'
import { formatRelativeTime } from '@/utils/formatters'
import {
  centralUrlRule,
  looksPrivate,
  storeCentralUrl,
  storedCentralUrl,
} from '@/utils/githubConnect'

const props = defineProps<{ modelValue: boolean; application: DeployApplicationView | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const deploys = useDeploysStore()
const workflow = ref<DeployWorkflowView | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const copied = ref(false)
const centralUrl = ref(storedCentralUrl())
const status = ref<DeployGithubStatusView | null>(null)
const statusLoading = ref(false)
const opening = ref(false)
const manualPanel = ref<string | undefined>(undefined)
const feedback = reactive({ visible: false, message: '', color: 'success', url: '' })

/** Com conexão ou token, a central confere o repositório por você. */
const hasAccess = computed(() =>
  Boolean(props.application?.githubConnectionId || props.application?.credentialId)
)

const workflowMeta = computed(() => WORKFLOW_STATE[status.value?.workflow ?? 'unknown'])
const buildStatus = computed(() =>
  status.value?.lastBuild ? buildMeta(status.value.lastBuild) : null
)

const pullRequestLabel = computed(() => {
  if (status.value?.pullRequest) return 'Atualizar o PR'
  return status.value?.workflow === 'different'
    ? 'Propor o workflow gerado num PR'
    : 'Abrir o PR com o workflow'
})

const references = computed(() => {
  const list = props.application?.targets.map((target) => target.reference) ?? []
  return [...new Set(list)].join(', ') || 'main'
})

/** "Criar no GitHub" já com o nome e o conteúdo do arquivo. */
const createUrl = computed(() => {
  if (!props.application || !workflow.value) return undefined
  const query = new URLSearchParams({ filename: workflow.value.path, value: workflow.value.yaml })
  return `https://github.com/${props.application.repository}/new/HEAD?${query.toString()}`
})

function imageLine(name: string): string {
  const owner = (props.application?.repository.split('/')[0] ?? '').toLowerCase()
  const variable = props.application?.imageTagVariable ?? 'IMAGE_TAG'
  return `image: ghcr.io/${owner}/${name}:\${${variable}:-latest}`
}

function cleanCentralUrl(): string | null {
  const value = centralUrl.value.trim().replace(/\/+$/, '')
  return centralUrlRule(value) === true ? value : null
}

async function copyYaml() {
  if (!workflow.value) return
  try {
    await navigator.clipboard.writeText(workflow.value.yaml)
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
  } catch {
    copied.value = false
  }
}

async function loadWorkflow() {
  const id = props.application?.id
  const url = cleanCentralUrl()
  if (id === undefined || !url) return
  loading.value = true
  error.value = null
  try {
    workflow.value = await deploys.workflow(id, url)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível gerar o workflow'
  } finally {
    loading.value = false
  }
}

async function loadStatus() {
  const id = props.application?.id
  const url = cleanCentralUrl()
  if (id === undefined || !url || !hasAccess.value) return
  statusLoading.value = true
  try {
    status.value = await deploys.githubStatus(id, url)
  } catch (err) {
    status.value = null
    error.value = err instanceof Error ? err.message : 'Não foi possível conferir o repositório'
  } finally {
    statusLoading.value = false
    // Sem quem abra o PR, o caminho é o manual: já aberto.
    manualPanel.value = status.value?.canWrite ? undefined : 'manual'
  }
}

/** O endereço mudou: o YAML (e a comparação com o do repositório) também. */
function applyCentralUrl() {
  const url = cleanCentralUrl()
  if (!url) return
  storeCentralUrl(url)
  void loadWorkflow()
  void loadStatus()
}

async function openPullRequest() {
  const id = props.application?.id
  const url = cleanCentralUrl()
  if (id === undefined || !url) return
  opening.value = true
  error.value = null
  try {
    const pull = await deploys.githubPullRequest(id, url)
    storeCentralUrl(url)
    feedback.message = `PR #${pull.number} aberto: revise e faça o merge para ligar o deploy.`
    feedback.color = 'success'
    feedback.url = pull.url
    feedback.visible = true
    await loadStatus()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível abrir o PR'
  } finally {
    opening.value = false
  }
}

watch(
  () => [props.modelValue, props.application?.id] as const,
  ([visible, id]) => {
    if (!visible || id === undefined) return
    centralUrl.value = storedCentralUrl()
    status.value = null
    error.value = null
    manualPanel.value = hasAccess.value ? undefined : 'manual'
    void loadWorkflow()
    void loadStatus()
  },
  { immediate: true }
)
</script>

<style scoped>
.status-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 0;
}
</style>

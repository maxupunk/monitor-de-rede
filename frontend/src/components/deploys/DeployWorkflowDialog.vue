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

      <v-card-text class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" class="mb-4">{{ error }}</v-alert>
        <v-skeleton-loader v-if="loading" type="paragraph@3"></v-skeleton-loader>
        <template v-else-if="workflow && application">
          <div class="step">
            <v-avatar color="primary" size="28" class="step-number">1</v-avatar>
            <div class="flex-grow-1 min-w-0">
              <div class="font-weight-bold mb-1">Diga ao repositório onde está a central</div>
              <div class="text-body-2 text-high-emphasis mb-2">
                Em <strong>Settings → Secrets and variables → Actions → Variables</strong>, crie a
                variável abaixo com o endereço público desta central.
              </div>
              <CopyableCommand :label="workflow.urlVariable" :command="publicUrl" class="mb-2" />
              <v-btn
                :href="`https://github.com/${application.repository}/settings/variables/actions`"
                target="_blank"
                rel="noopener"
                color="primary"
                variant="tonal"
                size="small"
                prepend-icon="mdi-open-in-new"
              >
                Abrir as variáveis do repositório
              </v-btn>
            </div>
          </div>

          <div class="step">
            <v-avatar color="primary" size="28" class="step-number">2</v-avatar>
            <div class="flex-grow-1 min-w-0">
              <div class="font-weight-bold mb-1">Crie o workflow</div>
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
            </div>
          </div>

          <div class="step">
            <v-avatar color="primary" size="28" class="step-number">3</v-avatar>
            <div class="flex-grow-1 min-w-0">
              <div class="font-weight-bold mb-1">Aponte o compose para as imagens publicadas</div>
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
                Esta aplicação não lista imagens para compilar: o workflow só avisa a central.
                Cadastre as imagens na aplicação para o GitHub compilá-las.
              </v-alert>
            </div>
          </div>
        </template>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="flat" @click="emit('update:modelValue', false)">
          Concluir
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import CodeTextarea from '@/components/CodeTextarea.vue'
import CopyableCommand from '@/components/CopyableCommand.vue'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployWorkflowView } from '@/bindings/DeployWorkflowView'
import { useDeploysStore } from '@/stores/deploys'

const props = defineProps<{ modelValue: boolean; application: DeployApplicationView | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const deploys = useDeploysStore()
const workflow = ref<DeployWorkflowView | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const copied = ref(false)

/** O endereço por onde o operador abriu a central — o público, na prática. */
const publicUrl = computed(() => window.location.origin)

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

watch(
  () => [props.modelValue, props.application?.id] as const,
  async ([visible, id]) => {
    if (!visible || id === undefined) return
    loading.value = true
    error.value = null
    try {
      workflow.value = await deploys.workflow(id)
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Não foi possível gerar o workflow'
    } finally {
      loading.value = false
    }
  },
  { immediate: true }
)
</script>

<style scoped>
.step {
  display: flex;
  gap: 16px;
  margin-bottom: 24px;
}

.step-number {
  flex: 0 0 auto;
  font-weight: 700;
}
</style>

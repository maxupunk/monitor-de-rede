<template>
  <div>
    <v-alert
      v-if="!deploys.githubConnections.length && !editing"
      type="info"
      variant="tonal"
      class="mb-4"
      icon="mdi-github"
    >
      <div class="font-weight-bold mb-1">Conecte o GitHub e o resto se preenche sozinho</div>
      <div class="text-body-2 mb-2">
        Você escolhe o repositório numa lista; o NetMonitor lê o compose e preenche as imagens e a
        variável da tag. Depois ele mesmo abre o PR do workflow.
      </div>
      <v-btn
        color="primary"
        variant="flat"
        size="small"
        prepend-icon="mdi-github"
        @click="emit('connect-github')"
      >
        Conectar GitHub
      </v-btn>
    </v-alert>

    <v-row dense>
      <v-col v-if="deploys.githubConnections.length" cols="12" sm="5">
        <v-select
          v-model="connectionChoice"
          :items="connectionItems"
          item-title="title"
          item-value="value"
          item-props="props"
          label="Conexão com o GitHub"
          variant="outlined"
          prepend-inner-icon="mdi-github"
        >
          <template #append>
            <IconAction
              icon="mdi-cog"
              label="Conexões e tokens"
              @click="emit('manage-connections')"
            />
          </template>
        </v-select>
      </v-col>
      <v-col cols="12" :sm="deploys.githubConnections.length ? 7 : 6">
        <DeployRepositoryPicker
          v-if="model.githubConnectionId !== null"
          v-model="model.repository"
          :source="source"
          @chosen="onChosen"
        />
        <v-text-field
          v-else
          v-model="model.repository"
          label="Repositório no GitHub *"
          placeholder="dono/repositorio"
          hint="Como aparece no endereço: github.com/dono/repositorio"
          persistent-hint
          variant="outlined"
          prepend-inner-icon="mdi-github"
          :rules="[requiredRule('Informe o repositório'), repositoryRule]"
          @update:model-value="suggestFromRepository"
        >
          <template #append>
            <IconAction
              icon="mdi-auto-fix"
              label="Preencher pelo compose do repositório"
              :loading="discovering"
              :disabled="repositoryRule(model.repository) !== true"
              @click="discover"
            />
          </template>
        </v-text-field>
      </v-col>
    </v-row>

    <v-alert
      v-if="discovering || discovery || discoveryError"
      :type="discoveryError ? 'warning' : 'success'"
      variant="tonal"
      density="compact"
      class="mt-2"
      data-test="compose-discovery"
    >
      <template v-if="discovering">Lendo o compose do repositório…</template>
      <template v-else-if="discoveryError">{{ discoveryError }}</template>
      <template v-else-if="discovery">
        <div>{{ discoverySummary }}</div>
        <ul v-if="discovery.notes.length" class="mt-1 pl-4">
          <li v-for="note in discovery.notes" :key="note">{{ note }}</li>
        </ul>
      </template>
    </v-alert>

    <v-row dense class="mt-2">
      <v-col cols="12" sm="6">
        <v-text-field
          v-model="model.name"
          label="Nome *"
          placeholder="Ex.: SGCv4"
          variant="outlined"
          :rules="[requiredRule('Informe um nome')]"
          @update:model-value="nameTouched = true"
        ></v-text-field>
      </v-col>
      <v-col cols="12" sm="6">
        <v-combobox
          v-model="model.composeFiles"
          :items="discovery?.files ?? []"
          label="Arquivos compose *"
          hint="Relativos à raiz do repositório"
          persistent-hint
          variant="outlined"
          multiple
          chips
          closable-chips
          :rules="[(value: string[]) => value.length > 0 || 'Informe ao menos um arquivo']"
        ></v-combobox>
      </v-col>
      <v-col cols="12" sm="6">
        <v-select
          v-model="model.credentialId"
          :items="credentialItems"
          :label="model.githubConnectionId !== null ? 'Token para imagens privadas' : 'Token'"
          :hint="
            model.githubConnectionId !== null
              ? 'Só se as imagens no ghcr.io forem privadas'
              : 'Para repositório e imagens privados'
          "
          persistent-hint
          variant="outlined"
          clearable
        >
          <template #append>
            <IconAction
              icon="mdi-key-plus"
              label="Conexões e tokens"
              @click="emit('manage-connections')"
            />
          </template>
        </v-select>
      </v-col>
    </v-row>

    <div class="text-subtitle-2 font-weight-bold mt-4 mb-1">Imagens que o GitHub compila</div>
    <div class="text-body-2 text-high-emphasis mb-2">
      Usadas para gerar o workflow. Deixe vazio se as imagens já são publicadas de outro jeito.
    </div>
    <div v-for="(image, index) in model.images" :key="index" class="d-flex ga-2 align-start">
      <v-text-field
        v-model="image.name"
        label="Nome"
        placeholder="sgc-api"
        density="compact"
        variant="outlined"
      ></v-text-field>
      <v-text-field
        v-model="image.dockerfile"
        label="Dockerfile"
        placeholder="apps/api/Dockerfile"
        density="compact"
        variant="outlined"
      ></v-text-field>
      <v-text-field
        v-model="image.context"
        label="Contexto"
        density="compact"
        variant="outlined"
        class="image-context"
      ></v-text-field>
      <v-btn
        icon="mdi-delete"
        color="error"
        variant="text"
        :aria-label="`Remover ${image.name || 'imagem'}`"
        @click="model.images.splice(index, 1)"
      ></v-btn>
    </div>
    <v-btn color="primary" variant="tonal" size="small" prepend-icon="mdi-plus" @click="addImage">
      Adicionar imagem
    </v-btn>

    <v-expansion-panels variant="accordion" class="mt-4">
      <v-expansion-panel elevation="0" class="border">
        <v-expansion-panel-title class="font-weight-bold">Avançado</v-expansion-panel-title>
        <v-expansion-panel-text>
          <v-row dense>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="model.slug"
                label="Identificador"
                hint="Também o nome padrão do projeto compose"
                persistent-hint
                variant="outlined"
                @update:model-value="slugTouched = true"
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="model.imageTagVariable"
                label="Variável da tag das imagens"
                hint="O compose usa ${IMAGE_TAG}; o deploy a preenche com o commit"
                persistent-hint
                variant="outlined"
              ></v-text-field>
            </v-col>
            <v-col cols="12">
              <div class="text-subtitle-2 font-weight-bold mb-1">Vários alvos no mesmo push</div>
              <v-radio-group v-model="model.rollout" hide-details>
                <v-radio value="sequential" color="primary">
                  <template #label>
                    <span>
                      <strong>Um de cada vez</strong> — o próximo só se o anterior subir saudável
                      (recomendado)
                    </span>
                  </template>
                </v-radio>
                <v-radio value="parallel" color="primary">
                  <template #label>
                    <span><strong>Todos juntos</strong> — mais rápido, sem canário</span>
                  </template>
                </v-radio>
              </v-radio-group>
            </v-col>
          </v-row>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import IconAction from '@/components/IconAction.vue'
import DeployRepositoryPicker from '@/components/deploys/DeployRepositoryPicker.vue'
import type { DeployApplicationInput } from '@/bindings/DeployApplicationInput'
import type { DeployComposeDiscovery } from '@/bindings/DeployComposeDiscovery'
import type { DeployRepositoryView } from '@/bindings/DeployRepositoryView'
import { useDeploysStore, type GithubSource } from '@/stores/deploys'
import { applyDiscovery, slugify } from '@/utils/deployForms'
import { requiredRule } from '@/utils/formRules'

const model = defineModel<DeployApplicationInput>({ required: true })
const props = defineProps<{ editing?: boolean }>()
const emit = defineEmits<{ 'manage-connections': []; 'connect-github': [] }>()

const deploys = useDeploysStore()
const nameTouched = ref(Boolean(props.editing))
const slugTouched = ref(Boolean(props.editing))
const discovery = ref<DeployComposeDiscovery | null>(null)
const discovering = ref(false)
const discoveryError = ref<string | null>(null)

/** O `v-select` não lida bem com `null` como valor: 0 é "digitar". */
const MANUAL = 0

const connectionChoice = computed({
  get: () => model.value.githubConnectionId ?? MANUAL,
  set: (value: number) => {
    model.value.githubConnectionId = value === MANUAL ? null : value
    discovery.value = null
    discoveryError.value = null
  },
})

const connectionItems = computed(() => [
  ...deploys.githubConnections.map((connection) => ({
    title: connection.account,
    value: connection.id,
    props: {
      subtitle: connection.installed ? 'GitHub App' : 'Falta instalar o app',
      disabled: !connection.installed,
    },
  })),
  { title: 'Digitar o repositório', value: MANUAL, props: { subtitle: 'Público ou com token' } },
])

const source = computed<GithubSource>(() => ({
  connectionId: model.value.githubConnectionId,
  credentialId: model.value.credentialId,
}))

const credentialItems = computed(() =>
  deploys.credentials.map((credential) => ({
    title: `${credential.name} (${credential.username})`,
    value: credential.id,
  }))
)

const discoverySummary = computed(() => {
  const found = discovery.value
  if (!found) return ''
  if (!found.suggestedFiles.length) return 'Nenhum compose na raiz do repositório.'
  const images = found.images.length
    ? `${found.images.length} imagem(ns) para o GitHub compilar`
    : 'nenhum serviço com build:'
  const variable = found.imageTagVariable ? `, tag em ${found.imageTagVariable}` : ''
  return `Preenchido a partir de ${found.suggestedFiles.join(', ')}: ${images}${variable}.`
})

const REPOSITORY = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/

function cleanRepository(value: string): string {
  return value
    .trim()
    .replace(/\.git$/, '')
    .replace(/^https:\/\/github\.com\//, '')
}

function repositoryRule(value: string): true | string {
  return REPOSITORY.test(cleanRepository(value)) || 'Use dono/repositorio'
}

/** O nome do repositório sugere o nome e o identificador da aplicação. */
function suggestFromRepository(value: string) {
  const clean = cleanRepository(value)
  if (clean !== value) model.value.repository = clean
  const name = clean.split('/')[1] ?? ''
  if (!name) return
  if (!nameTouched.value) model.value.name = name
  if (!slugTouched.value) model.value.slug = slugify(name)
}

/** Lê o compose do repositório e preenche o que ele diz. */
async function discover() {
  const repository = cleanRepository(model.value.repository)
  if (repositoryRule(repository) !== true) return
  discovering.value = true
  discoveryError.value = null
  try {
    const found = await deploys.githubCompose(source.value, repository)
    discovery.value = found
    model.value = applyDiscovery(model.value, found)
  } catch (err) {
    discovery.value = null
    const reason = err instanceof Error ? `: ${err.message}` : ''
    discoveryError.value = `Não deu para ler o compose${reason}. Preencha à mão.`
  } finally {
    discovering.value = false
  }
}

function onChosen(repository: DeployRepositoryView) {
  suggestFromRepository(repository.fullName)
  void discover()
}

function addImage() {
  model.value.images.push({ name: '', dockerfile: 'Dockerfile', context: '.' })
}

onMounted(() => {
  // Aplicação nova com o GitHub conectado: já começa pela lista.
  if (!props.editing && model.value.githubConnectionId === null && !model.value.repository) {
    const installed = deploys.githubConnections.find((connection) => connection.installed)
    if (installed) model.value.githubConnectionId = installed.id
  }
})
</script>

<style scoped>
.image-context {
  max-width: 120px;
}
</style>

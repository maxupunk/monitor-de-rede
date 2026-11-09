<template>
  <div>
    <v-row dense>
      <v-col cols="12" sm="6">
        <v-text-field
          v-model="model.repository"
          label="Repositório no GitHub *"
          placeholder="dono/repositorio"
          hint="Como aparece no endereço: github.com/dono/repositorio"
          persistent-hint
          variant="outlined"
          prepend-inner-icon="mdi-github"
          :rules="[requiredRule('Informe o repositório'), repositoryRule]"
          @update:model-value="suggestFromRepository"
        ></v-text-field>
      </v-col>
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
        <v-select
          v-model="model.credentialId"
          :items="credentialItems"
          label="Credencial"
          hint="Para repositório e imagens privados"
          persistent-hint
          variant="outlined"
          clearable
        >
          <template #append>
            <v-btn
              icon="mdi-key-plus"
              variant="text"
              color="primary"
              aria-label="Gerenciar credenciais"
              @click="emit('manage-credentials')"
            >
              <v-icon>mdi-key-plus</v-icon>
              <v-tooltip activator="parent" location="top">Gerenciar credenciais</v-tooltip>
            </v-btn>
          </template>
        </v-select>
      </v-col>
      <v-col cols="12" sm="6">
        <v-combobox
          v-model="model.composeFiles"
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
import { computed, ref } from 'vue'
import type { DeployApplicationInput } from '@/bindings/DeployApplicationInput'
import { useDeploysStore } from '@/stores/deploys'
import { requiredRule } from '@/utils/formRules'

const model = defineModel<DeployApplicationInput>({ required: true })
const props = defineProps<{ editing?: boolean }>()
const emit = defineEmits<{ 'manage-credentials': [] }>()

const deploys = useDeploysStore()
const nameTouched = ref(Boolean(props.editing))
const slugTouched = ref(Boolean(props.editing))

const credentialItems = computed(() =>
  deploys.credentials.map((credential) => ({
    title: `${credential.name} (${credential.username})`,
    value: credential.id,
  }))
)

const REPOSITORY = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/

function repositoryRule(value: string): true | string {
  const clean = value
    .trim()
    .replace(/\.git$/, '')
    .replace(/^https:\/\/github\.com\//, '')
  return REPOSITORY.test(clean) || 'Use dono/repositorio'
}

/** O nome do repositório sugere o nome e o identificador da aplicação. */
function suggestFromRepository(value: string) {
  const clean = value
    .trim()
    .replace(/\.git$/, '')
    .replace(/^https:\/\/github\.com\//, '')
  if (clean !== value) model.value.repository = clean
  const name = clean.split('/')[1] ?? ''
  if (!name) return
  if (!nameTouched.value) model.value.name = name
  if (!slugTouched.value) {
    model.value.slug = name
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, '-')
      .replace(/^[-_]+|[-_]+$/g, '')
      .slice(0, 63)
  }
}

function addImage() {
  model.value.images.push({ name: '', dockerfile: 'Dockerfile', context: '.' })
}
</script>

<style scoped>
.image-context {
  max-width: 120px;
}
</style>

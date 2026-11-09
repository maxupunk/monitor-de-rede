<template>
  <div>
    <div class="text-body-1 mb-3">
      O NetMonitor cria um <strong>GitHub App</strong> só desta central na conta que você escolher.
      Você confirma no GitHub, escolhe os repositórios e volta para cá — sem copiar token.
    </div>

    <v-radio-group v-model="owner" hide-details class="mb-2">
      <v-radio value="me" color="primary">
        <template #label>
          <span><strong>Minha conta</strong> — a que está logada no GitHub neste navegador</span>
        </template>
      </v-radio>
      <v-radio value="org" color="primary">
        <template #label>
          <span><strong>Uma organização</strong> — você precisa ser dono dela</span>
        </template>
      </v-radio>
    </v-radio-group>
    <v-text-field
      v-if="owner === 'org'"
      v-model="organization"
      label="Organização *"
      placeholder="Ex.: master-sistemas"
      hint="O login dela, como aparece em github.com/…"
      persistent-hint
      variant="outlined"
      prepend-inner-icon="mdi-domain"
      class="mb-2"
    ></v-text-field>

    <v-checkbox v-model="allowWrite" color="primary" hide-details class="mt-1">
      <template #label>
        <span>
          <strong>Deixar o NetMonitor abrir o PR do workflow</strong> (recomendado) — pede escrita
          no conteúdo, nos PRs e nos workflows. Sem isso, você copia o arquivo à mão.
        </span>
      </template>
    </v-checkbox>

    <v-alert type="info" variant="tonal" density="compact" class="mt-3">
      O app fica privado na sua conta, com o webhook desligado. A chave dele mora cifrada nesta
      central; o servidor só recebe um token de leitura de uma hora a cada deploy.
    </v-alert>

    <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mt-3">
      {{ error }}
    </v-alert>

    <div class="d-flex flex-wrap ga-2 mt-4">
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-github"
        :loading="sending"
        :disabled="owner === 'org' && !organization.trim()"
        @click="go"
      >
        Continuar no GitHub
      </v-btn>
      <v-btn variant="text" @click="emit('cancel')">Voltar</v-btn>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useDeploysStore } from '@/stores/deploys'
import { rememberGithubReturn, submitManifest, type GithubReturn } from '@/utils/githubConnect'

const props = defineProps<{ returnTo: GithubReturn }>()
const emit = defineEmits<{ cancel: [] }>()

const deploys = useDeploysStore()
const owner = ref<'me' | 'org'>('me')
const organization = ref('')
const allowWrite = ref(true)
const sending = ref(false)
const error = ref<string | null>(null)

async function go() {
  sending.value = true
  error.value = null
  try {
    const form = await deploys.githubManifest({
      origin: window.location.origin,
      organization: owner.value === 'org' ? organization.value.trim() : undefined,
      allowWrite: allowWrite.value,
    })
    rememberGithubReturn(props.returnTo)
    submitManifest(form)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível falar com o GitHub'
    sending.value = false
  }
}
</script>

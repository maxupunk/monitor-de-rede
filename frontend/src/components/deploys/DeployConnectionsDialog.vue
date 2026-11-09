<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 680"
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
        <v-card-title class="font-weight-bold">Conexões com o GitHub</v-card-title>
        <v-card-subtitle class="text-wrap text-high-emphasis">
          Tudo fica cifrado nesta central. O servidor só recebe o que o deploy precisa, na hora, e
          não guarda.
        </v-card-subtitle>
      </v-card-item>

      <v-card-text class="px-6">
        <div class="text-subtitle-1 font-weight-bold mb-1">Repositórios</div>
        <DeployGithubConnections
          :key="session"
          :return-to="returnTo"
          :start-connecting="startConnecting"
        />

        <v-divider class="my-5"></v-divider>

        <div class="text-subtitle-1 font-weight-bold mb-1">Imagens privadas no ghcr.io</div>
        <div class="text-body-2 text-high-emphasis mb-3">
          O ghcr.io só aceita token clássico para baixar imagem privada. Imagens públicas dispensam.
        </div>
        <DeployTokenCredentials :key="session" />
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="flat" @click="emit('update:modelValue', false)">
          Fechar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import DeployGithubConnections from '@/components/deploys/DeployGithubConnections.vue'
import DeployTokenCredentials from '@/components/deploys/DeployTokenCredentials.vue'
import type { GithubReturn } from '@/utils/githubConnect'

const props = withDefaults(
  defineProps<{ modelValue: boolean; returnTo?: GithubReturn; startConnecting?: boolean }>(),
  { returnTo: 'connections', startConnecting: false }
)
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

/** Cada abertura começa do zero (o atalho "Conectar GitHub" abre o formulário). */
const session = ref(0)
watch(
  () => props.modelValue,
  (visible) => {
    if (visible) session.value += 1
  }
)
</script>

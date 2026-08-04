<template>
  <v-dialog :model-value="modelValue" max-width="820" @update:model-value="onUpdateModelValue">
    <v-card v-if="artifact" class="rounded-lg">
      <v-card-title class="d-flex align-center font-weight-bold">
        <v-icon start color="primary">mdi-file-code-outline</v-icon>
        Pronto para conectar — {{ artifact.label }}
      </v-card-title>

      <v-card-subtitle class="pb-2">
        {{ artifact.fileName }}
      </v-card-subtitle>

      <v-card-text>
        <v-alert
          v-if="hasPrivateKey"
          type="warning"
          variant="tonal"
          density="comfortable"
          class="mb-4"
          text="A chave privada aparece uma única vez. Copie o script agora — depois só é possível rotacionar as chaves."
        ></v-alert>

        <v-list density="compact" class="bg-transparent mb-3">
          <v-list-item
            v-for="(step, index) in artifact.instructions"
            :key="index"
            :title="step"
            :prepend-icon="stepIcons[index] || 'mdi-numeric'"
          ></v-list-item>
        </v-list>

        <div v-if="qrSvg" class="text-center mb-4">
          <!-- eslint-disable-next-line vue/no-v-html -->
          <div class="qr-wrapper d-inline-block pa-3 bg-white rounded-lg" v-html="qrSvg"></div>
          <div class="text-caption text-grey mt-2">
            Escaneie com o aplicativo WireGuard do celular
          </div>
        </div>

        <v-sheet class="script-box rounded-lg pa-4" color="grey-darken-4">
          <pre class="script-content">{{ artifact.content }}</pre>
        </v-sheet>
      </v-card-text>

      <v-card-actions class="px-4 pb-4">
        <v-btn variant="text" prepend-icon="mdi-download" @click="download"> Baixar arquivo </v-btn>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="close">Fechar</v-btn>
        <v-btn
          color="primary"
          variant="flat"
          size="large"
          :prepend-icon="copied ? 'mdi-check' : 'mdi-content-copy'"
          @click="copyAll"
        >
          {{ copied ? 'Copiado!' : 'Copiar tudo' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { VpnArtifact } from '@/stores/vpn'

const props = defineProps<{
  modelValue: boolean
  artifact?: VpnArtifact | null
  qrSvg?: string | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const copied = ref(false)
const stepIcons = ['mdi-numeric-1-circle', 'mdi-numeric-2-circle', 'mdi-numeric-3-circle']

const hasPrivateKey = computed(
  () => !!props.artifact && !props.artifact.content.includes('CHAVE-PRIVADA-INDISPONIVEL')
)

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) copied.value = false
  }
)

function onUpdateModelValue(value: boolean) {
  emit('update:modelValue', value)
}

function close() {
  emit('update:modelValue', false)
}

async function copyAll() {
  if (!props.artifact) return
  try {
    await navigator.clipboard.writeText(props.artifact.content)
    copied.value = true
  } catch {
    copied.value = false
  }
}

function download() {
  if (!props.artifact) return
  const blob = new Blob([props.artifact.content], { type: 'text/plain;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = props.artifact.fileName
  link.click()
  URL.revokeObjectURL(url)
}
</script>

<style scoped>
.script-box {
  max-height: 340px;
  overflow: auto;
}
.script-content {
  color: #e0e0e0;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 12px;
  line-height: 1.6;
  margin: 0;
  white-space: pre;
}
.qr-wrapper :deep(svg) {
  width: 240px;
  height: 240px;
}
</style>

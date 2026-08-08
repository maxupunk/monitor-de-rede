<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 920"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emitOpen"
  >
    <v-card v-if="artifact" class="rounded-lg">
      <v-card-title class="d-flex align-center font-weight-bold">
        <v-icon start color="primary">mdi-check-decagram-outline</v-icon>
        Pronto para conectar — {{ artifact.label }}
      </v-card-title>

      <v-card-subtitle class="pb-2">
        Confira os dados, escolha como instalar e aplique no dispositivo.
      </v-card-subtitle>

      <v-card-text>
        <v-alert
          v-if="hasPrivateKey"
          type="warning"
          variant="tonal"
          density="comfortable"
          class="mb-4"
          text="A chave privada aparece uma única vez. Copie o script (ou leia o QR Code) agora — depois só é possível rotacionar as chaves."
        ></v-alert>
        <v-alert
          v-else
          type="info"
          variant="tonal"
          density="comfortable"
          class="mb-4"
          text="A chave privada já foi entregue e não pode ser exibida novamente. Para reinstalar o dispositivo, rotacione as chaves."
        ></v-alert>

        <!-- Celular: o QR Code é o caminho principal, então abre a tela -->
        <v-sheet v-if="qrCode" class="qr-panel rounded-lg pa-4 mb-4 text-center" color="surface">
          <div class="text-subtitle-2 font-weight-medium mb-1">
            <v-icon size="18" start color="primary">mdi-qrcode-scan</v-icon>
            Leia com o aplicativo WireGuard
          </div>
          <div class="text-caption text-medium-emphasis mb-3">
            No app, toque em "+" › "Ler a partir do código QR".
          </div>
          <!-- eslint-disable-next-line vue/no-v-html -->
          <div class="qr-wrapper d-inline-block pa-3 bg-white rounded-lg" v-html="qrCode"></div>
        </v-sheet>

        <v-alert
          v-else-if="artifact.supportsQrCode"
          type="warning"
          variant="tonal"
          density="comfortable"
          class="mb-4"
          text="QR Code indisponível: sem a chave privada ele geraria um túnel que nunca conecta. Rotacione as chaves para obter um novo."
        ></v-alert>

        <!-- Os dados do túnel, para conferência ou configuração manual -->
        <v-expansion-panels v-model="summaryOpen" variant="accordion" class="mb-4">
          <v-expansion-panel value="summary" elevation="0">
            <v-expansion-panel-title class="text-body-2 font-weight-medium">
              <v-icon size="18" start>mdi-information-outline</v-icon>
              Dados do túnel
            </v-expansion-panel-title>
            <v-expansion-panel-text>
              <v-row dense>
                <v-col v-for="item in artifact.summary" :key="item.label" cols="12" sm="6">
                  <div class="text-caption text-medium-emphasis">{{ item.label }}</div>
                  <div class="text-body-2 summary-value">{{ item.value }}</div>
                </v-col>
              </v-row>
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>

        <v-tabs v-model="activeTab" density="comfortable" color="primary" class="mb-2">
          <v-tab v-for="doc in documents" :key="doc.id" :value="doc.id">
            <v-icon size="18" start>{{ doc.icon }}</v-icon>
            {{ doc.label }}
            <v-chip
              v-if="doc.id === defaultTab && documents.length > 1"
              size="x-small"
              class="ml-2"
            >
              Recomendado
            </v-chip>
          </v-tab>
        </v-tabs>

        <div v-if="activeDocument" class="pt-2">
          <div class="text-caption text-medium-emphasis mb-2">
            {{ activeDocument.hint }} · <code>{{ activeDocument.fileName }}</code>
          </div>

          <v-list density="compact" class="bg-transparent mb-2">
            <v-list-item
              v-for="(step, index) in activeDocument.instructions"
              :key="index"
              :title="step"
              :prepend-icon="stepIcons[index] || 'mdi-numeric'"
            ></v-list-item>
          </v-list>

          <v-sheet class="script-box rounded-lg pa-4" color="grey-darken-4">
            <pre class="script-content">{{ activeDocument.content }}</pre>
          </v-sheet>
        </div>
      </v-card-text>

      <v-card-actions class="px-4 pb-4">
        <v-btn variant="text" prepend-icon="mdi-download" @click="download">Baixar arquivo</v-btn>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="close">Fechar</v-btn>
        <v-btn
          color="primary"
          variant="flat"
          size="large"
          :prepend-icon="copied ? 'mdi-check' : 'mdi-content-copy'"
          @click="copyAll"
        >
          {{ copied ? 'Copiado!' : copyLabel }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { VpnArtifact } from '@/stores/vpn'

/** Uma aba do visualizador: o artefato principal ou uma das variantes de instalação. */
interface ArtifactDocument {
  id: string
  label: string
  hint: string
  icon: string
  fileName: string
  content: string
  instructions: string[]
}

const props = defineProps<{
  modelValue: boolean
  artifact?: VpnArtifact | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const copied = ref(false)
const activeTab = ref('base')
const summaryOpen = ref<string | undefined>()
const stepIcons = ['mdi-numeric-1-circle', 'mdi-numeric-2-circle', 'mdi-numeric-3-circle']

const hasPrivateKey = computed(
  () => !!props.artifact && !props.artifact.content.includes('CHAVE-PRIVADA-INDISPONIVEL')
)

/** O QR Code chega junto do artefato — a chave privada só existe naquela resposta. */
const qrCode = computed(() => props.artifact?.qrSvg || null)

/** Aba do artefato principal — arquivo de configuração ou script do próprio equipamento. */
const baseDocument = computed<ArtifactDocument | null>(() => {
  const artifact = props.artifact
  if (!artifact) return null

  const isConfigFile = artifact.language === 'ini'

  return {
    id: 'base',
    label: isConfigFile ? 'Configuração WireGuard' : 'Script padrão',
    hint: isConfigFile
      ? 'Arquivo .conf oficial — importável em qualquer cliente WireGuard'
      : 'Bloco de comandos para colar no terminal do equipamento',
    icon: isConfigFile ? 'mdi-file-cog-outline' : 'mdi-console-line',
    fileName: artifact.fileName,
    content: artifact.content,
    instructions: artifact.instructions,
  }
})

const documents = computed<ArtifactDocument[]>(() => {
  const base = baseDocument.value
  if (!base) return []

  const variants = (props.artifact?.variants ?? []).map((variant) => ({
    id: variant.id,
    label: variant.label,
    hint: variant.hint,
    icon: variant.icon,
    fileName: variant.fileName,
    content: variant.content,
    instructions: variant.instructions,
  }))

  return [base, ...variants]
})

/**
 * Onde a tela abre. Quando o artefato principal é um arquivo `.conf` (Windows,
 * Linux) o caminho guiado é o script de terminal — ele instala o cliente, grava
 * o perfil e sobe o túnel sozinho. MikroTik e OpenWrt já entregam script na aba
 * principal, então nela mesmo se começa.
 */
const defaultTab = computed(() => {
  const artifact = props.artifact
  const isConfigFile = artifact?.language === 'ini'
  const firstVariant = artifact?.variants?.[0]

  return isConfigFile && firstVariant ? firstVariant.id : 'base'
})

const activeDocument = computed<ArtifactDocument | null>(
  () => documents.value.find((doc) => doc.id === activeTab.value) ?? documents.value[0] ?? null
)

const copyLabel = computed(() =>
  activeDocument.value?.id === 'base' ? 'Copiar tudo' : 'Copiar script'
)

watch(
  () => props.modelValue,
  (isOpen) => {
    if (!isOpen) return

    copied.value = false
    activeTab.value = defaultTab.value
    // No celular o QR resolve sozinho; nos demais, os dados vêm abertos.
    summaryOpen.value = props.artifact?.supportsQrCode ? undefined : 'summary'
  }
)

// Trocar de aba invalida o "Copiado!" — ele se refere ao conteúdo anterior.
watch(activeTab, () => {
  copied.value = false
})

function emitOpen(value: boolean) {
  emit('update:modelValue', value)
}

function close() {
  emit('update:modelValue', false)
}

async function copyAll() {
  const doc = activeDocument.value
  if (!doc) return

  try {
    await navigator.clipboard.writeText(doc.content)
    copied.value = true
  } catch {
    copied.value = false
  }
}

function download() {
  const doc = activeDocument.value
  if (!doc) return

  const blob = new Blob([doc.content], { type: 'text/plain;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = doc.fileName
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
.summary-value {
  font-family: 'Consolas', 'Monaco', monospace;
  overflow-wrap: anywhere;
}
.qr-panel {
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}
.qr-wrapper :deep(svg) {
  width: 240px;
  height: 240px;
}
</style>

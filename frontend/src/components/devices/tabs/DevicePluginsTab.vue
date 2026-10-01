<template>
  <div>
    <!-- Cabeçalho: o que o sistema sabe do equipamento e o que dá para fazer -->
    <div class="d-flex align-center flex-wrap ga-2 mb-3">
      <v-chip v-if="view" size="small" color="primary" variant="tonal">
        <v-icon start size="14">mdi-chip</v-icon>
        {{ view.platform }}
      </v-chip>
      <v-chip v-if="view" size="small" :color="view.firmware ? 'info' : 'warning'" variant="tonal">
        <v-icon start size="14">mdi-update</v-icon>
        {{ view.firmware ? `firmware ${view.firmware}` : 'firmware não lido' }}
      </v-chip>
      <v-spacer></v-spacer>
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-robot-outline"
        size="small"
        @click="askAi"
      >
        Criar ou usar com IA
      </v-btn>
    </div>

    <v-progress-linear v-if="store.deviceLoading && !view" indeterminate color="primary" />
    <v-alert v-if="store.error" type="error" variant="tonal" class="mb-4">
      {{ store.error }}
    </v-alert>

    <template v-if="view">
      <v-tabs v-model="section" color="primary" density="compact" show-arrows class="mb-4">
        <v-tab value="catalog" prepend-icon="mdi-storefront-outline">Catálogo</v-tab>
        <v-tab value="credentials" prepend-icon="mdi-key-variant">
          Credenciais ({{ view.credentials.length }})
        </v-tab>
        <v-tab value="history" prepend-icon="mdi-history">Histórico</v-tab>
      </v-tabs>

      <v-window v-model="section" :touch="false">
        <v-window-item value="catalog">
          <div class="d-flex align-center flex-wrap ga-2 mb-3">
            <span class="text-body-2">
              Instale um plugin e ele ganha uma aba própria neste equipamento, ao lado das demais.
            </span>
            <v-spacer></v-spacer>
            <v-btn
              color="primary"
              variant="flat"
              prepend-icon="mdi-plus"
              size="small"
              @click="openEditor(null)"
            >
              Novo plugin
            </v-btn>
            <v-btn
              color="secondary"
              variant="flat"
              prepend-icon="mdi-file-import-outline"
              size="small"
              @click="fileInput?.click()"
            >
              Importar
            </v-btn>
            <v-btn
              color="secondary"
              variant="text"
              size="small"
              to="/plugins"
              prepend-icon="mdi-bookshelf"
            >
              Biblioteca
            </v-btn>
            <input
              ref="fileInput"
              type="file"
              accept=".json,application/json"
              class="d-none"
              @change="importFile"
            />
          </div>
          <PluginCatalog
            :plugins="view.plugins"
            :installing="installing"
            @install="install"
            @open="openInstalled"
            @review="openReview"
            @edit="openEditor"
          />
        </v-window-item>

        <v-window-item value="credentials">
          <DeviceCredentials
            :device-id="deviceId"
            :credentials="view.credentials"
            :agents="view.agents"
            @notify="notify"
          />
        </v-window-item>

        <v-window-item value="history">
          <PluginRunsList :runs="view.runs" :plugins="view.plugins" />
        </v-window-item>
      </v-window>
    </template>

    <PluginEditorDialog
      v-model="editor.open"
      :plugin-id="editor.pluginId"
      :device-id="deviceId"
      @saved="store.loadDevice(deviceId)"
    />
    <PluginReviewDialog
      v-model="review.open"
      :detail="review.detail"
      :busy="review.busy"
      @review="reviewAgain"
      @accept="acceptReview"
    />

    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="6000" location="bottom">
      {{ feedback.text }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import type { PluginDetail } from '@/bindings/PluginDetail'
import { usePluginsStore } from '@/stores/plugins'
import { useAiStore } from '@/stores/ai'
import { confirm } from '@/composables/useConfirm'
import { readPackageFile } from '@/utils/pluginPresentation'
import PluginEditorDialog from '@/components/plugins/PluginEditorDialog.vue'
import PluginReviewDialog from '@/components/plugins/PluginReviewDialog.vue'
import PluginCatalog from '@/components/plugins/device/PluginCatalog.vue'
import DeviceCredentials from '@/components/plugins/device/DeviceCredentials.vue'
import PluginRunsList from '@/components/plugins/device/PluginRunsList.vue'

const props = defineProps<{
  deviceId: number
  deviceName: string
}>()

const emit = defineEmits<{
  /** Abrir a aba do plugin instalado (ela mora na página do dispositivo). */
  openPlugin: [pluginId: number]
}>()

const store = usePluginsStore()
const aiStore = useAiStore()
const fileInput = ref<HTMLInputElement | null>(null)
const section = ref('catalog')
const installing = ref<number | null>(null)

const feedback = reactive({ show: false, text: '', color: 'success' })
const editor = reactive({ open: false, pluginId: null as number | null })
const review = reactive({
  open: false,
  detail: null as PluginDetail | null,
  busy: null as 'review' | 'accept' | null,
})

const view = computed(() => store.deviceViews[props.deviceId] ?? null)

onMounted(() => {
  // A página já carrega a visão para montar as abas dos instalados.
  if (!view.value) void store.loadDevice(props.deviceId)
})

function notify(text: string, color = 'success') {
  feedback.text = text
  feedback.color = color
  feedback.show = true
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback
}

function askAi() {
  const opened = aiStore.askAbout(
    `Quero agir no dispositivo ${props.deviceName} (id ${props.deviceId}). ` +
      'Veja primeiro os plugins que já servem para ele (list_device_plugins) e me diga o que ' +
      'dá para fazer. Se for preciso criar um plugin novo, siga o guia de autoria e peça ' +
      'minha aprovação a cada acesso ao equipamento.',
    { deviceId: props.deviceId }
  )
  if (!opened) notify('Aguarde a resposta atual do assistente terminar.', 'warning')
}

async function install(pluginId: number) {
  installing.value = pluginId
  try {
    await store.installPlugin(props.deviceId, pluginId)
    openInstalled(pluginId)
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao instalar o plugin'), 'error')
  } finally {
    installing.value = null
  }
}

function openInstalled(pluginId: number) {
  emit('openPlugin', pluginId)
}

function openEditor(pluginId: number | null) {
  editor.pluginId = pluginId
  editor.open = true
}

async function openReview(pluginId: number) {
  try {
    review.detail = await store.fetchDetail(pluginId)
    review.open = true
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao carregar a revisão'), 'error')
  }
}

async function reviewAgain() {
  if (!review.detail) return
  review.busy = 'review'
  try {
    review.detail = await store.lifecycle(review.detail.summary.id, 'review', props.deviceId)
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao revisar'), 'error')
  } finally {
    review.busy = null
  }
}

async function acceptReview(acknowledge: boolean) {
  if (!review.detail) return
  review.busy = 'accept'
  try {
    review.detail = await store.acceptReview(review.detail.summary.id, acknowledge, props.deviceId)
    review.open = false
    notify('Plugin liberado como rascunho. Instale-o e rode os testes antes de ativar.')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao liberar o plugin'), 'error')
  } finally {
    review.busy = null
  }
}

async function importFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  try {
    const pkg = await readPackageFile(file)
    const exclusive = await confirm({
      title: 'Importar plugin',
      message:
        'O plugin entra em quarentena e passa por revisão de segurança antes de poder ser usado. ' +
        `Importar como exclusivo de ${props.deviceName}?`,
      confirmText: 'Só deste equipamento',
      cancelText: 'Para todos os compatíveis',
    })
    const detail = await store.importPlugin(pkg, exclusive ? props.deviceId : null, props.deviceId)
    review.detail = detail
    review.open = true
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao importar o plugin'), 'error')
  }
}
</script>

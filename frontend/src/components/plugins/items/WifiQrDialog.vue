<template>
  <v-dialog :model-value="modelValue" max-width="440" @update:model-value="close">
    <v-card class="rounded-lg">
      <v-card-item>
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-qrcode</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-wrap">{{ ssid }}</v-card-title>
        <v-card-subtitle class="text-wrap">
          Aponte a câmera do celular para entrar na rede.
        </v-card-subtitle>
      </v-card-item>

      <v-card-text>
        <div v-if="loading" class="d-flex flex-column align-center ga-3 py-8">
          <v-progress-circular indeterminate color="primary" />
          <span class="text-body-2">{{ loadingText }}</span>
        </div>

        <v-alert v-else-if="error" type="error" variant="tonal" class="mb-2">
          {{ error }}
          <template #append>
            <v-btn
              color="error"
              variant="flat"
              size="small"
              prepend-icon="mdi-refresh"
              @click="load"
            >
              Tentar de novo
            </v-btn>
          </template>
        </v-alert>

        <template v-else-if="qr">
          <div class="qr-frame mx-auto mb-4">
            <svg
              :viewBox="`0 0 ${qr.size} ${qr.size}`"
              role="img"
              :aria-label="`QR code da rede ${ssid}`"
              shape-rendering="crispEdges"
            >
              <rect :width="qr.size" :height="qr.size" fill="#ffffff" />
              <path :d="qr.path" fill="#000000" />
            </svg>
          </div>

          <div class="d-flex justify-space-between ga-2 text-body-2 py-1">
            <span>Segurança</span>
            <strong>{{ securityText }}</strong>
          </div>
          <div v-if="needsPassword" class="d-flex align-center justify-space-between ga-2 py-1">
            <span class="text-body-2">Senha</span>
            <span class="d-flex align-center ga-1">
              <code class="password">{{ showPassword ? password : '••••••••' }}</code>
              <v-btn
                :icon="showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'"
                :aria-label="showPassword ? 'Esconder a senha' : 'Mostrar a senha'"
                size="small"
                variant="text"
                color="primary"
                @click="showPassword = !showPassword"
              />
              <v-btn
                :icon="copied ? 'mdi-check' : 'mdi-content-copy'"
                aria-label="Copiar a senha"
                size="small"
                variant="text"
                color="primary"
                @click="copyPassword"
              />
            </span>
          </div>
          <v-alert v-if="hidden" type="info" variant="tonal" density="compact" class="mt-2">
            Rede oculta: o código já avisa o celular para procurá-la pelo nome.
          </v-alert>
          <p v-if="source" class="text-body-small mt-3">
            Senha lida agora de {{ source }}. Ela não fica guardada no NetMonitor.
          </p>
        </template>
      </v-card-text>

      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn variant="text" color="primary" @click="close(false)">Fechar</v-btn>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-download"
          :disabled="!qr"
          @click="download"
        >
          Baixar imagem
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * O QR code de uma rede Wi-Fi da lista (`list.wifiQr`). A senha é lida na
 * hora pela ação que revela (ADR 012), no primeiro equipamento que a tem e
 * responde — sem gravar nem publicar — e o código é gerado aqui, no navegador.
 */
import { computed, ref, watch } from 'vue'
import { encode } from 'uqr'
import type { WifiQr } from '@/bindings/WifiQr'
import { withAccess } from '@/composables/useDeviceAccess'
import { usePluginsStore } from '@/stores/plugins'
import { paramsFromItem } from '@/utils/itemList'
import { qrPath, wifiNeedsPassword, wifiQrPayload, wifiQrSecurity } from '@/utils/wifiQr'

const props = defineProps<{
  modelValue: boolean
  pluginId: number
  spec: WifiQr
  item: Record<string, unknown> | null
  /** Quem tem a rede, na ordem em que a senha é tentada. */
  devices: { deviceId: number; name: string }[]
}>()

const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const pluginsStore = usePluginsStore()
const loading = ref(false)
const loadingText = ref('')
const error = ref<string | null>(null)
const password = ref('')
const source = ref<string | null>(null)
const showPassword = ref(false)
const copied = ref(false)

const ssid = computed(() => String(props.item?.[props.spec.ssid] ?? ''))
const encryption = computed(() => String(props.item?.[props.spec.encryption] ?? 'none'))
const hidden = computed(() =>
  props.spec.hidden ? props.item?.[props.spec.hidden] === true : false
)
const needsPassword = computed(() => wifiNeedsPassword(encryption.value))
const securityText = computed(() => {
  switch (wifiQrSecurity(encryption.value)) {
    case 'WPA':
      return 'WPA2/WPA3, com senha'
    case 'WEP':
      return 'WEP, com senha'
    default:
      return encryption.value.startsWith('owe')
        ? 'Aberta protegida, sem senha'
        : 'Aberta, sem senha'
  }
})

const qr = computed(() => {
  if (loading.value || error.value || !ssid.value) return null
  if (needsPassword.value && !password.value) return null
  const { data, size } = encode(
    wifiQrPayload({
      ssid: ssid.value,
      encryption: encryption.value,
      password: password.value,
      hidden: hidden.value,
    }),
    { ecc: 'M', border: 2 }
  )
  return { size, path: qrPath(data), data }
})

watch(
  () => props.modelValue,
  (open) => {
    if (open) void load()
  },
  { immediate: true }
)

/** A senha, do primeiro equipamento que responde. Rede sem senha não lê nada. */
async function load() {
  password.value = ''
  source.value = null
  error.value = null
  showPassword.value = false
  copied.value = false
  if (!needsPassword.value || !props.item) return
  const item = props.item
  loading.value = true
  let lastError = 'Nenhum equipamento com esta rede respondeu.'
  try {
    for (const device of props.devices) {
      loadingText.value = `Lendo a senha em ${device.name}…`
      try {
        const output = await withAccess(
          () =>
            pluginsStore.revealSecret(
              device.deviceId,
              props.pluginId,
              props.spec.reveal.action,
              paramsFromItem(props.spec.reveal, item)
            ),
          { retry: 'auto', what: 'Ler a senha da rede' }
        )
        const value = output[props.spec.secret]
        if (typeof value === 'string' && value !== '') {
          password.value = value
          source.value = device.name
          return
        }
        lastError = `${device.name} não tem senha cadastrada nesta rede.`
      } catch (err: unknown) {
        lastError = err instanceof Error ? err.message : 'Falha ao ler a senha.'
      }
    }
    error.value = lastError
  } finally {
    loading.value = false
  }
}

async function copyPassword() {
  try {
    await navigator.clipboard.writeText(password.value)
    copied.value = true
  } catch {
    copied.value = false
  }
}

/** A imagem para imprimir ou mandar: o código e o nome da rede embaixo. */
function download() {
  const code = qr.value
  if (!code) return
  const scale = 12
  const caption = 48
  const canvas = document.createElement('canvas')
  canvas.width = code.size * scale
  canvas.height = code.size * scale + caption
  const context = canvas.getContext('2d')
  if (!context) return
  context.fillStyle = '#ffffff'
  context.fillRect(0, 0, canvas.width, canvas.height)
  context.fillStyle = '#000000'
  code.data.forEach((row, y) =>
    row.forEach((dark, x) => {
      if (dark) context.fillRect(x * scale, y * scale, scale, scale)
    })
  )
  context.font = 'bold 22px sans-serif'
  context.textAlign = 'center'
  context.fillText(
    `Wi-Fi: ${ssid.value}`,
    canvas.width / 2,
    code.size * scale + 30,
    canvas.width - 16
  )
  const link = document.createElement('a')
  link.href = canvas.toDataURL('image/png')
  link.download = `wifi-${ssid.value.replace(/[^\w-]+/g, '_') || 'rede'}.png`
  link.click()
}

function close(open: boolean) {
  if (open) return
  // A senha some da memória da tela junto com o diálogo.
  password.value = ''
  source.value = null
  emit('update:modelValue', false)
}
</script>

<style scoped>
.qr-frame {
  width: 100%;
  max-width: 280px;
  padding: 8px;
  background: #ffffff;
  border-radius: 12px;
  border: 1px solid rgb(var(--v-theme-primary));
}
.qr-frame svg {
  display: block;
  width: 100%;
  height: auto;
}
.password {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  color: rgb(var(--v-theme-on-surface));
  overflow-wrap: anywhere;
}
</style>

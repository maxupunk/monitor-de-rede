<template>
  <v-btn
    v-if="auth.canManagePlugins"
    :size="size"
    color="primary"
    variant="flat"
    prepend-icon="mdi-puzzle-plus-outline"
    :loading="installing"
    v-bind="$attrs"
    @click.stop="install"
  >
    {{ label ?? 'Instalar agora' }}
  </v-btn>
  <span v-else class="admin-only text-body-small" v-bind="$attrs">
    <v-icon size="16" color="info">mdi-account-key-outline</v-icon>
    Peça a um administrador para instalar «{{ pluginName }}».
  </span>
</template>

<script setup lang="ts">
/**
 * "Instalar agora": instala (e liga, se desligado) o plugin no equipamento
 * ali mesmo, pela store (`installPlugin`). Quem não pode instalar vê quem pode.
 */
import { ref } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { usePluginsStore } from '@/stores/plugins'

defineOptions({ inheritAttrs: false })

const props = withDefaults(
  defineProps<{
    deviceId: number
    pluginId: number
    pluginName: string
    label?: string
    size?: string
  }>(),
  { label: undefined, size: 'small' }
)

const emit = defineEmits<{
  /** Começou a instalar — quem mostra o andamento acompanha. */
  started: []
  installed: []
  failed: [message: string]
}>()

const auth = useAuthStore()
const store = usePluginsStore()
const installing = ref(false)

async function install() {
  installing.value = true
  emit('started')
  try {
    await store.installPlugin(props.deviceId, props.pluginId)
    emit('installed')
  } catch (err: unknown) {
    emit('failed', err instanceof Error && err.message ? err.message : 'Falha ao instalar o plugin')
  } finally {
    installing.value = false
  }
}
</script>

<style scoped>
.admin-only {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: rgb(var(--v-theme-on-surface));
}
</style>

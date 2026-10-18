<template>
  <v-btn
    v-if="auth.canManagePlugins"
    :size="size"
    color="warning"
    variant="flat"
    prepend-icon="mdi-key-plus"
    v-bind="$attrs"
    @click.stop="open"
  >
    {{ label ?? `Cadastrar o acesso (${transportLabel(transport)})` }}
  </v-btn>
  <span v-else class="admin-only text-body-small" v-bind="$attrs">
    <v-icon size="16" color="info">mdi-account-key-outline</v-icon>
    Peça a um administrador para cadastrar o acesso {{ transportLabel(transport) }}.
  </span>
</template>

<script setup lang="ts">
/**
 * "Cadastrar o acesso (SSH)": abre o formulário do acesso ali mesmo
 * (`useDeviceAccess`). Quem não pode cadastrar vê quem pode.
 */
import type { CredentialView } from '@/bindings/CredentialView'
import type { TransportKind } from '@/bindings/TransportKind'
import { requestAccess } from '@/composables/useDeviceAccess'
import { useAuthStore } from '@/stores/auth'
import { transportLabel } from '@/utils/deviceCredentials'

defineOptions({ inheritAttrs: false })

const props = withDefaults(
  defineProps<{
    deviceId: number
    transport: TransportKind
    /** Por que o acesso é pedido — vai no topo do formulário. */
    reason?: string
    label?: string
    size?: string
  }>(),
  { reason: undefined, label: undefined, size: 'small' }
)

const emit = defineEmits<{ saved: [credential: CredentialView] }>()

const auth = useAuthStore()

async function open() {
  const saved = await requestAccess({
    deviceId: props.deviceId,
    transport: props.transport,
    reason: props.reason,
  })
  if (saved) emit('saved', saved)
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

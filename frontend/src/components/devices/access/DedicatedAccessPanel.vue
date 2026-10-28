<template>
  <v-card border flat class="rounded-lg">
    <v-card-item>
      <template #prepend>
        <v-avatar color="success" variant="tonal" rounded="lg">
          <v-icon>mdi-key-chain</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-subtitle-1 font-weight-bold text-wrap">
        Acesso dedicado do NetMonitor
      </v-card-title>
      <v-card-subtitle class="subtitle">
        {{ dedicatedPlatformLabel(access.platform) }} · usuário {{ credential.username }}
      </v-card-subtitle>
    </v-card-item>

    <v-card-text class="pt-0">
      <p class="text-body-2 mb-3">
        Nenhuma senha está guardada: o NetMonitor entra com uma chave só deste equipamento. Trocar a
        senha do administrador não afeta o monitoramento.
      </p>
      <dl class="facts text-body-2">
        <dt>Chave do NetMonitor</dt>
        <dd class="mono">{{ access.keyFingerprint }}</dd>
        <dt>Identidade do equipamento</dt>
        <dd class="mono">{{ access.hostKey ?? 'não fixada' }}</dd>
        <dt>Chave instalada em</dt>
        <dd>{{ formatDateTime(access.provisionedAt) }}</dd>
      </dl>

      <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mt-3">
        {{ error }}
        <div v-if="offerLocalOnly" class="mt-2">
          <v-btn
            color="error"
            variant="flat"
            size="small"
            :loading="busy === 'local'"
            @click="revoke(true)"
          >
            Apagar só no NetMonitor
          </v-btn>
        </div>
      </v-alert>
    </v-card-text>

    <v-card-actions class="px-4 pb-4 flex-wrap ga-2">
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-key-change"
        :loading="busy === 'rotate'"
        :disabled="busy !== null || disabled"
        @click="rotate"
      >
        Rotacionar chave
      </v-btn>
      <v-btn
        color="error"
        variant="flat"
        prepend-icon="mdi-key-remove"
        :loading="busy === 'revoke'"
        :disabled="busy !== null || disabled"
        @click="revoke(false)"
      >
        Revogar acesso
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
/**
 * O acesso dedicado por chave de um equipamento (ADR 016): o que está
 * instalado, e as duas ações sobre ele — rotacionar a chave e revogar.
 */
import { computed, ref } from 'vue'
import type { CredentialView } from '@/bindings/CredentialView'
import { confirm } from '@/composables/useConfirm'
import { usePluginsStore } from '@/stores/plugins'
import { type DedicatedCredential, dedicatedPlatformLabel } from '@/utils/deviceCredentials'
import { formatDateTime } from '@/utils/formatters'

const props = defineProps<{
  deviceId: number
  credential: DedicatedCredential
  disabled?: boolean
}>()

const emit = defineEmits<{
  rotated: [credential: CredentialView]
  /** `leftovers`: o que o equipamento recusou e fica para remover à mão. */
  revoked: [leftovers: string[]]
}>()

const store = usePluginsStore()
const busy = ref<'rotate' | 'revoke' | 'local' | null>(null)
const error = ref<string | null>(null)
const offerLocalOnly = ref(false)

const access = computed(() => props.credential.dedicated)

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

async function rotate() {
  busy.value = 'rotate'
  error.value = null
  offerLocalOnly.value = false
  try {
    emit('rotated', await store.rotateDedicated(props.deviceId))
  } catch (err: unknown) {
    error.value = describe(err, 'Não foi possível rotacionar a chave.')
  } finally {
    busy.value = null
  }
}

async function revoke(localOnly: boolean) {
  const ok = await confirm({
    title: localOnly ? 'Apagar só no NetMonitor' : 'Revogar acesso dedicado',
    message: localOnly
      ? 'A chave continua no equipamento até alguém removê-la lá. Use quando ele foi trocado ou descartado.'
      : 'A chave sai do equipamento e a credencial sai daqui. Para voltar a gerenciá-lo, cadastre o acesso de novo.',
    confirmText: localOnly ? 'Apagar só aqui' : 'Revogar',
    confirmColor: 'error',
  })
  if (!ok) return
  busy.value = localOnly ? 'local' : 'revoke'
  error.value = null
  try {
    const outcome = await store.revokeDedicated(props.deviceId, localOnly)
    emit('revoked', outcome.leftovers)
  } catch (err: unknown) {
    error.value = describe(err, 'Não foi possível revogar o acesso.')
    offerLocalOnly.value = !localOnly
  } finally {
    busy.value = null
  }
}
</script>

<style scoped>
.subtitle {
  opacity: 1;
  color: rgb(var(--v-theme-on-surface));
}

.facts {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 4px 12px;
}

.facts dt {
  font-weight: 600;
}

.facts dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
</style>

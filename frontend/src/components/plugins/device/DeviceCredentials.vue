<template>
  <v-card border flat class="rounded-lg">
    <v-card-title class="d-flex align-center ga-2 text-subtitle-1 font-weight-bold">
      <v-icon color="primary">mdi-key-variant</v-icon>
      Credenciais de acesso
      <v-spacer></v-spacer>
      <v-btn
        size="small"
        color="primary"
        variant="flat"
        prepend-icon="mdi-plus"
        @click="openCredential(null)"
      >
        Adicionar
      </v-btn>
    </v-card-title>
    <v-card-text>
      <div v-if="credentials.length === 0" class="text-body-2">
        Nenhuma credencial. Plugins por SSH/Telnet precisam de uma; HTTP sem login funciona sem.
      </div>
      <v-list v-else density="compact" class="py-0">
        <v-list-item
          v-for="credential in credentials"
          :key="credential.kind"
          :title="`${credential.kind.toUpperCase()} · ${credential.username || '(sem usuário)'}`"
          :subtitle="credentialSubtitle(credential)"
        >
          <template #prepend>
            <v-icon :color="credentialReady(credential) ? 'success' : 'warning'">
              {{ credentialReady(credential) ? 'mdi-lock-check-outline' : 'mdi-lock-clock' }}
            </v-icon>
          </template>
          <template #append>
            <v-btn
              v-if="credential.storage === 'ask' && !credential.sessionActive"
              size="small"
              color="warning"
              variant="flat"
              class="mr-2"
              @click="openSession(credential)"
            >
              Informar senha
            </v-btn>
            <v-btn
              icon="mdi-pencil-outline"
              size="small"
              variant="text"
              color="primary"
              :aria-label="`Editar credencial ${credential.kind}`"
              @click="openCredential(credential)"
            />
            <v-btn
              icon="mdi-delete-outline"
              size="small"
              variant="text"
              color="error"
              :aria-label="`Remover credencial ${credential.kind}`"
              @click="removeCredential(credential)"
            />
          </template>
        </v-list-item>
      </v-list>
    </v-card-text>

    <CredentialDialog
      v-model="dialog.open"
      :existing="dialog.existing"
      :agents="agents"
      :saving="dialog.saving"
      @save="saveCredential"
    />
  </v-card>
</template>

<script setup lang="ts">
import { reactive } from 'vue'
import type { AgentRouteOption } from '@/bindings/AgentRouteOption'
import type { CredentialInput } from '@/bindings/CredentialInput'
import type { CredentialView } from '@/bindings/CredentialView'
import { usePluginsStore } from '@/stores/plugins'
import { confirm, prompt } from '@/composables/useConfirm'
import CredentialDialog from '@/components/plugins/CredentialDialog.vue'

const props = defineProps<{
  deviceId: number
  credentials: CredentialView[]
  agents: AgentRouteOption[]
}>()

const emit = defineEmits<{ notify: [message: string, color: string] }>()

const store = usePluginsStore()
const dialog = reactive({
  open: false,
  existing: null as CredentialView | null,
  saving: false,
})

function describe(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback
}

function credentialReady(credential: CredentialView): boolean {
  return credential.storage === 'vault' ? credential.hasStoredSecret : credential.sessionActive
}

function credentialSubtitle(credential: CredentialView): string {
  const where = credential.viaProbeId
    ? `via agente #${credential.viaProbeId}`
    : 'a partir da central'
  const secret =
    credential.storage === 'vault'
      ? credential.hasStoredSecret
        ? 'senha guardada cifrada'
        : 'sem senha guardada'
      : credential.sessionActive
        ? 'senha da sessão informada'
        : 'senha pedida a cada sessão'
  return `porta ${credential.port}${credential.https ? ' (HTTPS)' : ''} · ${secret} · ${where}`
}

function openCredential(existing: CredentialView | null) {
  dialog.existing = existing
  dialog.open = true
}

async function saveCredential(input: CredentialInput) {
  dialog.saving = true
  try {
    await store.saveCredential(props.deviceId, input)
    dialog.open = false
    emit('notify', 'Credencial salva.', 'success')
  } catch (err: unknown) {
    emit('notify', describe(err, 'Falha ao salvar a credencial'), 'error')
  } finally {
    dialog.saving = false
  }
}

async function removeCredential(credential: CredentialView) {
  const ok = await confirm({
    title: 'Remover credencial',
    message: `Remover a credencial ${credential.kind.toUpperCase()}? Plugins que dependem dela deixam de funcionar.`,
    confirmText: 'Remover',
    confirmColor: 'error',
  })
  if (!ok) return
  try {
    await store.deleteCredential(props.deviceId, credential.kind)
  } catch (err: unknown) {
    emit('notify', describe(err, 'Falha ao remover a credencial'), 'error')
  }
}

async function openSession(credential: CredentialView) {
  const secret = await prompt({
    title: `Senha ${credential.kind.toUpperCase()}`,
    message: `Senha de ${credential.username} para esta sessão (vale 30 minutos; não é gravada).`,
    inputLabel: 'Senha',
    inputType: 'password',
    confirmText: 'Usar nesta sessão',
  })
  if (!secret) return
  try {
    await store.openSession(props.deviceId, credential.kind, secret)
  } catch (err: unknown) {
    emit('notify', describe(err, 'Falha ao registrar a senha'), 'error')
  }
}
</script>

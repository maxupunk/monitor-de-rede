<template>
  <div>
    <!--
      Com uma credencial pronta, a tela não pede nada: mostra qual vai ser
      usada e deixa trocar. Os campos só aparecem quando há o que digitar.
    -->
    <v-sheet
      v-if="useSaved && ready"
      border
      rounded="lg"
      class="pa-3 d-flex align-center flex-wrap ga-3"
    >
      <v-icon color="success" size="28">mdi-lock-check-outline</v-icon>
      <div class="flex-grow-1 min-width-0">
        <div class="font-weight-bold">Credencial salva · {{ kindLabel }}</div>
        <div class="text-body-2">{{ saved?.username }} · {{ secretLabel }}</div>
      </div>
      <v-btn
        variant="tonal"
        color="primary"
        size="small"
        :disabled="disabled"
        prepend-icon="mdi-account-edit-outline"
        @click="useSaved = false"
      >
        Usar outro login
      </v-btn>
    </v-sheet>

    <template v-else>
      <v-alert v-if="saved && !ready" type="info" variant="tonal" density="compact" class="mb-3">
        A credencial {{ kindLabel }} deste equipamento pede a senha a cada sessão. Digite-a abaixo.
      </v-alert>

      <v-row density="compact">
        <v-col cols="12" sm="6">
          <v-text-field
            v-model="username"
            label="Usuário"
            autocomplete="off"
            density="compact"
            variant="outlined"
            :error-messages="errors.username"
            :disabled="disabled"
            prepend-inner-icon="mdi-account-outline"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6">
          <v-text-field
            v-model="password"
            label="Senha"
            autocomplete="new-password"
            :type="showPassword ? 'text' : 'password'"
            :append-inner-icon="showPassword ? 'mdi-eye-off' : 'mdi-eye'"
            density="compact"
            variant="outlined"
            :error-messages="errors.password"
            :disabled="disabled"
            prepend-inner-icon="mdi-lock-outline"
            @click:append-inner="showPassword = !showPassword"
          ></v-text-field>
        </v-col>
      </v-row>

      <v-checkbox
        v-model="save"
        color="primary"
        density="compact"
        :disabled="disabled"
        hide-details
        label="Salvar para os próximos acessos"
      ></v-checkbox>
      <div class="text-body-2 save-hint">
        {{
          save
            ? 'Fica cifrada nas credenciais do dispositivo e serve também aos plugins. Só é salva se o acesso funcionar.'
            : 'Sem salvar, o login vale só para esta ativação.'
        }}
      </div>
    </template>

    <div class="d-flex flex-wrap ga-2 mt-2">
      <v-btn
        v-if="!useSaved && ready"
        variant="text"
        color="primary"
        size="small"
        prepend-icon="mdi-lock-check-outline"
        :disabled="disabled"
        @click="useSaved = true"
      >
        Usar a credencial salva
      </v-btn>
      <v-btn
        variant="text"
        color="primary"
        size="small"
        prepend-icon="mdi-key-variant"
        :to="manageRoute"
        @click="emit('manage')"
      >
        Gerenciar credenciais
      </v-btn>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { CredentialView } from '@/bindings/CredentialView'
import { credentialReady, credentialSecretLabel } from '@/utils/deviceCredentials'

/**
 * O login de acesso ao equipamento: a credencial salva (a mesma da aba
 * "Plugins › Credenciais") ou usuário e senha digitados, com a opção de
 * salvá-los ali.
 */
const props = defineProps<{
  deviceId: number
  /** A credencial que serve ao meio de acesso escolhido — ver `pickShellCredential`. */
  saved: CredentialView | null
  errors: { username: string; password: string }
  disabled?: boolean
}>()

const emit = defineEmits<{
  /** O operador foi gerenciar as credenciais — quem abriu esta tela a fecha. */
  manage: []
}>()

const useSaved = defineModel<boolean>('useSaved', { required: true })
const username = defineModel<string>('username', { required: true })
const password = defineModel<string>('password', { required: true })
const save = defineModel<boolean>('save', { required: true })

const showPassword = ref(false)

const ready = computed(() => Boolean(props.saved && credentialReady(props.saved)))
const kindLabel = computed(() => props.saved?.kind.toUpperCase() ?? '')
const secretLabel = computed(() => (props.saved ? credentialSecretLabel(props.saved) : ''))

const manageRoute = computed(() => ({
  path: `/devices/${props.deviceId}`,
  query: { tab: 'plugins', section: 'credentials' },
}))

// A senha visível não sobrevive à troca de login.
watch(useSaved, () => {
  showPassword.value = false
})
</script>

<style scoped>
.min-width-0 {
  min-width: 0;
}

/* Alinha a explicação ao texto do checkbox, não à caixa. */
.save-hint {
  padding-inline-start: 40px;
}
</style>

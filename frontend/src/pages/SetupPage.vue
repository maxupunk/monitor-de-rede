<template>
  <AuthShell
    wide
    title="Primeiro acesso"
    subtitle="Nenhum usuário cadastrado ainda. Crie o administrador desta instalação."
  >
    <v-expand-transition>
      <v-alert
        v-if="authStore.error"
        type="error"
        variant="tonal"
        density="comfortable"
        class="mb-5"
        rounded="lg"
        closable
        @click:close="authStore.clearError()"
      >
        {{ authStore.error }}
      </v-alert>
    </v-expand-transition>

    <v-form ref="formRef" validate-on="submit" @submit.prevent="handleSetup">
      <div class="text-overline text-medium-emphasis mb-2">Administrador</div>

      <v-text-field
        v-model="name"
        label="Nome"
        autocomplete="name"
        autofocus
        prepend-inner-icon="mdi-account-outline"
        variant="solo-filled"
        density="comfortable"
        flat
        rounded="lg"
        class="mb-3"
        hide-details="auto"
        :rules="nameRules"
        @update:model-value="authStore.clearError()"
      ></v-text-field>

      <v-text-field
        v-model="email"
        label="E-mail"
        type="email"
        autocomplete="username"
        prepend-inner-icon="mdi-email-outline"
        variant="solo-filled"
        density="comfortable"
        flat
        rounded="lg"
        class="mb-3"
        hide-details="auto"
        :rules="emailRules"
        @update:model-value="authStore.clearError()"
      ></v-text-field>

      <v-text-field
        v-model="password"
        label="Senha"
        :type="showPassword ? 'text' : 'password'"
        autocomplete="new-password"
        prepend-inner-icon="mdi-lock-outline"
        :append-inner-icon="showPassword ? 'mdi-eye-off-outline' : 'mdi-eye-outline'"
        variant="solo-filled"
        density="comfortable"
        flat
        rounded="lg"
        hide-details="auto"
        :rules="passwordRules"
        @click:append-inner="showPassword = !showPassword"
        @update:model-value="authStore.clearError()"
      ></v-text-field>

      <div class="strength mt-2 mb-3">
        <!-- Barra cheia e cinza, com o campo vazio, parecia um divisor no meio
             do formulário; zerada, ela só aparece quando há o que medir. -->
        <v-progress-linear
          :model-value="strength.score * 25"
          :color="strength.color"
          :bg-opacity="password ? 0.18 : 0"
          height="3"
          rounded
        ></v-progress-linear>
        <div class="d-flex justify-space-between align-center mt-1">
          <span class="text-caption text-medium-emphasis">Mínimo de 8 caracteres</span>
          <span
            v-if="strength.label"
            class="text-caption font-weight-medium"
            :class="`text-${strength.color}`"
          >
            {{ strength.label }}
          </span>
        </div>
      </div>

      <v-text-field
        v-model="passwordConfirmation"
        label="Confirme a senha"
        :type="showPassword ? 'text' : 'password'"
        autocomplete="new-password"
        prepend-inner-icon="mdi-lock-check-outline"
        variant="solo-filled"
        density="comfortable"
        flat
        rounded="lg"
        class="mb-6"
        hide-details="auto"
        :rules="passwordConfirmationRules"
        @update:model-value="authStore.clearError()"
      ></v-text-field>

      <v-divider class="mb-5"></v-divider>

      <div class="d-flex align-center justify-space-between mb-2">
        <span class="text-overline text-medium-emphasis">Token de instalação</span>
        <v-btn
          variant="text"
          size="small"
          density="comfortable"
          class="text-none"
          :append-icon="showHint ? 'mdi-chevron-up' : 'mdi-chevron-down'"
          @click="showHint = !showHint"
        >
          Onde encontrar
        </v-btn>
      </div>

      <v-expand-transition>
        <v-sheet v-show="showHint" class="token-hint mb-4 pa-3" rounded="lg">
          <p class="text-caption mb-2">
            O servidor imprime o token num quadro destacado ao iniciar. Rode no host:
          </p>

          <div v-for="cmd in commands" :key="cmd" class="command mb-2">
            <code class="command__text">{{ cmd }}</code>
            <v-tooltip :text="copied === cmd ? 'Copiado' : 'Copiar'" location="top">
              <template #activator="{ props: tip }">
                <v-btn
                  v-bind="tip"
                  :icon="copied === cmd ? 'mdi-check' : 'mdi-content-copy'"
                  :color="copied === cmd ? 'success' : undefined"
                  size="x-small"
                  variant="text"
                  :aria-label="`Copiar comando: ${cmd}`"
                  @click="copy(cmd)"
                ></v-btn>
              </template>
            </v-tooltip>
          </div>

          <p class="text-caption text-medium-emphasis mb-0">
            Definiu <code>SETUP_TOKEN</code> no ambiente? Use aquele valor.
          </p>
        </v-sheet>
      </v-expand-transition>

      <v-text-field
        v-model="token"
        label="Token"
        autocomplete="off"
        spellcheck="false"
        prepend-inner-icon="mdi-key-variant"
        variant="solo-filled"
        density="comfortable"
        flat
        rounded="lg"
        class="mb-6 token-field"
        hide-details="auto"
        :rules="tokenRules"
        @update:model-value="authStore.clearError()"
      ></v-text-field>

      <v-btn
        type="submit"
        color="primary"
        block
        size="large"
        variant="flat"
        rounded="lg"
        class="text-none font-weight-bold"
        :loading="authStore.loading"
        append-icon="mdi-arrow-right"
      >
        Criar administrador e entrar
      </v-btn>
    </v-form>
  </AuthShell>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import AuthShell from '@/components/auth/AuthShell.vue'
import { useAuthStore } from '@/stores/auth'
import {
  emailRule,
  matchesRule,
  minLengthRule,
  passwordRule,
  passwordStrength,
  requiredRule,
  MIN_NAME_LENGTH,
  type ValidationRule,
  type VuetifyForm,
} from '@/utils/formRules'

const authStore = useAuthStore()
const router = useRouter()

const formRef = ref<VuetifyForm | null>(null)
const name = ref('')
const email = ref('')
const password = ref('')
const passwordConfirmation = ref('')
const token = ref('')
const showPassword = ref(false)
const copied = ref<string | null>(null)

/**
 * A ajuda começa **aberta**.
 *
 * Quem chega nesta tela quase nunca sabe o que é o token — é literalmente a
 * primeira coisa que vê do sistema. Escondê-la atrás de um clique economizaria
 * altura à custa da única pergunta que a tela precisa responder. O botão existe
 * para fechar depois, não para descobrir.
 */
const showHint = ref(true)

const commands = [
  'docker compose logs server',
  'docker compose exec server backend-cli task auth_setup_token',
]

const strength = computed(() => passwordStrength(password.value))

const nameRules: ValidationRule[] = [
  minLengthRule(MIN_NAME_LENGTH, `O nome precisa ter ao menos ${MIN_NAME_LENGTH} caracteres.`),
]
const emailRules: ValidationRule[] = [emailRule()]
const passwordRules: ValidationRule[] = [passwordRule()]
const passwordConfirmationRules: ValidationRule[] = [
  matchesRule(() => password.value, 'As senhas não conferem.'),
]
const tokenRules: ValidationRule[] = [requiredRule('Informe o token de instalação.')]

/**
 * Copia o comando para a área de transferência.
 *
 * `navigator.clipboard` só existe em contexto seguro (https ou localhost) — e
 * uma instalação nova costuma ser acessada por IP em http puro, que é
 * justamente o caso em que a API não está lá. O silêncio no `catch` é
 * proposital: o comando continua visível e selecionável, então o operador não
 * fica sem saída, e um erro na tela só assustaria à toa.
 */
async function copy(value: string) {
  try {
    await navigator.clipboard.writeText(value)
    copied.value = value
    setTimeout(() => {
      if (copied.value === value) copied.value = null
    }, 2000)
  } catch {
    // Sem clipboard: o texto segue à mão para seleção manual.
  }
}

async function handleSetup() {
  const { valid } = (await formRef.value?.validate()) ?? { valid: false }
  if (!valid) return

  const created = await authStore.completeSetup({
    name: name.value.trim(),
    email: email.value.trim(),
    password: password.value,
    token: token.value.trim(),
  })

  if (created) await router.push({ name: 'dashboard' })
}
</script>

<style scoped>
.token-hint {
  background: rgba(33, 150, 243, 0.07);
  border: 1px solid rgba(33, 150, 243, 0.22);
}

.command {
  display: flex;
  align-items: center;
  gap: 8px;
  background: rgba(0, 0, 0, 0.32);
  border-radius: 8px;
  padding: 6px 6px 6px 12px;
}

/* O comando é longo e não pode empurrar o card: quebra dentro da própria
   caixa, com o botão de copiar preso à direita. */
.command__text {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 0.75rem;
  line-height: 1.5;
  word-break: break-all;
  color: rgba(255, 255, 255, 0.82);
}

/* Token é uma cadeia sem sentido de 32 caracteres: em fonte proporcional,
   conferir se o `l` colado no `1` está certo vira adivinhação. */
.token-field :deep(input) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  letter-spacing: 0.02em;
}
</style>

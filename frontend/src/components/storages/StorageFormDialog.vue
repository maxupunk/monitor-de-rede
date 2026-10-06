<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 720"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center pt-4 px-6">
        <v-icon v-if="selected" start :color="selected.color">{{ selected.icon }}</v-icon>
        {{ title }}
      </v-card-title>
      <v-card-subtitle class="px-6 pb-2 text-wrap">
        {{
          step === 'provider'
            ? 'Escolha onde as cópias de segurança das configurações vão ficar.'
            : selected?.hint
        }}
      </v-card-subtitle>

      <v-card-text class="px-6">
        <!-- Passo 1: onde guardar -->
        <v-row v-if="step === 'provider'" dense>
          <v-col v-for="item in STORAGE_PROVIDERS" :key="item.value" cols="12" sm="6">
            <v-card
              variant="outlined"
              class="provider-card pa-3 d-flex align-center ga-3 h-100"
              :class="{ 'provider-card--active': provider === item.value }"
              @click="chooseProvider(item.value)"
            >
              <v-avatar :color="item.color" variant="tonal" rounded="lg" size="44">
                <v-icon size="26">{{ item.icon }}</v-icon>
              </v-avatar>
              <div class="min-w-0">
                <div class="font-weight-bold">{{ item.label }}</div>
                <div class="text-caption text-high-emphasis">{{ item.hint }}</div>
              </div>
            </v-card>
          </v-col>
        </v-row>

        <!-- Passo 2: conexão e política -->
        <v-form v-else ref="formRef" autocomplete="off" @submit.prevent="save">
          <v-text-field
            v-model="form.name"
            label="Nome *"
            placeholder="Ex.: NAS do escritório"
            variant="outlined"
            :rules="[required('Informe um nome')]"
            class="mb-2"
          ></v-text-field>

          <div class="section-title">Conexão</div>

          <template v-if="configType === 'local'">
            <v-text-field
              v-model="form.basePath"
              label="Subpasta"
              placeholder="copias"
              variant="outlined"
              :prefix="localRootPrefix"
              hint="Deixe em branco para usar a raiz. Para gravar num disco externo ou NAS, monte-o dentro desta pasta no servidor."
              persistent-hint
              class="mb-4"
            ></v-text-field>
          </template>

          <template v-else-if="configType === 's3'">
            <v-text-field
              v-if="provider !== 'aws_s3'"
              v-model="form.endpoint"
              label="Endpoint *"
              :placeholder="endpointPlaceholder"
              variant="outlined"
              :rules="[required('Informe o endpoint')]"
              class="mb-2"
            ></v-text-field>
            <v-row dense>
              <v-col cols="12" sm="7">
                <v-text-field
                  v-model="form.bucket"
                  label="Bucket *"
                  variant="outlined"
                  :rules="[required('Informe o bucket')]"
                ></v-text-field>
              </v-col>
              <v-col v-if="provider !== 'cloudflare_r2'" cols="12" sm="5">
                <v-text-field
                  v-model="form.region"
                  label="Região"
                  :placeholder="provider === 'aws_s3' ? 'sa-east-1' : 'us-east-1'"
                  variant="outlined"
                ></v-text-field>
              </v-col>
            </v-row>
            <v-text-field
              v-model="form.accessKeyId"
              label="Access Key ID *"
              variant="outlined"
              autocomplete="off"
              :rules="[required('Informe o Access Key ID')]"
              class="mb-2"
            ></v-text-field>
            <v-text-field
              v-model="form.secretAccessKey"
              label="Secret Access Key"
              variant="outlined"
              v-bind="secretField('secretAccessKey')"
              class="mb-2"
            ></v-text-field>
            <v-text-field
              v-model="form.prefix"
              label="Pasta dentro do bucket"
              placeholder="netmonitor"
              variant="outlined"
              hint="Opcional. Útil para dividir um bucket entre vários sistemas."
              persistent-hint
              class="mb-4"
            ></v-text-field>
          </template>

          <template v-else-if="configType === 'gcs'">
            <v-text-field
              v-model="form.bucket"
              label="Bucket *"
              variant="outlined"
              :rules="[required('Informe o bucket')]"
              class="mb-2"
            ></v-text-field>
            <v-textarea
              v-model="form.credentialsJson"
              label="JSON da conta de serviço"
              variant="outlined"
              rows="4"
              class="mono mb-2"
              v-bind="secretHint('credentialsJson')"
            ></v-textarea>
            <v-text-field
              v-model="form.prefix"
              label="Pasta dentro do bucket"
              placeholder="netmonitor"
              variant="outlined"
              class="mb-4"
            ></v-text-field>
          </template>

          <template v-else-if="configType === 'azure_blob'">
            <v-text-field
              v-model="form.connectionString"
              label="Connection string"
              variant="outlined"
              v-bind="secretField('connectionString')"
              class="mb-2"
            ></v-text-field>
            <v-text-field
              v-model="form.container"
              label="Container *"
              variant="outlined"
              :rules="[required('Informe o container')]"
              class="mb-2"
            ></v-text-field>
            <v-text-field
              v-model="form.prefix"
              label="Pasta dentro do container"
              placeholder="netmonitor"
              variant="outlined"
              class="mb-4"
            ></v-text-field>
          </template>

          <template v-else-if="configType === 'sftp'">
            <v-row dense>
              <v-col cols="8" sm="9">
                <v-text-field
                  v-model="form.host"
                  label="Servidor *"
                  placeholder="nas.local ou 192.168.0.20"
                  variant="outlined"
                  :rules="[required('Informe o servidor')]"
                ></v-text-field>
              </v-col>
              <v-col cols="4" sm="3">
                <v-text-field
                  v-model.number="form.port"
                  label="Porta"
                  type="number"
                  variant="outlined"
                ></v-text-field>
              </v-col>
            </v-row>
            <v-text-field
              v-model="form.username"
              label="Usuário *"
              variant="outlined"
              autocomplete="off"
              :rules="[required('Informe o usuário')]"
              class="mb-2"
            ></v-text-field>
            <v-btn-toggle
              v-model="form.sftpAuth"
              color="primary"
              variant="outlined"
              divided
              mandatory
              density="comfortable"
              class="mb-4"
            >
              <v-btn value="password" prepend-icon="mdi-form-textbox-password">Senha</v-btn>
              <v-btn value="key" prepend-icon="mdi-key-variant">Chave privada</v-btn>
            </v-btn-toggle>
            <v-text-field
              v-if="form.sftpAuth === 'password'"
              v-model="form.password"
              label="Senha"
              variant="outlined"
              v-bind="secretField('password')"
              class="mb-2"
            ></v-text-field>
            <template v-else>
              <v-textarea
                v-model="form.privateKey"
                label="Chave privada (OpenSSH ou PEM)"
                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                variant="outlined"
                rows="4"
                class="mono mb-2"
                v-bind="secretHint('privateKey')"
              ></v-textarea>
              <v-text-field
                v-model="form.passphrase"
                label="Passphrase da chave"
                variant="outlined"
                v-bind="secretField('passphrase', false)"
                class="mb-2"
              ></v-text-field>
            </template>
            <v-text-field
              v-model="form.basePath"
              label="Pasta no servidor"
              placeholder="/srv/backups/netmonitor"
              variant="outlined"
              class="mb-2"
            ></v-text-field>
            <v-alert
              v-if="form.hostKeyFingerprint"
              type="info"
              variant="tonal"
              density="compact"
              icon="mdi-fingerprint"
              class="mb-4"
            >
              <div class="text-body-2">
                Identidade do servidor memorizada:
                <code class="text-break">{{ form.hostKeyFingerprint }}</code>
              </div>
              <div class="text-caption">
                Uma chave diferente é recusada. Se o servidor foi reinstalado, aceite a nova
                identidade.
              </div>
              <template #append>
                <v-btn size="small" variant="tonal" color="info" @click="forgetHostKey">
                  Aceitar nova
                </v-btn>
              </template>
            </v-alert>
          </template>

          <div class="d-flex flex-wrap align-center ga-3 mb-2">
            <v-btn
              color="info"
              variant="tonal"
              prepend-icon="mdi-connection"
              :loading="testing"
              @click="runTest"
            >
              Testar conexão
            </v-btn>
            <span v-if="!testResult" class="text-caption text-high-emphasis">
              O teste também roda ao salvar.
            </span>
          </div>
          <v-alert
            v-if="testResult"
            :type="testResult.ok ? 'success' : 'error'"
            variant="tonal"
            density="compact"
            class="mb-4"
          >
            <div class="text-body-2">{{ testResult.message }}</div>
            <div
              v-if="testResult.ok && testResult.hostKeyFingerprint && !form.hostKeyFingerprint"
              class="text-caption mt-1"
            >
              Servidor identificado por <code>{{ testResult.hostKeyFingerprint }}</code> — essa
              identidade será memorizada no primeiro uso.
            </div>
          </v-alert>

          <div class="section-title mt-6">Backup automático</div>
          <v-switch
            v-model="form.backupEnabled"
            color="success"
            inset
            hide-details
            label="Enviar cópias das configurações automaticamente"
            class="mb-2"
          ></v-switch>
          <v-row dense>
            <v-col cols="12" sm="6">
              <v-select
                v-model="form.backupIntervalHours"
                :items="intervalItems"
                label="Frequência"
                variant="outlined"
                :disabled="!form.backupEnabled"
              ></v-select>
            </v-col>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model.number="form.backupRetention"
                label="Manter as últimas"
                type="number"
                suffix="cópias"
                variant="outlined"
                min="1"
                max="365"
                :rules="[retentionRule]"
              ></v-text-field>
            </v-col>
          </v-row>
          <div class="text-caption text-high-emphasis">
            As cópias mais antigas que isso são apagadas sozinhas — só os arquivos de backup que o
            sistema criou, na pasta <code>netmonitor-backups</code>. O arquivo traz a community SNMP
            dos equipamentos: prefira um destino privado.
          </div>
        </v-form>

        <v-alert v-if="saveError" type="error" variant="tonal" density="compact" class="mt-4">
          {{ saveError }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-btn
          v-if="step === 'form' && !storageId"
          variant="text"
          prepend-icon="mdi-arrow-left"
          @click="step = 'provider'"
        >
          Trocar tipo
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-btn
          v-if="step === 'form'"
          :color="forceSave ? 'warning' : 'primary'"
          variant="flat"
          :loading="saving"
          @click="save"
        >
          {{ forceSave ? 'Salvar mesmo assim' : storageId ? 'Salvar' : 'Criar' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import type { StorageConfig } from '@/bindings/StorageConfig'
import type { StorageDestinationDetail } from '@/bindings/StorageDestinationDetail'
import type { StorageDestinationInput } from '@/bindings/StorageDestinationInput'
import type { StorageProvider } from '@/bindings/StorageProvider'
import type { StorageTestResponse } from '@/bindings/StorageTestResponse'
import { useStoragesStore } from '@/stores/storages'
import {
  BACKUP_INTERVALS,
  intervalLabel,
  providerInfo,
  SECRET_LABELS,
  STORAGE_PROVIDERS,
} from '@/utils/storagePresentation'
import { requiredRule } from '@/utils/formRules'

const props = defineProps<{
  modelValue: boolean
  /** `null` cria um armazenamento novo. */
  storageId: number | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', value: StorageDestinationDetail, created: boolean): void
}>()

const storagesStore = useStoragesStore()

const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const step = ref<'provider' | 'form'>('provider')
const provider = ref<StorageProvider>('local')
const saving = ref(false)
const testing = ref(false)
const testResult = ref<StorageTestResponse | null>(null)
/** Config testada por último — mudou depois disso, o teste não vale mais. */
const testedSignature = ref<string | null>(null)
const forceSave = ref(false)
const saveError = ref<string | null>(null)
const secretsSet = ref<string[]>([])
const visibleSecrets = ref<string[]>([])

const form = reactive({
  name: '',
  basePath: '',
  endpoint: '',
  bucket: '',
  region: '',
  accessKeyId: '',
  secretAccessKey: '',
  prefix: '',
  credentialsJson: '',
  connectionString: '',
  container: '',
  host: '',
  port: 22 as number | string,
  username: '',
  sftpAuth: 'password' as 'password' | 'key',
  password: '',
  privateKey: '',
  passphrase: '',
  hostKeyFingerprint: '',
  backupEnabled: true,
  backupIntervalHours: 24,
  backupRetention: 14,
})

const selected = computed(() => (step.value === 'form' ? providerInfo(provider.value) : null))
const configType = computed(() => providerInfo(provider.value).configType)
const title = computed(() => {
  if (props.storageId) return 'Editar armazenamento'
  return step.value === 'provider'
    ? 'Novo armazenamento'
    : `Novo armazenamento — ${selected.value?.label}`
})
const localRootPrefix = computed(() => {
  const root = storagesStore.meta?.localRoot ?? 'storage'
  return `${root.replace(/[\\/]+$/, '')}/`
})
const endpointPlaceholder = computed(() => {
  switch (provider.value) {
    case 'cloudflare_r2':
      return 'https://<account-id>.r2.cloudflarestorage.com'
    case 'minio':
      return 'http://minio.local:9000'
    default:
      return 'https://s3.us-west-002.backblazeb2.com'
  }
})
const intervalItems = computed(() => {
  const items = [...BACKUP_INTERVALS]
  // Frequência gravada fora dos presets (pela API) continua selecionável.
  if (!items.some((item) => item.value === form.backupIntervalHours)) {
    items.push({ value: form.backupIntervalHours, title: intervalLabel(form.backupIntervalHours) })
  }
  return items
})

function required(message: string) {
  return requiredRule(message)
}

function retentionRule(value: number | string): true | string {
  const n = Number(value)
  return (Number.isInteger(n) && n >= 1 && n <= 365) || 'Entre 1 e 365 cópias'
}

/**
 * Campo de segredo: mascarado, com o olho para conferir o que se digitou, e
 * sem autopreenchimento — o navegador não pode oferecer a senha do login aqui.
 * Na edição, o segredo gravado nunca vem: o campo vazio mantém o atual.
 */
function secretField(name: string, requiredOnCreate = true) {
  const visible = visibleSecrets.value.includes(name)
  return {
    type: visible ? 'text' : 'password',
    autocomplete: 'new-password',
    'append-inner-icon': visible ? 'mdi-eye-off' : 'mdi-eye',
    'onClick:appendInner': () => toggleSecret(name),
    rules: requiredOnCreate && !secretsSet.value.includes(name) ? [required('Obrigatório')] : [],
    ...secretHint(name),
  }
}

function secretHint(name: string) {
  return secretsSet.value.includes(name)
    ? {
        placeholder: '••••••••',
        hint: `${SECRET_LABELS[name] ?? 'Valor'} gravado — deixe em branco para manter.`,
        'persistent-hint': true,
      }
    : {}
}

function toggleSecret(name: string) {
  visibleSecrets.value = visibleSecrets.value.includes(name)
    ? visibleSecrets.value.filter((item) => item !== name)
    : [...visibleSecrets.value, name]
}

function trimmed(value: string): string | undefined {
  const text = value.trim()
  return text === '' ? undefined : text
}

function buildConfig(): StorageConfig {
  switch (configType.value) {
    case 'local':
      return { type: 'local', basePath: trimmed(form.basePath) }
    case 's3':
      return {
        type: 's3',
        bucket: form.bucket.trim(),
        region: provider.value === 'cloudflare_r2' ? undefined : trimmed(form.region),
        endpoint: provider.value === 'aws_s3' ? undefined : trimmed(form.endpoint),
        accessKeyId: form.accessKeyId.trim(),
        secretAccessKey: form.secretAccessKey || undefined,
        prefix: trimmed(form.prefix),
      }
    case 'gcs':
      return {
        type: 'gcs',
        bucket: form.bucket.trim(),
        credentialsJson: trimmed(form.credentialsJson),
        prefix: trimmed(form.prefix),
      }
    case 'azure_blob':
      return {
        type: 'azure_blob',
        container: form.container.trim(),
        connectionString: trimmed(form.connectionString),
        prefix: trimmed(form.prefix),
      }
    case 'sftp':
      return {
        type: 'sftp',
        host: form.host.trim(),
        port: Number(form.port) || 22,
        username: form.username.trim(),
        password: form.sftpAuth === 'password' ? form.password || undefined : undefined,
        privateKey: form.sftpAuth === 'key' ? trimmed(form.privateKey) : undefined,
        passphrase: form.sftpAuth === 'key' ? form.passphrase || undefined : undefined,
        basePath: trimmed(form.basePath),
        hostKeyFingerprint: trimmed(form.hostKeyFingerprint),
      }
  }
}

function buildInput(): StorageDestinationInput {
  return {
    name: form.name.trim(),
    provider: provider.value,
    config: buildConfig(),
    backupEnabled: form.backupEnabled,
    backupIntervalHours: form.backupIntervalHours,
    backupRetention: Number(form.backupRetention),
  }
}

function signature(): string {
  return JSON.stringify({ provider: provider.value, config: buildConfig() })
}

// Mudou a conexão depois do teste: o resultado não vale mais.
watch(
  () => signature(),
  (current) => {
    if (testedSignature.value && current !== testedSignature.value) {
      testResult.value = null
      testedSignature.value = null
      forceSave.value = false
    }
  }
)

function resetForm() {
  Object.assign(form, {
    name: '',
    basePath: '',
    endpoint: '',
    bucket: '',
    region: '',
    accessKeyId: '',
    secretAccessKey: '',
    prefix: '',
    credentialsJson: '',
    connectionString: '',
    container: '',
    host: '',
    port: 22,
    username: '',
    sftpAuth: 'password',
    password: '',
    privateKey: '',
    passphrase: '',
    hostKeyFingerprint: '',
    backupEnabled: true,
    backupIntervalHours: 24,
    backupRetention: 14,
  })
  secretsSet.value = []
  visibleSecrets.value = []
  testResult.value = null
  testedSignature.value = null
  forceSave.value = false
  saveError.value = null
}

function fillFrom(detail: StorageDestinationDetail) {
  provider.value = detail.provider
  form.name = detail.name
  form.backupEnabled = detail.backupEnabled
  form.backupIntervalHours = detail.backupIntervalHours
  form.backupRetention = detail.backupRetention
  secretsSet.value = detail.secretsSet
  const config = detail.config
  switch (config.type) {
    case 'local':
      form.basePath = config.basePath ?? ''
      break
    case 's3':
      form.bucket = config.bucket
      form.region = config.region ?? ''
      form.endpoint = config.endpoint ?? ''
      form.accessKeyId = config.accessKeyId
      form.prefix = config.prefix ?? ''
      break
    case 'gcs':
      form.bucket = config.bucket
      form.prefix = config.prefix ?? ''
      break
    case 'azure_blob':
      form.container = config.container
      form.prefix = config.prefix ?? ''
      break
    case 'sftp':
      form.host = config.host
      form.port = config.port ?? 22
      form.username = config.username
      form.basePath = config.basePath ?? ''
      form.hostKeyFingerprint = config.hostKeyFingerprint ?? ''
      form.sftpAuth = detail.secretsSet.includes('privateKey') ? 'key' : 'password'
      break
  }
}

watch(
  () => props.modelValue,
  async (open) => {
    if (!open) return
    resetForm()
    void storagesStore.fetchMeta()
    if (props.storageId == null) {
      step.value = 'provider'
      return
    }
    step.value = 'form'
    try {
      fillFrom(await storagesStore.fetchDetail(props.storageId))
    } catch (err) {
      saveError.value = err instanceof Error ? err.message : 'Erro ao carregar o armazenamento'
    }
  }
)

function chooseProvider(value: StorageProvider) {
  const previousLabel = providerInfo(provider.value).label
  provider.value = value
  // Sugere o nome do tipo, sem atropelar o que o operador já digitou.
  if (!form.name || form.name === previousLabel) form.name = providerInfo(value).label
  step.value = 'form'
}

function forgetHostKey() {
  form.hostKeyFingerprint = ''
}

async function runTest(): Promise<StorageTestResponse | null> {
  testing.value = true
  saveError.value = null
  try {
    const current = signature()
    testResult.value = await storagesStore.testDraft({
      id: props.storageId,
      provider: provider.value,
      config: buildConfig(),
    })
    testedSignature.value = current
    return testResult.value
  } catch (err) {
    saveError.value = err instanceof Error ? err.message : 'Erro ao testar a conexão'
    return null
  } finally {
    testing.value = false
  }
}

/**
 * Salvar testa antes, se a config mudou desde o último teste. Falhou: o botão
 * vira "Salvar mesmo assim" — dá para cadastrar um NAS que está desligado
 * agora, mas não sem saber.
 */
async function save() {
  const validation = await formRef.value?.validate()
  if (validation && !validation.valid) return

  if (!forceSave.value) {
    const fresh = testResult.value && testedSignature.value === signature()
    const result = fresh ? testResult.value : await runTest()
    if (!result) return
    if (!result.ok) {
      forceSave.value = true
      return
    }
    // A identidade que o teste acabou de ver é a que fica memorizada.
    if (configType.value === 'sftp' && !form.hostKeyFingerprint && result.hostKeyFingerprint) {
      form.hostKeyFingerprint = result.hostKeyFingerprint
    }
  }

  saving.value = true
  saveError.value = null
  try {
    const created = props.storageId == null
    const saved = await storagesStore.save(props.storageId, buildInput())
    emit('saved', saved, created)
    close()
  } catch (err) {
    saveError.value = err instanceof Error ? err.message : 'Erro ao salvar o armazenamento'
  } finally {
    saving.value = false
  }
}

function close() {
  emit('update:modelValue', false)
}
</script>

<style scoped>
.provider-card {
  cursor: pointer;
  border-width: 1px;
  transition:
    border-color 0.15s,
    box-shadow 0.15s;
}
.provider-card:hover {
  border-color: rgb(var(--v-theme-primary));
}
.provider-card--active {
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 0 0 1px rgb(var(--v-theme-primary));
}
.section-title {
  font-weight: 700;
  font-size: 0.875rem;
  margin: 8px 0 12px;
}
.mono :deep(textarea) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.8rem;
}
.min-w-0 {
  min-width: 0;
}
</style>

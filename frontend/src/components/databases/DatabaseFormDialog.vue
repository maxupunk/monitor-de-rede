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
        <v-icon v-if="step === 'form'" start :color="engine.color">{{ engine.icon }}</v-icon>
        {{ title }}
      </v-card-title>
      <v-card-subtitle class="px-6 pb-2 text-wrap">
        {{
          step === 'engine'
            ? 'Qual banco de dados você quer proteger?'
            : 'O backup roda direto pelo protocolo do banco — não precisa instalar nada no servidor.'
        }}
      </v-card-subtitle>

      <v-card-text class="px-6">
        <!-- Passo 1: qual SGBD -->
        <v-row v-if="step === 'engine'" dense>
          <v-col v-for="item in DATABASE_ENGINES" :key="item.value" cols="12" sm="4">
            <v-card
              variant="outlined"
              class="engine-card pa-4 text-center h-100"
              @click="chooseEngine(item.value)"
            >
              <v-avatar :color="item.color" variant="tonal" rounded="lg" size="52" class="mb-2">
                <v-icon size="30">{{ item.icon }}</v-icon>
              </v-avatar>
              <div class="font-weight-bold">{{ item.label }}</div>
              <div class="text-caption text-high-emphasis">{{ item.hint }}</div>
            </v-card>
          </v-col>
        </v-row>

        <!-- Passo 2: conexão, bancos, destino e agenda -->
        <v-form v-else ref="formRef" autocomplete="off" @submit.prevent="save">
          <v-text-field
            v-model="form.name"
            label="Nome *"
            placeholder="Ex.: ERP produção"
            variant="outlined"
            :rules="[requiredRule('Informe um nome')]"
            class="mb-2"
          ></v-text-field>

          <div class="section-title">Conexão</div>
          <v-row dense>
            <v-col cols="8" sm="9">
              <v-text-field
                v-model="form.host"
                label="Servidor *"
                placeholder="10.0.0.20 ou db.empresa.local"
                variant="outlined"
                :rules="[requiredRule('Informe o servidor')]"
                v-bind="loopbackFieldHint(form.host)"
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
          <v-row dense>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="form.username"
                label="Usuário *"
                variant="outlined"
                autocomplete="off"
                :rules="[requiredRule('Informe o usuário')]"
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model="form.password"
                label="Senha"
                variant="outlined"
                v-bind="secrets.field('password', false)"
              ></v-text-field>
            </v-col>
          </v-row>
          <v-select
            v-model="form.sslMode"
            :items="SSL_MODES"
            item-title="title"
            item-value="value"
            label="TLS (criptografia da conexão)"
            variant="outlined"
            class="mb-2"
          >
            <template #item="{ props: itemProps, item }">
              <v-list-item v-bind="itemProps" :subtitle="item.subtitle"></v-list-item>
            </template>
          </v-select>

          <div class="d-flex flex-wrap align-center ga-3 mb-2">
            <v-btn
              color="info"
              variant="tonal"
              prepend-icon="mdi-database-search-outline"
              :loading="verified.testing.value"
              @click="verified.run"
            >
              Testar e listar bancos
            </v-btn>
            <span v-if="!probe" class="text-caption text-high-emphasis">
              O teste também roda ao salvar.
            </span>
          </div>
          <v-alert v-if="probe" type="success" variant="tonal" density="compact" class="mb-4">
            Conectado a <strong>{{ probe.version }}</strong> em {{ probe.latencyMs }} ms —
            {{ probe.databases.length }} {{ probe.databases.length === 1 ? 'banco' : 'bancos' }}
            visíveis para este usuário.
          </v-alert>
          <v-alert
            v-if="verified.error.value"
            type="error"
            variant="tonal"
            density="compact"
            class="mb-4"
          >
            {{ verified.error.value }}
          </v-alert>

          <div class="section-title mt-4">O que copiar</div>
          <v-radio-group v-model="scope" hide-details class="mb-2">
            <v-radio
              value="all"
              color="primary"
              label="Todos os bancos — inclusive os criados depois"
            ></v-radio>
            <v-radio value="some" color="primary" label="Só os bancos que eu escolher"></v-radio>
          </v-radio-group>
          <div v-if="scope === 'some'" class="mb-2">
            <v-chip-group v-if="databaseOptions.length" v-model="form.databases" multiple column>
              <v-chip
                v-for="name in databaseOptions"
                :key="name"
                :value="name"
                filter
                variant="outlined"
                color="primary"
              >
                {{ name }}
              </v-chip>
            </v-chip-group>
            <div v-else class="text-body-2 text-high-emphasis">
              Clique em "Testar e listar bancos" para escolher.
            </div>
            <div
              v-if="scope === 'some' && databaseOptions.length && form.databases.length === 0"
              class="text-caption text-high-emphasis"
            >
              Marque os bancos que entram no backup.
            </div>
          </div>

          <div class="section-title mt-4">Para onde vão as cópias</div>
          <DestinationSelect
            v-model="form.storageDestinationId"
            clearable
            hint="As cópias ficam na pasta database-backups do destino."
            class="mb-2"
          />

          <div class="section-title mt-4">Quando</div>
          <BackupPolicyFields
            v-model:enabled="form.backupEnabled"
            v-model:interval-hours="form.backupIntervalHours"
            v-model:retention="form.backupRetention"
            :disabled="!form.storageDestinationId"
            :label="
              form.storageDestinationId
                ? 'Fazer backup automaticamente'
                : 'Escolha um destino para ligar o backup automático'
            "
          >
            Mantém as últimas cópias de cada banco; as mais antigas são apagadas do destino. Cópia
            consistente sem travar o banco: PostgreSQL numa transação de leitura, MySQL e MariaDB
            com <code>CONSISTENT SNAPSHOT</code> (tabelas InnoDB).
          </BackupPolicyFields>
        </v-form>

        <v-alert v-if="saveError" type="error" variant="tonal" density="compact" class="mt-4">
          {{ saveError }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-btn
          v-if="step === 'form' && !connectionId"
          variant="text"
          prepend-icon="mdi-arrow-left"
          @click="step = 'engine'"
        >
          Trocar banco
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-btn
          v-if="step === 'form'"
          :color="verified.forceSave.value ? 'warning' : 'primary'"
          variant="flat"
          :loading="saving"
          @click="save"
        >
          {{ verified.forceSave.value ? 'Salvar mesmo assim' : connectionId ? 'Salvar' : 'Criar' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import type { DatabaseConnectionInput } from '@/bindings/DatabaseConnectionInput'
import type { DatabaseConnectionResponse } from '@/bindings/DatabaseConnectionResponse'
import type { DatabaseEngine } from '@/bindings/DatabaseEngine'
import type { DatabaseProbeResponse } from '@/bindings/DatabaseProbeResponse'
import type { SslMode } from '@/bindings/SslMode'
import BackupPolicyFields from '@/components/backup/BackupPolicyFields.vue'
import DestinationSelect from '@/components/backup/DestinationSelect.vue'
import { useSecretFields } from '@/composables/useSecretFields'
import { useVerifiedSave } from '@/composables/useVerifiedSave'
import { useDatabasesStore } from '@/stores/databases'
import { useStoragesStore } from '@/stores/storages'
import { DATABASE_ENGINES, engineInfo, SSL_MODES } from '@/utils/databasePresentation'
import { requiredRule } from '@/utils/formRules'
import { loopbackFieldHint } from '@/utils/hostHints'

const props = defineProps<{
  modelValue: boolean
  /** `null` cria uma conexão nova. */
  connectionId: number | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', value: DatabaseConnectionResponse, created: boolean): void
}>()

const databasesStore = useDatabasesStore()
const storagesStore = useStoragesStore()
const secrets = useSecretFields({ password: 'Senha' })

const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const step = ref<'engine' | 'form'>('engine')
const scope = ref<'all' | 'some'>('all')
const saving = ref(false)
const saveError = ref<string | null>(null)

const form = reactive({
  name: '',
  engine: 'postgres' as DatabaseEngine,
  host: '',
  port: 5432 as number | string,
  username: '',
  password: '',
  sslMode: 'prefer' as SslMode,
  databases: [] as string[],
  storageDestinationId: null as number | null,
  backupEnabled: true,
  backupIntervalHours: 24,
  backupRetention: 7,
})

const engine = computed(() => engineInfo(form.engine))
const title = computed(() => {
  if (props.connectionId) return 'Editar conexão de banco'
  return step.value === 'engine' ? 'Nova conexão de banco' : `Nova conexão — ${engine.value.label}`
})

function signature(): string {
  return JSON.stringify({
    engine: form.engine,
    host: form.host.trim(),
    port: Number(form.port),
    username: form.username.trim(),
    password: form.password,
    sslMode: form.sslMode,
  })
}

const verified = useVerifiedSave<DatabaseProbeResponse>({
  signature,
  test: () =>
    databasesStore.probe({
      id: props.connectionId,
      engine: form.engine,
      host: form.host.trim(),
      port: Number(form.port) || engine.value.defaultPort,
      username: form.username.trim(),
      password: form.password || undefined,
      sslMode: form.sslMode,
    }),
  succeeded: () => true,
})
const probe = verified.result

/** O que dá para marcar: o que o servidor listou mais o que já estava escolhido. */
const databaseOptions = computed(() => {
  const listed = probe.value?.databases ?? []
  return [...new Set([...listed, ...form.databases])].sort((a, b) => a.localeCompare(b))
})

// Sem destino não há backup automático.
watch(
  () => form.storageDestinationId,
  (id) => {
    if (id == null) form.backupEnabled = false
  }
)

function resetForm() {
  Object.assign(form, {
    name: '',
    engine: 'postgres',
    host: '',
    port: 5432,
    username: '',
    password: '',
    sslMode: 'prefer',
    databases: [],
    storageDestinationId: storagesStore.storages[0]?.id ?? null,
    backupEnabled: storagesStore.storages.length > 0,
    backupIntervalHours: 24,
    backupRetention: 7,
  })
  scope.value = 'all'
  secrets.reset()
  verified.reset()
  saveError.value = null
}

function fillFrom(row: DatabaseConnectionResponse) {
  Object.assign(form, {
    name: row.name,
    engine: row.engine,
    host: row.host,
    port: row.port,
    username: row.username,
    password: '',
    sslMode: row.sslMode,
    databases: [...row.databases],
    storageDestinationId: row.storageDestinationId,
    backupEnabled: row.backupEnabled,
    backupIntervalHours: row.backupIntervalHours,
    backupRetention: row.backupRetention,
  })
  scope.value = row.databases.length ? 'some' : 'all'
  secrets.reset(row.passwordSet ? ['password'] : [])
}

watch(
  () => props.modelValue,
  async (open) => {
    if (!open) return
    if (!storagesStore.loaded) await storagesStore.fetchStorages()
    resetForm()
    const existing = databasesStore.connections.find((item) => item.id === props.connectionId)
    if (existing) {
      fillFrom(existing)
      step.value = 'form'
    } else {
      step.value = 'engine'
    }
  }
)

function chooseEngine(value: DatabaseEngine) {
  const previous = engineInfo(form.engine)
  form.engine = value
  // A porta acompanha o SGBD, a menos que o operador já a tenha mudado.
  if (!form.port || Number(form.port) === previous.defaultPort) {
    form.port = engineInfo(value).defaultPort
  }
  if (!form.name || form.name === previous.label) form.name = engineInfo(value).label
  if (!form.username) form.username = value === 'postgres' ? 'postgres' : 'root'
  step.value = 'form'
}

function buildInput(): DatabaseConnectionInput {
  return {
    name: form.name.trim(),
    engine: form.engine,
    host: form.host.trim(),
    port: Number(form.port) || engine.value.defaultPort,
    username: form.username.trim(),
    password: form.password || undefined,
    sslMode: form.sslMode,
    databases: scope.value === 'all' ? [] : form.databases,
    storageDestinationId: form.storageDestinationId,
    backupEnabled: form.backupEnabled && form.storageDestinationId != null,
    backupIntervalHours: form.backupIntervalHours,
    backupRetention: Number(form.backupRetention),
  }
}

async function save() {
  const validation = await formRef.value?.validate()
  if (validation && !validation.valid) return
  if (scope.value === 'some' && form.databases.length === 0) {
    saveError.value = 'Escolha pelo menos um banco, ou marque "Todos os bancos".'
    return
  }
  const { ok } = await verified.ready()
  if (!ok) return

  saving.value = true
  saveError.value = null
  try {
    const created = props.connectionId == null
    const saved = await databasesStore.save(props.connectionId, buildInput())
    emit('saved', saved, created)
    close()
  } catch (err) {
    saveError.value = err instanceof Error ? err.message : 'Erro ao salvar a conexão'
  } finally {
    saving.value = false
  }
}

function close() {
  emit('update:modelValue', false)
}
</script>

<style scoped>
.engine-card {
  cursor: pointer;
  transition: border-color 0.15s;
}
.engine-card:hover {
  border-color: rgb(var(--v-theme-primary));
}
.section-title {
  font-weight: 700;
  font-size: 0.875rem;
  margin: 8px 0 12px;
}
</style>

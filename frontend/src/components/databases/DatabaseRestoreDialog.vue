<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 600"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="backup" class="rounded-lg">
      <v-card-title class="font-weight-bold pt-4 px-6">Restaurar cópia</v-card-title>
      <v-card-subtitle class="px-6 text-wrap">
        <strong>{{ backup.databaseName }}</strong> de {{ formatDateTime(backup.startedAt) }}
        <span v-if="backup.sizeBytes != null"> · {{ formatBytes(backup.sizeBytes) }}</span>
        <span> · {{ formatCompactCount(backup.rows) }} linhas</span>
      </v-card-subtitle>

      <v-card-text class="px-6">
        <template v-if="!job">
          <v-select
            v-model="targetId"
            :items="targets"
            item-title="name"
            item-value="id"
            label="Restaurar no servidor"
            variant="outlined"
            :hint="`Só conexões ${family === 'postgres' ? 'PostgreSQL' : 'MySQL/MariaDB'} recebem esta cópia.`"
            persistent-hint
            class="mb-4"
          ></v-select>

          <v-btn-toggle
            v-model="mode"
            color="primary"
            variant="outlined"
            divided
            mandatory
            class="mb-4 w-100"
          >
            <v-btn value="new_database" class="flex-1-1" prepend-icon="mdi-database-plus-outline">
              Banco novo
            </v-btn>
            <v-btn value="replace" class="flex-1-1" prepend-icon="mdi-database-sync-outline">
              Substituir existente
            </v-btn>
          </v-btn-toggle>

          <v-text-field
            v-model="database"
            :label="mode === 'new_database' ? 'Nome do banco novo' : 'Banco que será substituído'"
            variant="outlined"
            :rules="[nameRule]"
            class="mb-2"
          ></v-text-field>

          <v-alert v-if="mode === 'new_database'" type="info" variant="tonal" density="compact">
            O banco é criado agora e o atual não é tocado — o jeito seguro de conferir uma cópia. Se
            algo falhar, o banco novo é apagado.
          </v-alert>
          <template v-else>
            <v-alert type="warning" variant="tonal" density="compact" class="mb-3">
              <template v-if="family === 'postgres'">
                Todo o conteúdo de <strong>{{ database }}</strong> é substituído pelo da cópia, numa
                transação só: se algo falhar, o banco fica exatamente como estava.
              </template>
              <template v-else>
                <strong>{{ database }}</strong> é apagado antes de a cópia ser aplicada. No
                MySQL/MariaDB isso não volta atrás: se a restauração falhar no meio, o banco fica
                incompleto. Na dúvida, restaure num banco novo primeiro.
              </template>
            </v-alert>
            <v-text-field
              v-model="confirmation"
              :label="`Digite ${database} para confirmar`"
              variant="outlined"
              autocomplete="off"
            ></v-text-field>
          </template>

          <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mt-2">
            {{ error }}
          </v-alert>
        </template>

        <DatabaseJobProgress v-else :job="job" />
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="emit('update:modelValue', false)">
          {{ job ? 'Fechar' : 'Cancelar' }}
        </v-btn>
        <v-btn
          v-if="!job"
          :color="mode === 'replace' ? 'error' : 'primary'"
          variant="flat"
          prepend-icon="mdi-database-import-outline"
          :disabled="!canRestore"
          :loading="starting"
          @click="start"
        >
          Restaurar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { DatabaseBackupResponse } from '@/bindings/DatabaseBackupResponse'
import type { DatabaseConnectionResponse } from '@/bindings/DatabaseConnectionResponse'
import type { RestoreMode } from '@/bindings/RestoreMode'
import DatabaseJobProgress from './DatabaseJobProgress.vue'
import { useDatabasesStore } from '@/stores/databases'
import { engineInfo, restoredDatabaseName } from '@/utils/databasePresentation'
import { formatBytes, formatCompactCount, formatDateTime } from '@/utils/formatters'

const props = defineProps<{
  modelValue: boolean
  backup: DatabaseBackupResponse | null
  /** Conexão de onde a cópia veio. */
  source: DatabaseConnectionResponse | null
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>()

const databasesStore = useDatabasesStore()

const targetId = ref<number | null>(null)
const mode = ref<RestoreMode>('new_database')
const database = ref('')
const confirmation = ref('')
const starting = ref(false)
const error = ref<string | null>(null)
/** Id do andamento desta restauração, para não mostrar o de outra. */
const startedJobId = ref<string | null>(null)

const family = computed(() => engineInfo(props.source?.engine ?? 'postgres').family)
const targets = computed(() =>
  databasesStore.connections.filter((item) => engineInfo(item.engine).family === family.value)
)
const job = computed(() => {
  if (!startedJobId.value || targetId.value == null) return null
  const current = databasesStore.jobFor('restore', targetId.value)
  return current?.id === startedJobId.value ? current : null
})

const NAME = /^[A-Za-z_][A-Za-z0-9_-]{0,62}$/
function nameRule(value: string): true | string {
  return NAME.test(value.trim()) || 'Use letras, números, _ ou -, começando por letra'
}

const canRestore = computed(
  () =>
    targetId.value != null &&
    nameRule(database.value) === true &&
    (mode.value === 'new_database' || confirmation.value.trim() === database.value.trim())
)

// Trocar o modo troca a sugestão de nome: novo não pode ser o da origem.
watch(mode, (value) => {
  if (!props.backup) return
  database.value =
    value === 'replace'
      ? props.backup.databaseName
      : restoredDatabaseName(props.backup.databaseName)
  confirmation.value = ''
})

watch(
  () => props.modelValue,
  (open) => {
    if (!open || !props.backup) return
    targetId.value = props.source?.id ?? null
    mode.value = 'new_database'
    database.value = restoredDatabaseName(props.backup.databaseName)
    confirmation.value = ''
    error.value = null
    startedJobId.value = null
  }
)

async function start() {
  if (!props.backup || targetId.value == null) return
  starting.value = true
  error.value = null
  try {
    const snapshot = await databasesStore.restore(props.backup.id, {
      targetConnectionId: targetId.value,
      database: database.value.trim(),
      mode: mode.value,
      confirmDatabase: mode.value === 'replace' ? confirmation.value.trim() : undefined,
    })
    startedJobId.value = snapshot.id
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Erro ao iniciar a restauração'
  } finally {
    starting.value = false
  }
}
</script>

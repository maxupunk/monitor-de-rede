<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="success">mdi-database-sync-outline</v-icon>
      Backup e Restauração
    </v-card-title>
    <v-card-text class="mt-2">
      <p class="text-caption text-grey-darken-1 mb-4">
        O backup grava as <strong>configurações</strong> do banco — sites, redes, dispositivos,
        interfaces, enlaces, monitores, regras de alerta, servidores DNS, servidor VPN com seus
        peers e as preferências do sistema. Histórico de coleta (métricas, resultados, eventos) e
        contas de acesso ficam de fora.
      </p>

      <v-alert
        v-if="backupStore.error"
        type="error"
        variant="tonal"
        density="compact"
        class="mb-4"
        closable
        @click:close="backupStore.error = null"
      >
        {{ backupStore.error }}
      </v-alert>

      <v-row>
        <v-col cols="12" md="6">
          <div class="pa-4 rounded-lg border h-100 d-flex flex-column">
            <div class="font-weight-bold text-subtitle-2 mb-1">Exportar</div>
            <div class="text-caption text-grey-darken-1 mb-4">
              Baixa um arquivo JSON com a configuração atual. Ele carrega a community SNMP dos
              dispositivos e as chaves da VPN cifradas — guarde-o como guardaria uma senha.
            </div>
            <v-spacer></v-spacer>
            <div>
              <v-btn
                color="success"
                variant="flat"
                prepend-icon="mdi-download"
                :loading="backupStore.exporting"
                @click="emit('export')"
              >
                Baixar Backup
              </v-btn>
            </div>
          </div>
        </v-col>

        <v-col cols="12" md="6">
          <div class="pa-4 rounded-lg border h-100 d-flex flex-column">
            <div class="font-weight-bold text-subtitle-2 mb-1">Restaurar</div>
            <div class="text-caption text-grey-darken-1 mb-3">
              Substitui toda a configuração atual pela do arquivo. O histórico de coleta dos
              equipamentos atuais é descartado junto.
            </div>

            <v-file-input
              v-model="selectedFile"
              label="Arquivo de backup (.json)"
              accept="application/json,.json"
              variant="outlined"
              density="comfortable"
              prepend-icon=""
              prepend-inner-icon="mdi-file-upload-outline"
              hide-details
              class="mb-3"
              @update:model-value="onFileSelected"
            ></v-file-input>

            <div v-if="backupStore.pendingCounts" class="mb-3">
              <div class="text-caption text-grey-darken-1 mb-1">
                <strong>{{ backupStore.pendingName }}</strong> —
                {{ backupStore.pendingCounts.totalRows }} registros
              </div>
              <v-chip
                v-for="row in backupStore.pendingCounts.tables"
                :key="row.table"
                size="x-small"
                variant="tonal"
                color="info"
                class="mr-1 mb-1"
              >
                {{ tableLabel(row.table) }}: {{ row.rows }}
              </v-chip>
            </div>

            <v-spacer></v-spacer>
            <div>
              <v-btn
                color="warning"
                variant="flat"
                prepend-icon="mdi-database-import-outline"
                :disabled="!backupStore.pendingFile"
                :loading="backupStore.restoring"
                @click="emit('confirm-restore')"
              >
                Restaurar Configurações
              </v-btn>
            </div>
          </div>
        </v-col>
      </v-row>

      <v-alert
        v-if="backupStore.lastRestore"
        type="success"
        variant="tonal"
        density="compact"
        class="mt-4"
      >
        Restauração concluída — {{ backupStore.lastRestore.totalRows }} registros aplicados.
      </v-alert>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useBackupStore, tableLabel } from '@/stores/backup'

const emit = defineEmits<{
  export: []
  'confirm-restore': []
  'file-selected': [file: File | null]
}>()

const backupStore = useBackupStore()
const selectedFile = ref<File | File[] | null>(null)

async function onFileSelected(value: File | File[] | null) {
  const file = Array.isArray(value) ? value[0] : value
  if (!file) {
    backupStore.clearFile()
    emit('file-selected', null)
    return
  }
  emit('file-selected', file)
}
</script>

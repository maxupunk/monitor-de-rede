<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 640"
    :fullscreen="$vuetify.display.xs"
    :persistent="downloading"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center ga-2 pt-4 px-6">
        <v-icon color="primary">mdi-database-export-outline</v-icon>
        Baixar o backup completo
      </v-card-title>

      <v-card-text class="px-6">
        <p class="text-body-2 mb-3">
          Um arquivo <code>.json.gz</code> com os bancos inteiros do NetMonitor, para guardar fora
          deste servidor ou levar a outra instalação. Volta pelo
          <strong>Restaurar › De um arquivo</strong> — ou logo no primeiro acesso de uma instalação
          nova.
        </p>

        <v-card border flat class="pa-3 mb-3">
          <div class="font-weight-bold text-body-2 mb-1">Vai no arquivo</div>
          <ul class="text-body-2 pl-4 mb-2">
            <li>
              <strong>Banco principal</strong> — configuração, usuários e senhas, histórico de
              monitores, métricas, alertas, auditoria, conversas da IA, deploys e o plano de backup.
            </li>
            <li><strong>Banco de logs</strong> — os logs Syslog dos equipamentos.</li>
          </ul>
          <div class="font-weight-bold text-body-2 mb-1">Fica de fora</div>
          <ul class="text-body-2 pl-4 mb-0">
            <li>
              Os <strong>bancos protegidos</strong> dos seus servidores: as cópias deles já ficam
              nos destinos, cada uma com o seu histórico.
            </li>
            <li>
              As camadas das imagens do registry (ficam no disco) e o que o sistema baixa ou refaz
              sozinho: fabricantes, base de vulnerabilidades e filas de envio.
            </li>
          </ul>
        </v-card>

        <v-alert type="warning" variant="tonal" density="compact" class="mb-3">
          O arquivo leva senhas (com hash), tokens e segredos cifrados com a
          <code>ENCRYPTION_KEY</code> desta instalação: guarde-o como guardaria a própria chave.
        </v-alert>

        <div v-if="usageStore.usage" class="text-body-2 mb-3" data-current-size>
          Tamanho atual dos bancos:
          <strong>{{ formatBinaryBytes(usageStore.usage.totalBytes) }}</strong
          >. O arquivo sai comprimido e costuma ficar bem menor.
        </div>

        <div v-if="downloading" class="mb-3" data-download-progress>
          <div class="d-flex justify-space-between text-body-2 mb-1">
            <span class="font-weight-bold">Baixando…</span>
            <span
              >{{ formatBinaryBytes(backupStore.fullDownload?.receivedBytes ?? 0) }} recebidos</span
            >
          </div>
          <v-progress-linear indeterminate color="primary" height="8" rounded></v-progress-linear>
        </div>

        <v-alert
          v-if="done"
          type="success"
          variant="tonal"
          density="compact"
          class="mb-3"
          data-download-done
        >
          Backup completo baixado ({{ formatBinaryBytes(done) }}). Confira a pasta de downloads do
          navegador.
        </v-alert>

        <v-alert
          v-if="backupStore.error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-3"
          :text="backupStore.error"
        ></v-alert>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn
          v-if="downloading"
          variant="text"
          color="error"
          @click="backupStore.cancelFullDownload()"
        >
          Cancelar download
        </v-btn>
        <v-btn v-else variant="text" color="primary" @click="emit('update:modelValue', false)">
          Fechar
        </v-btn>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-download"
          :loading="downloading"
          @click="download"
        >
          Baixar agora
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useBackupStore } from '@/stores/backup'
import { useDatabaseUsageStore } from '@/stores/databaseUsage'
import { formatBinaryBytes } from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>()

const backupStore = useBackupStore()
const usageStore = useDatabaseUsageStore()
/** Tamanho do último arquivo baixado, para a confirmação. */
const done = ref<number | null>(null)

const downloading = computed(() => backupStore.fullDownload != null)

async function download() {
  done.value = null
  done.value = await backupStore.downloadFull()
}

// O tamanho atual ajuda a esperar o download; é lido ao abrir, uma vez.
watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    done.value = null
    backupStore.error = null
    void usageStore.fetchUsage()
  },
  { immediate: true }
)
</script>

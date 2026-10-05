<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 d-flex flex-column fill-height">
    <v-card-title class="font-weight-bold d-flex align-center ga-2 px-0">
      <v-icon color="primary">mdi-barcode-scan</v-icon>
      Base de fabricantes (MAC)
    </v-card-title>
    <v-card-text class="px-0 flex-grow-1">
      <p class="text-body-2 mb-3">
        Identifica o fabricante pelo MAC — na descoberta, nos conflitos de rede e no cadastro de
        equipamentos — e ajuda a reconhecer IoT, câmeras, TVs e celulares.
      </p>

      <v-alert
        v-if="vendors.error"
        type="error"
        variant="tonal"
        density="compact"
        class="mb-3"
        closable
        :text="vendors.error"
        @click:close="vendors.error = null"
      ></v-alert>

      <div v-if="status" class="d-flex flex-wrap ga-2 mb-3">
        <v-chip
          :color="status.entries > 0 ? 'success' : 'warning'"
          variant="tonal"
          prepend-icon="mdi-database"
        >
          {{
            status.entries > 0
              ? `${formatCompactCount(status.entries)} blocos do IEEE`
              : 'Registro do IEEE não baixado'
          }}
        </v-chip>
        <v-chip color="info" variant="tonal" prepend-icon="mdi-package-variant-closed">
          {{ status.builtinEntries }} na base embutida
        </v-chip>
        <v-chip
          v-if="status.updatedAt"
          :color="status.stale ? 'warning' : 'primary'"
          variant="tonal"
          prepend-icon="mdi-update"
        >
          Atualizado {{ formatRelativeTime(status.updatedAt) }}
        </v-chip>
      </div>
      <p v-if="status" class="text-body-2 mb-4">
        {{
          status.autoUpdate
            ? `Atualização automática ligada: o servidor baixa de novo a cada ${MAX_AGE_DAYS} dias.`
            : 'Atualização automática desligada (OUI_AUTO_UPDATE=false): só pelo botão abaixo.'
        }}
        Sem internet, a base embutida continua respondendo.
      </p>

      <v-text-field
        v-model="mac"
        label="Consultar um MAC"
        placeholder="Ex: 5C:CF:7F:12:34:56"
        prepend-inner-icon="mdi-magnify"
        variant="outlined"
        density="comfortable"
        hide-details
        clearable
      ></v-text-field>
      <MacVendorHint :mac="mac" />
    </v-card-text>
    <v-card-actions class="px-0">
      <v-spacer></v-spacer>
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-cloud-download-outline"
        :loading="vendors.refreshing || status?.updating"
        @click="refresh"
      >
        Atualizar agora
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import MacVendorHint from '@/components/devices/MacVendorHint.vue'
import { useVendorsStore } from '@/stores/vendors'
import { formatCompactCount, formatRelativeTime } from '@/utils/formatters'

/** Espelho de `vendors::service::MAX_AGE_DAYS`. */
const MAX_AGE_DAYS = 30

const emit = defineEmits<{ saved: [message: string] }>()

const vendors = useVendorsStore()
const status = computed(() => vendors.status)
const mac = ref<string | null>('')

onMounted(() => void vendors.fetchStatus())

async function refresh() {
  const outcome = await vendors.refresh()
  if (outcome) {
    emit('saved', `Base de fabricantes atualizada: ${formatCompactCount(outcome.entries)} blocos.`)
  }
}
</script>

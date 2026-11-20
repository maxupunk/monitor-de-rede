<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 900"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center flex-wrap ga-2 pt-4 px-6">
        <v-icon color="info">mdi-chart-donut</v-icon>
        O que ocupa o espaço do banco
        <v-spacer></v-spacer>
        <v-btn
          color="info"
          variant="tonal"
          size="small"
          prepend-icon="mdi-refresh"
          :loading="store.loading"
          @click="store.fetchUsage()"
        >
          Atualizar
        </v-btn>
      </v-card-title>
      <v-card-subtitle class="px-6 text-wrap">
        Cada tabela, da maior para a menor, com a área do produto a que pertence.
      </v-card-subtitle>

      <v-card-text class="px-6">
        <v-progress-linear
          v-if="store.loading && !store.usage"
          indeterminate
          color="info"
          class="mb-4"
        ></v-progress-linear>

        <v-alert
          v-if="store.error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-4"
          :text="store.error"
        ></v-alert>

        <template v-if="store.usage">
          <div class="d-flex align-center flex-wrap ga-2 mb-4">
            <span class="text-body-1 font-weight-bold">
              Total: {{ formatBinaryBytes(store.usage.totalBytes) }}
            </span>
            <v-chip size="small" color="info" variant="tonal">
              {{ store.usage.dbType === 'sqlite' ? 'SQLite' : 'PostgreSQL' }}
            </v-chip>
            <v-spacer></v-spacer>
            <v-switch
              v-model="byArea"
              color="primary"
              inset
              hide-details
              density="compact"
              label="Agrupar por área"
            ></v-switch>
          </div>

          <v-card
            v-for="section in store.usage.databases"
            :key="section.kind"
            border
            flat
            class="mb-4"
            :data-section="section.kind"
          >
            <v-card-item>
              <template #prepend>
                <v-avatar color="info" variant="tonal" rounded="lg">
                  <v-icon>
                    {{ section.kind === 'logs' ? 'mdi-text-box-search-outline' : 'mdi-database' }}
                  </v-icon>
                </v-avatar>
              </template>
              <v-card-title class="text-subtitle-1 font-weight-bold">
                {{ section.title }} · {{ formatBinaryBytes(section.sizeBytes) }}
              </v-card-title>
              <v-card-subtitle v-if="section.name" class="text-high-emphasis">
                {{ section.name }}
              </v-card-subtitle>
            </v-card-item>

            <v-card-text class="pt-0">
              <v-alert
                v-if="!section.detailAvailable"
                type="info"
                variant="tonal"
                density="compact"
                class="mb-3"
              >
                Este SQLite não informa o tamanho de cada tabela (a extensão <code>dbstat</code> não
                está compilada). Abaixo, o tamanho total do arquivo e o número exato de linhas de
                cada tabela — a porcentagem é pelas linhas.
              </v-alert>

              <template v-if="byArea">
                <div v-for="group in groupByArea(section)" :key="group.area" class="mb-3">
                  <div class="d-flex align-center flex-wrap ga-2 mb-1">
                    <v-chip size="small" :color="AREA_COLORS[group.area]" variant="flat">
                      {{ group.title }}
                    </v-chip>
                    <span class="text-body-2 font-weight-bold">
                      {{ sizeLabel(group.totalBytes, group.rows) }}
                    </span>
                    <span class="text-body-2">· {{ formatPercent(group.share) }}</span>
                  </div>
                  <TableUsageList :rows="group.tables" :show-area="false" />
                </div>
              </template>
              <TableUsageList v-else :rows="usageRows(section)" :show-area="true" />

              <div
                v-if="section.otherBytes"
                class="d-flex align-center justify-space-between text-body-2 px-1 pt-2 border-t"
              >
                <span>Outros: catálogo do banco, sequências e espaço livre a reaproveitar</span>
                <span class="font-weight-bold">{{ formatBinaryBytes(section.otherBytes) }}</span>
              </div>
            </v-card-text>
          </v-card>

          <v-alert type="info" variant="tonal" density="comfortable" icon="mdi-broom">
            <div class="font-weight-bold mb-1">Como liberar espaço</div>
            <div class="text-body-2">
              O histórico mais antigo que a retenção de cada tabela (o “guarda N dias” acima) sai
              sozinho, todo dia. Para liberar já, <strong>Apagar Histórico</strong> remove métricas,
              resultados, alertas passados e logs, e mantém dispositivos, monitores e demais
              cadastros.
            </div>
            <v-btn
              color="error"
              variant="tonal"
              size="small"
              class="mt-2"
              prepend-icon="mdi-delete-sweep-outline"
              @click="emit('clear-history')"
            >
              Apagar Histórico…
            </v-btn>
          </v-alert>
        </template>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" color="primary" @click="emit('update:modelValue', false)">
          Fechar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import TableUsageList from './TableUsageList.vue'
import { useDatabaseUsageStore } from '@/stores/databaseUsage'
import { AREA_COLORS, groupByArea, usageRows } from '@/utils/databaseUsage'
import { formatBinaryBytes, formatInteger, formatPercent } from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  /** O operador quer liberar espaço: o cartão abre a confirmação que já existe. */
  (e: 'clear-history'): void
}>()

const store = useDatabaseUsageStore()
const byArea = ref(true)

/** Sem o tamanho por tabela, o grupo diz quantas linhas tem. */
function sizeLabel(bytes: number | null, rows: number): string {
  return bytes != null ? formatBinaryBytes(bytes) : `${formatInteger(rows)} linhas`
}

// Mede ao abrir — sob demanda, nunca em ciclo.
watch(
  () => props.modelValue,
  (open) => {
    if (open) void store.fetchUsage()
  },
  { immediate: true }
)
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 760"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-item class="pt-4 px-6">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-history</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Histórico — {{ target?.name }}</v-card-title>
        <v-card-subtitle class="text-wrap text-high-emphasis">
          Abra um deploy para ver o console. Num deploy que deu certo, "Restaurar esta versão" volta
          a imagem e o .env dele.
        </v-card-subtitle>
      </v-card-item>
      <v-card-text class="px-4">
        <v-alert v-if="error" type="error" variant="tonal" class="mb-4">{{ error }}</v-alert>
        <v-skeleton-loader v-if="loading" type="list-item-two-line@5"></v-skeleton-loader>
        <DeployRunsList
          v-else
          :runs="runs"
          :show-place="false"
          empty-text="Este alvo ainda não teve deploy."
          @open="(id) => emit('open-run', id)"
        />
      </v-card-text>
      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="emit('update:modelValue', false)">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import DeployRunsList from '@/components/deploys/DeployRunsList.vue'
import type { DeployRunView } from '@/bindings/DeployRunView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import { useDeploysStore } from '@/stores/deploys'

const props = defineProps<{ modelValue: boolean; target: DeployTargetView | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; 'open-run': [id: number] }>()

const deploys = useDeploysStore()
const loaded = ref<DeployRunView[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

/** O histórico lido, com o estado mais novo que o SSE trouxe. */
const runs = computed(() => loaded.value.map((run) => deploys.runs[run.id] ?? run))

watch(
  () => props.modelValue,
  async (visible) => {
    if (!visible || !props.target) return
    loading.value = true
    error.value = null
    try {
      loaded.value = await deploys.history(props.target.id)
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Não foi possível ler o histórico'
    } finally {
      loading.value = false
    }
  },
  { immediate: true }
)
</script>

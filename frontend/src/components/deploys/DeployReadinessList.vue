<template>
  <div>
    <div class="d-flex align-center ga-2 mb-2">
      <div class="text-subtitle-2 font-weight-bold">Checagem de prontidão</div>
      <v-spacer></v-spacer>
      <v-btn
        size="small"
        variant="tonal"
        color="primary"
        prepend-icon="mdi-refresh"
        :loading="loading"
        @click="emit('refresh')"
      >
        Checar de novo
      </v-btn>
    </div>

    <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-2">
      {{ error }}
    </v-alert>
    <v-skeleton-loader v-else-if="loading && !readiness" type="list-item-two-line@3">
    </v-skeleton-loader>
    <template v-else-if="readiness">
      <v-alert
        :type="readiness.ready ? 'success' : 'error'"
        variant="tonal"
        density="compact"
        class="mb-3"
        data-test="readiness-summary"
      >
        {{
          readiness.ready
            ? 'Tudo pronto para o primeiro deploy.'
            : 'Resolva os itens em vermelho antes do primeiro deploy.'
        }}
      </v-alert>
      <v-list density="compact" class="bg-transparent pa-0">
        <v-list-item
          v-for="check in readiness.checks"
          :key="check.id"
          :title="check.label"
          :subtitle="check.detail"
          class="px-0 readiness-item"
        >
          <template #prepend>
            <v-icon :color="READINESS_META[check.level].color" class="mr-3">
              {{ READINESS_META[check.level].icon }}
            </v-icon>
          </template>
          <div v-if="check.fix && check.level !== 'ok'" class="text-body-2 text-high-emphasis mt-1">
            <v-icon size="16" color="info" class="mr-1">mdi-lightbulb-on-outline</v-icon>
            {{ check.fix }}
          </div>
        </v-list-item>
      </v-list>
    </template>
  </div>
</template>

<script setup lang="ts">
import type { DeployReadinessView } from '@/bindings/DeployReadinessView'
import { READINESS_META } from '@/utils/deployPresentation'

defineProps<{
  readiness: DeployReadinessView | null
  loading: boolean
  error: string | null
}>()

const emit = defineEmits<{ refresh: [] }>()
</script>

<style scoped>
.readiness-item :deep(.v-list-item-subtitle) {
  -webkit-line-clamp: unset;
  white-space: normal;
}
</style>

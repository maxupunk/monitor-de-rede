<template>
  <v-card variant="outlined" class="rounded-lg pa-3">
    <div class="d-flex align-center justify-space-between flex-wrap ga-2">
      <div class="d-flex align-center ga-2">
        <v-icon color="primary" size="18">mdi-text-box-check-outline</v-icon>
        <span class="text-subtitle-2 font-weight-bold">Último resumo da rede</span>
        <span v-if="digest" class="text-caption text-medium-emphasis">
          {{ formatRelativeTime(digest.generatedAt) }} · {{ periodLabel }}
        </span>
      </div>
      <v-btn
        size="small"
        color="primary"
        variant="tonal"
        prepend-icon="mdi-creation-outline"
        :loading="aiStore.runningDigest"
        :disabled="!aiStore.settings?.enabled"
        @click="aiStore.runDigest()"
      >
        Gerar agora
      </v-btn>
    </div>

    <v-alert
      v-if="aiStore.digestError"
      type="error"
      variant="tonal"
      density="compact"
      class="mt-2"
      closable
      @click:close="aiStore.digestError = null"
    >
      {{ aiStore.digestError }}
    </v-alert>

    <div v-if="digest" class="digest-text text-body-2 mt-2">{{ digest.text }}</div>
    <div v-else-if="!aiStore.loadingDigest" class="text-caption text-medium-emphasis mt-2">
      Nenhum resumo gerado ainda. Gere agora ou ative o envio periódico acima.
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useAiStore } from '@/stores/ai'
import { formatRelativeTime } from '@/utils/formatters'

const aiStore = useAiStore()
const digest = computed(() => aiStore.latestDigest)

const periodLabel = computed(() =>
  (digest.value?.periodHours ?? 24) > 24 ? 'últimos 7 dias' : 'últimas 24 h'
)

onMounted(() => {
  if (!aiStore.latestDigest) void aiStore.loadLatestDigest()
})
</script>

<style scoped>
.digest-text {
  white-space: pre-wrap;
  max-height: 260px;
  overflow-y: auto;
}
</style>

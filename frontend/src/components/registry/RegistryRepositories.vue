<template>
  <div>
    <v-card v-if="overview" elevation="2" class="pa-4 mb-4" data-test="registry-usage">
      <div class="d-flex align-center ga-2 mb-2">
        <v-icon color="primary">mdi-harddisk</v-icon>
        <span class="font-weight-bold">Espaço</span>
        <v-spacer></v-spacer>
        <span class="text-body-2">
          {{ formatBinaryBytes(overview.usedBytes) }} de
          {{ formatBinaryBytes(overview.quotaBytes) }} · guarda
          {{ overview.settings.keepVersions }} versões por imagem, além das que estão no ar
        </span>
      </div>
      <v-progress-linear
        :model-value="usagePercent"
        :color="usagePercent >= 95 ? 'error' : usagePercent >= 80 ? 'warning' : 'primary'"
        height="10"
        rounded
      ></v-progress-linear>
    </v-card>

    <v-card v-if="registry.loading && !overview" elevation="2" class="pa-6">
      <v-progress-linear indeterminate color="primary"></v-progress-linear>
    </v-card>

    <v-card
      v-else-if="overview && !overview.repositories.length"
      elevation="2"
      class="pa-6 text-center"
    >
      <v-icon size="48" color="primary" class="mb-2">mdi-package-variant</v-icon>
      <div class="text-h6 font-weight-bold mb-1">Nenhuma imagem ainda</div>
      <div class="text-body-1">
        A primeira chega quando um servidor compilar uma aplicação (Deploys › Compilar agora) ou
        quando alguém da equipe fizer <code>docker push</code> pela VPN.
      </div>
    </v-card>

    <v-row v-else dense>
      <v-col
        v-for="repository in overview?.repositories ?? []"
        :key="repository.id"
        cols="12"
        md="6"
        xl="4"
      >
        <ChoiceCard
          class="pa-4 h-100"
          :data-test="`repository-${repository.name}`"
          @click="open(repository.id)"
        >
          <div class="d-flex align-center ga-2 mb-1">
            <v-icon color="primary">mdi-package-variant-closed</v-icon>
            <span class="font-weight-bold text-truncate">{{ repository.name }}</span>
            <v-spacer></v-spacer>
            <v-chip size="small" color="primary" variant="tonal" label>
              {{ repository.versions }} versão(ões)
            </v-chip>
          </div>
          <div class="text-body-2 mb-2">
            <template v-if="repository.application">{{ repository.application }} · </template>
            {{ formatBinaryBytes(repository.bytes) }}
            <template v-if="repository.lastPush">
              · {{ formatRelativeTime(repository.lastPush) }}</template
            >
          </div>
          <v-chip
            :color="scanColor(repository.latestScan)"
            size="small"
            variant="tonal"
            label
            prepend-icon="mdi-shield-search"
          >
            {{ scanSentence(repository.latestScan) }}
          </v-chip>
        </ChoiceCard>
      </v-col>
    </v-row>

    <RegistryRepositoryDialog
      v-model="detailOpen"
      :repository-id="selectedId"
      :can-admin="canAdmin"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import ChoiceCard from '@/components/ChoiceCard.vue'
import RegistryRepositoryDialog from '@/components/registry/RegistryRepositoryDialog.vue'
import { useRegistryStore } from '@/stores/registry'
import { formatBinaryBytes, formatRelativeTime } from '@/utils/formatters'
import { scanColor, scanSentence } from '@/utils/registryPresentation'

defineProps<{ canAdmin: boolean }>()

const registry = useRegistryStore()
const overview = computed(() => registry.overview)
const detailOpen = ref(false)
const selectedId = ref<number | null>(null)

const usagePercent = computed(() => {
  const value = overview.value
  if (!value || value.quotaBytes === 0) return 0
  return Math.min(100, (value.usedBytes / value.quotaBytes) * 100)
})

function open(id: number) {
  selectedId.value = id
  detailOpen.value = true
}
</script>

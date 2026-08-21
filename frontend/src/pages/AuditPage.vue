<template>
  <div>
    <PageHeader
      title="Trilha de Auditoria"
      subtitle="Registro de quem alterou cada recurso, quando e como"
    >
      <template #actions>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-refresh"
          @click="auditStore.fetchLogs"
        >
          Atualizar
        </v-btn>
      </template>
    </PageHeader>

    <v-alert
      v-if="auditStore.error"
      type="error"
      variant="tonal"
      closable
      class="mb-4"
      @click:close="clearError"
    >
      {{ auditStore.error }}
    </v-alert>

    <v-card elevation="2" class="rounded-lg mb-6 pa-4">
      <v-row density="compact">
        <v-col cols="12" md="3">
          <v-text-field
            v-model="search"
            placeholder="Buscar na descrição ou recurso..."
            prepend-inner-icon="mdi-magnify"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            @keyup.enter="applySearch"
            @click:clear="applySearch"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6" md="3">
          <v-select
            v-model="userId"
            :items="userOptions"
            item-title="title"
            item-value="value"
            label="Usuário"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ userId })"
          ></v-select>
        </v-col>
        <v-col cols="12" sm="6" md="2">
          <v-select
            v-model="action"
            :items="actionOptions"
            item-title="label"
            item-value="value"
            label="Ação"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ action })"
          ></v-select>
        </v-col>
        <v-col cols="12" sm="6" md="2">
          <v-select
            v-model="resourceType"
            :items="resourceOptions"
            item-title="label"
            item-value="value"
            label="Recurso"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ resourceType })"
          ></v-select>
        </v-col>
        <v-col cols="12" sm="6" md="2">
          <v-text-field
            v-model="resourceId"
            placeholder="ID do recurso"
            label="ID do recurso"
            hide-details
            clearable
            density="compact"
            variant="outlined"
            type="number"
            @update:model-value="onResourceIdChange"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6" md="3">
          <v-text-field
            v-model="fromDate"
            label="De"
            type="datetime-local"
            hide-details
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ from: toIso(fromDate) })"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6" md="3">
          <v-text-field
            v-model="toDate"
            label="Até"
            type="datetime-local"
            hide-details
            density="compact"
            variant="outlined"
            @update:model-value="onFilterChange({ to: toIso(toDate) })"
          ></v-text-field>
        </v-col>
      </v-row>
      <div class="d-flex justify-end mt-2">
        <v-btn variant="text" size="small" prepend-icon="mdi-filter-remove" @click="clearAll">
          Limpar filtros
        </v-btn>
      </div>
    </v-card>

    <v-card elevation="2" rounded="lg">
      <v-data-table
        :headers="headers"
        :items="auditStore.entries"
        :loading="loading"
        item-value="id"
        expand-on-click
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum registro de auditoria encontrado"
      >
        <template #item.createdAt="{ item }">
          {{ formatDate(item.createdAt) }}
        </template>
        <template #item.user="{ item }">
          <div class="d-flex align-center ga-2">
            <v-icon size="16" color="grey">mdi-account-outline</v-icon>
            <span>{{
              item.userEmail ?? (item.userId != null ? `ID ${item.userId}` : 'Sistema')
            }}</span>
          </div>
        </template>
        <template #item.action="{ item }">
          <v-chip size="small" variant="tonal" :color="actionColor(item.action)">
            {{ actionLabel(item.action) }}
          </v-chip>
        </template>
        <template #item.resource="{ item }">
          <div>
            <div class="font-weight-medium">{{ resourceLabel(item.resourceType) }}</div>
            <div v-if="item.resourceLabel" class="text-caption text-grey">
              {{ item.resourceLabel }}
            </div>
          </div>
        </template>
        <template #expanded-row="{ columns, item }">
          <tr>
            <td :colspan="columns.length" class="pa-4 bg-grey-lighten-4">
              <div class="text-subtitle-2 font-weight-bold mb-2">Detalhes do evento</div>
              <v-row>
                <v-col cols="12" md="6">
                  <div class="text-caption text-grey mb-1">Endereço IP</div>
                  <div>{{ item.ipAddress ?? '—' }}</div>
                </v-col>
                <v-col cols="12" md="6">
                  <div class="text-caption text-grey mb-1">User-Agent</div>
                  <div class="text-truncate">{{ item.userAgent ?? '—' }}</div>
                </v-col>
              </v-row>
              <div v-if="hasChanges(item.changes)" class="mt-4">
                <div class="text-caption text-grey mb-1">Alterações</div>
                <div class="d-flex flex-column flex-md-row ga-4">
                  <v-card
                    v-if="item.changes?.old"
                    variant="outlined"
                    rounded="lg"
                    class="flex-1-1-0 pa-3"
                  >
                    <div class="text-caption font-weight-bold text-error mb-1">Anterior</div>
                    <pre class="text-body-2 overflow-x-auto">{{
                      JSON.stringify(item.changes.old, null, 2)
                    }}</pre>
                  </v-card>
                  <v-card
                    v-if="item.changes?.new"
                    variant="outlined"
                    rounded="lg"
                    class="flex-1-1-0 pa-3"
                  >
                    <div class="text-caption font-weight-bold text-success mb-1">Novo</div>
                    <pre class="text-body-2 overflow-x-auto">{{
                      JSON.stringify(item.changes.new, null, 2)
                    }}</pre>
                  </v-card>
                </div>
              </div>
            </td>
          </tr>
        </template>
      </v-data-table>

      <v-card-actions v-if="auditStore.meta" class="pa-4 justify-center">
        <v-btn
          variant="tonal"
          size="small"
          prepend-icon="mdi-chevron-left"
          :disabled="auditStore.meta.currentPage <= 1"
          @click="prevPage"
        >
          Anterior
        </v-btn>
        <div class="mx-4 text-body-2">
          Página {{ auditStore.meta.currentPage }} de {{ auditStore.meta.lastPage }}
          <span class="text-grey">({{ auditStore.meta.total }} registros)</span>
        </div>
        <v-btn
          variant="tonal"
          size="small"
          append-icon="mdi-chevron-right"
          :disabled="auditStore.meta.currentPage >= auditStore.meta.lastPage"
          @click="nextPage"
        >
          Próxima
        </v-btn>
      </v-card-actions>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import {
  useAuditStore,
  ACTION_OPTIONS,
  RESOURCE_OPTIONS,
  actionLabel,
  actionColor,
  resourceLabel,
  type AuditFilters,
} from '@/stores/audit'
import { useUsersStore } from '@/stores/users'

const auditStore = useAuditStore()
const usersStore = useUsersStore()

const search = ref('')
const userId = ref<number | null>(null)
const action = ref<string | null>(null)
const resourceType = ref<string | null>(null)
const resourceId = ref<number | null>(null)
const fromDate = ref('')
const toDate = ref('')

const actionOptions = ACTION_OPTIONS
const resourceOptions = RESOURCE_OPTIONS

const userOptions = computed(() =>
  usersStore.users.map((user) => ({ title: user.fullName || user.email, value: user.id }))
)

const headers = [
  { title: 'Data', key: 'createdAt', width: '180px' },
  { title: 'Usuário', key: 'user' },
  { title: 'Ação', key: 'action', width: '120px' },
  { title: 'Recurso', key: 'resource' },
  { title: 'Descrição', key: 'description' },
]

const loading = computed(() => auditStore.entries.length === 0 && !auditStore.error)

watch(
  () => auditStore.filters,
  (filters) => {
    search.value = filters.search
    userId.value = filters.userId
    action.value = filters.action
    resourceType.value = filters.resourceType
    resourceId.value = filters.resourceId
  },
  { immediate: true, deep: true }
)

function onFilterChange(next: Partial<AuditFilters>): void {
  auditStore.applyFilters(next)
}

function onResourceIdChange(value: string | number | null): void {
  const parsed = value === null || value === '' ? null : Number(value)
  onFilterChange({ resourceId: Number.isNaN(parsed as number) ? null : parsed })
}

function applySearch(): void {
  onFilterChange({ search: search.value ?? '' })
}

function clearAll(): void {
  search.value = ''
  userId.value = null
  action.value = null
  resourceType.value = null
  resourceId.value = null
  fromDate.value = ''
  toDate.value = ''
  auditStore.clearFilters()
}

function clearError(): void {
  auditStore.error = null
}

function toIso(local: string): string | null {
  if (!local) return null
  try {
    return new Date(local).toISOString()
  } catch {
    return null
  }
}

function formatDate(value: string): string {
  try {
    return new Date(value).toLocaleString('pt-BR', {
      dateStyle: 'short',
      timeStyle: 'short',
    })
  } catch {
    return value
  }
}

function hasChanges(changes: unknown): boolean {
  if (!changes || typeof changes !== 'object') return false
  const obj = changes as Record<string, unknown>
  return obj.old !== undefined || obj.new !== undefined
}

function prevPage(): void {
  if (!auditStore.meta) return
  auditStore.goToPage(auditStore.meta.currentPage - 1)
}

function nextPage(): void {
  if (!auditStore.meta) return
  auditStore.goToPage(auditStore.meta.currentPage + 1)
}

onMounted(() => {
  void usersStore.fetchUsers()
  void auditStore.fetchLogs()
})
</script>

<style scoped>
pre {
  white-space: pre-wrap;
  word-break: break-word;
}
</style>

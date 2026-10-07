<template>
  <v-card elevation="2" rounded="lg" class="h-100 d-flex flex-column">
    <!-- O quê -->
    <div class="d-flex align-start ga-3 pa-4 pb-2">
      <v-avatar :color="color" variant="tonal" rounded="lg" size="48">
        <v-icon size="28">{{ icon }}</v-icon>
      </v-avatar>
      <div class="flex-grow-1 min-w-0">
        <div class="text-subtitle-1 font-weight-bold text-truncate">{{ title }}</div>
        <div class="text-body-2 text-high-emphasis">{{ description }}</div>
      </div>
      <v-menu location="bottom end">
        <template #activator="{ props: menuProps }">
          <v-btn icon size="small" variant="text" color="primary" v-bind="menuProps">
            <v-icon>mdi-dots-vertical</v-icon>
            <v-tooltip activator="parent" location="top">Mais ações</v-tooltip>
          </v-btn>
        </template>
        <v-list density="compact">
          <v-list-item
            prepend-icon="mdi-calendar-edit"
            title="Configurar backup"
            subtitle="Destino, frequência e quantas cópias manter"
            @click="emit('configure')"
          />
          <slot name="menu"></slot>
        </v-list>
      </v-menu>
    </div>

    <div class="px-4 pb-3 flex-grow-1">
      <!-- Está protegido? -->
      <v-chip :color="health.color" variant="tonal" size="small" :prepend-icon="health.icon">
        {{ health.label }}
      </v-chip>
      <div v-if="health.advice" class="text-caption text-high-emphasis mt-1">
        {{ health.advice }}
      </div>

      <slot></slot>

      <!-- Para onde e quando -->
      <div v-if="plan.storageDestinationId != null" class="facts mt-3">
        <div class="fact">
          <v-icon size="18" color="primary">mdi-database-lock-outline</v-icon>
          <span>
            Guardado em <strong>{{ plan.storageDestinationName ?? '—' }}</strong>
          </span>
        </div>
        <div class="fact">
          <v-icon size="18" color="primary">mdi-calendar-sync</v-icon>
          <span v-if="plan.backupEnabled">
            {{ intervalLabel(plan.backupIntervalHours) }}, mantém as últimas
            {{ plan.backupRetention }}
          </span>
          <span v-else>Backup automático desligado</span>
        </div>
        <div v-if="!hideStatus" class="fact">
          <v-icon size="18" :color="last.color">{{ last.icon }}</v-icon>
          <span>{{ last.text }}</span>
        </div>
        <div v-if="!hideStatus && plan.nextBackupAt" class="fact">
          <v-icon size="18" color="primary">mdi-clock-outline</v-icon>
          <span>Próximo automático {{ formatTimeUntil(plan.nextBackupAt) }}</span>
        </div>
      </div>
      <v-alert
        v-if="!hideStatus && plan.lastBackupStatus === 'failed' && plan.lastBackupError"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-2 text-body-2"
      >
        <div class="error-text" :title="plan.lastBackupError">{{ plan.lastBackupError }}</div>
      </v-alert>
    </div>

    <v-divider></v-divider>
    <div class="d-flex flex-wrap ga-2 pa-3">
      <template v-if="plan.storageDestinationId != null">
        <v-btn
          color="success"
          variant="flat"
          prepend-icon="mdi-backup-restore"
          :loading="running"
          @click="emit('backup')"
        >
          Fazer backup agora
        </v-btn>
        <v-btn color="primary" variant="tonal" prepend-icon="mdi-history" @click="emit('restore')">
          Restaurar…
        </v-btn>
      </template>
      <v-btn
        v-else
        color="primary"
        variant="flat"
        prepend-icon="mdi-shield-plus-outline"
        @click="emit('configure')"
      >
        Escolher onde guardar
      </v-btn>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { backupHealth, intervalLabel, type BackupPlanView } from '@/utils/backupSchedule'
import { formatRelativeTime, formatTimeUntil } from '@/utils/formatters'

/**
 * Um item protegido — o NetMonitor ou um banco de dados — com as mesmas
 * perguntas respondidas na mesma ordem: o quê, está protegido, onde, quando,
 * e as duas ações que importam.
 */
const props = defineProps<{
  title: string
  /** O que é copiado, em uma frase. */
  description: string
  icon: string
  color: string
  plan: BackupPlanView
  running?: boolean
  /** Esconde o último/próximo backup (o andamento ao vivo ocupa o lugar). */
  hideStatus?: boolean
}>()

const emit = defineEmits<{
  backup: []
  restore: []
  configure: []
}>()

const health = computed(() => backupHealth(props.plan))

const last = computed(() => {
  const { lastBackupAt, lastBackupStatus } = props.plan
  if (!lastBackupAt)
    return { icon: 'mdi-circle-outline', color: 'info', text: 'Nenhum backup ainda' }
  const when = formatRelativeTime(lastBackupAt)
  return lastBackupStatus === 'failed'
    ? { icon: 'mdi-alert-circle', color: 'error', text: `Último backup falhou ${when}` }
    : { icon: 'mdi-check-circle', color: 'success', text: `Último backup ${when}` }
})
</script>

<style scoped>
.min-w-0 {
  min-width: 0;
}
.facts {
  display: grid;
  gap: 4px;
}
.fact {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.875rem;
}
/* Erro longo não empurra os botões para fora do cartão; o texto inteiro
   fica no `title`. */
.error-text {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}
</style>

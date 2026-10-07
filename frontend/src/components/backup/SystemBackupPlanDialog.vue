<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 620"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center pt-4 px-6">
        <v-icon start color="primary">mdi-monitor-dashboard</v-icon>
        Backup do NetMonitor
      </v-card-title>
      <v-card-subtitle class="px-6 pb-2 text-wrap">
        A cópia leva tudo o que você configurou aqui: dispositivos, monitores, alertas, VPN e
        preferências.
      </v-card-subtitle>

      <v-card-text class="px-6">
        <v-form ref="formRef" @submit.prevent="save">
          <div class="section-title">Para onde vão as cópias</div>
          <DestinationSelect v-model="form.storageDestinationId" class="mb-2" />

          <div class="section-title mt-4">Quando</div>
          <BackupPolicyFields
            v-model:enabled="form.backupEnabled"
            v-model:interval-hours="form.backupIntervalHours"
            v-model:retention="form.backupRetention"
            :disabled="form.storageDestinationId == null"
            :label="
              form.storageDestinationId == null
                ? 'Escolha um destino para ligar o backup automático'
                : 'Fazer backup automaticamente'
            "
          >
            As cópias mais antigas que isso são apagadas sozinhas — só os arquivos que o sistema
            criou, na pasta <code>netmonitor-backups</code>. A cópia traz a community SNMP dos
            equipamentos: prefira um destino privado.
          </BackupPolicyFields>
        </v-form>

        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mt-4">
          {{ error }}
        </v-alert>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" color="primary" @click="close">Cancelar</v-btn>
        <v-btn color="primary" variant="flat" :loading="saving" @click="save">Salvar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import BackupPolicyFields from './BackupPolicyFields.vue'
import DestinationSelect from './DestinationSelect.vue'
import { useBackupStore } from '@/stores/backup'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved'): void
}>()

const backupStore = useBackupStore()
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const saving = ref(false)
const error = ref<string | null>(null)
const form = reactive({
  storageDestinationId: null as number | null,
  backupEnabled: true,
  backupIntervalHours: 24,
  backupRetention: 14,
})

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    error.value = null
    const plan = backupStore.plan
    Object.assign(form, {
      storageDestinationId: plan?.storageDestinationId ?? null,
      // Primeira configuração: o automático já vem ligado — é o que protege.
      backupEnabled: plan?.storageDestinationId == null ? true : plan.backupEnabled,
      backupIntervalHours: plan?.backupIntervalHours ?? 24,
      backupRetention: plan?.backupRetention ?? 14,
    })
  }
)

async function save() {
  const validation = await formRef.value?.validate()
  if (validation && !validation.valid) return
  saving.value = true
  error.value = null
  try {
    await backupStore.savePlan({
      storageDestinationId: form.storageDestinationId,
      backupEnabled: form.backupEnabled && form.storageDestinationId != null,
      backupIntervalHours: form.backupIntervalHours,
      backupRetention: Number(form.backupRetention),
    })
    emit('saved')
    close()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Erro ao salvar o plano de backup'
  } finally {
    saving.value = false
  }
}

function close() {
  emit('update:modelValue', false)
}
</script>

<style scoped>
.section-title {
  font-weight: 700;
  font-size: 0.875rem;
  margin: 8px 0 12px;
}
</style>

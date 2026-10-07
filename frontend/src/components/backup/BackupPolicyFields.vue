<template>
  <div>
    <v-switch
      v-model="enabled"
      color="success"
      inset
      hide-details
      :label="label"
      :disabled="disabled"
      class="mb-2"
    ></v-switch>
    <v-row dense>
      <v-col cols="12" sm="6">
        <v-select
          v-model="intervalHours"
          :items="intervalItems(intervalHours)"
          label="Frequência"
          variant="outlined"
          :disabled="!enabled || disabled"
        ></v-select>
      </v-col>
      <v-col cols="12" sm="6">
        <v-text-field
          v-model.number="retention"
          label="Manter as últimas"
          type="number"
          suffix="cópias"
          variant="outlined"
          min="1"
          max="365"
          :rules="[retentionRule]"
        ></v-text-field>
      </v-col>
    </v-row>
    <div class="text-caption text-high-emphasis">
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { intervalItems, retentionRule } from '@/utils/backupSchedule'

/** Liga/desliga, frequência e retenção — o bloco "Backup automático" dos formulários. */
defineProps<{
  label: string
  /** Desliga o interruptor (ex.: sem armazenamento escolhido). */
  disabled?: boolean
}>()

const enabled = defineModel<boolean>('enabled', { required: true })
const intervalHours = defineModel<number>('intervalHours', { required: true })
const retention = defineModel<number>('retention', { required: true })
</script>

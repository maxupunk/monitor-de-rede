<template>
  <v-col v-if="field.kind === 'objects'" cols="12">
    <SettingsListEditor
      :title="field.label"
      :hint="field.hint"
      :item-schema="field.items ?? {}"
      :model-value="list"
      :disabled="disabled"
      @update:model-value="(items) => emit('update', items)"
    />
  </v-col>
  <v-col v-else-if="field.kind === 'list'" cols="12" :md="compact ? 12 : 6">
    <v-combobox
      :model-value="list"
      :label="field.label"
      :hint="field.hint ?? 'Digite e tecle Enter para incluir'"
      persistent-hint
      multiple
      chips
      closable-chips
      variant="outlined"
      density="comfortable"
      @update:model-value="(items) => emit('update', items)"
    ></v-combobox>
  </v-col>
  <v-col v-else-if="field.kind === 'boolean'" cols="12" :sm="compact ? 12 : 6">
    <v-switch
      :model-value="Boolean(value)"
      :label="field.label"
      :hint="field.hint"
      :persistent-hint="Boolean(field.hint)"
      :hide-details="!field.hint"
      color="primary"
      inset
      density="comfortable"
      @update:model-value="(checked) => emit('update', Boolean(checked))"
    ></v-switch>
  </v-col>
  <v-col v-else cols="12" :md="compact ? 12 : 6">
    <v-select
      v-if="field.options"
      :model-value="value ?? ''"
      :items="optionsOf(field)"
      :label="field.label"
      :hint="field.hint"
      :persistent-hint="Boolean(field.hint)"
      :rules="field.rules"
      variant="outlined"
      density="comfortable"
      @update:model-value="(selected) => emit('update', selected)"
    ></v-select>
    <v-combobox
      v-else-if="suggestions"
      :model-value="String(value ?? '')"
      :items="suggestions"
      :label="label"
      :hint="field.hint"
      :persistent-hint="Boolean(field.hint)"
      :rules="field.rules"
      variant="outlined"
      density="comfortable"
      @update:model-value="(chosen) => emit('update', chosen ?? '')"
    ></v-combobox>
    <v-text-field
      v-else
      :model-value="value ?? ''"
      :label="label"
      :hint="hint"
      :persistent-hint="Boolean(hint)"
      :rules="field.rules"
      :type="inputType"
      :autocomplete="field.secret ? 'new-password' : 'off'"
      :append-inner-icon="field.secret ? (revealed ? 'mdi-eye-off' : 'mdi-eye') : undefined"
      variant="outlined"
      density="comfortable"
      @click:append-inner="revealed = !revealed"
      @update:model-value="(text) => emit('update', text)"
    ></v-text-field>
  </v-col>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { SECRET_MASK, optionsOf, type SchemaField } from '@/utils/pluginSettings'
import SettingsListEditor from './SettingsListEditor.vue'

/** Um campo do formulário gerado do esquema. */
const props = defineProps<{
  field: SchemaField
  value: unknown
  disabled?: boolean
  compact?: boolean
  /** Valores oferecidos (o campo vira uma lista em que também se digita). */
  suggestions?: string[]
}>()

const emit = defineEmits<{ update: [value: unknown] }>()

const revealed = ref(false)
const list = computed(() => (Array.isArray(props.value) ? props.value : []))
const label = computed(() => props.field.label + (props.field.required ? ' *' : ''))
const inputType = computed(() => {
  if (props.field.secret) return revealed.value ? 'text' : 'password'
  return props.field.kind === 'integer' || props.field.kind === 'number' ? 'number' : 'text'
})

/**
 * Segredo já guardado volta mascarado: aí a dica explica como mantê-lo. Fora
 * isso vale a do esquema (num parâmetro, ex.: "vazio = manter a do roteador").
 */
const hint = computed(() => {
  if (props.field.secret && props.value === SECRET_MASK) {
    return 'Guardada cifrada. Deixe como está para manter, ou digite uma nova.'
  }
  if (props.field.secret) return props.field.hint ?? 'Não volta para a tela nem fica no histórico.'
  return props.field.hint
})
</script>

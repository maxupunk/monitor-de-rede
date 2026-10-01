<template>
  <v-card border flat class="rounded-lg">
    <v-card-title class="d-flex align-center ga-2 text-subtitle-1 font-weight-bold">
      {{ title }}
      <v-chip size="x-small" color="primary" variant="tonal">{{ modelValue.length }}</v-chip>
      <v-spacer></v-spacer>
      <v-btn
        size="small"
        color="primary"
        variant="flat"
        prepend-icon="mdi-plus"
        :disabled="disabled"
        @click="openItem(null)"
      >
        Adicionar
      </v-btn>
    </v-card-title>
    <v-card-subtitle v-if="hint">{{ hint }}</v-card-subtitle>
    <v-card-text>
      <div v-if="modelValue.length === 0" class="text-body-2 py-2">Nenhum item ainda.</div>
      <v-table v-else density="compact">
        <thead>
          <tr>
            <th v-for="column in columns" :key="column.name">{{ column.label }}</th>
            <th class="text-right">Ações</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(item, index) in items" :key="String(item.id ?? index)">
            <td v-for="column in columns" :key="column.name">
              <v-icon
                v-if="column.kind === 'boolean'"
                size="18"
                :color="item[column.name] ? 'success' : 'secondary'"
              >
                {{ item[column.name] ? 'mdi-check-circle' : 'mdi-minus-circle-outline' }}
              </v-icon>
              <span v-else :class="{ 'font-weight-bold': column === columns[0] }">
                {{ display(item[column.name]) }}
              </span>
            </td>
            <td class="text-right text-no-wrap">
              <v-btn
                icon="mdi-pencil-outline"
                size="small"
                variant="text"
                color="primary"
                :disabled="disabled"
                :aria-label="`Editar ${display(item[columns[0]?.name ?? ''])}`"
                @click="openItem(index)"
              />
              <v-btn
                icon="mdi-delete-outline"
                size="small"
                variant="text"
                color="error"
                :disabled="disabled"
                :aria-label="`Remover ${display(item[columns[0]?.name ?? ''])}`"
                @click="remove(index)"
              />
            </td>
          </tr>
        </tbody>
      </v-table>
    </v-card-text>

    <v-dialog v-model="dialog.open" max-width="640" scrollable>
      <v-card class="rounded-lg">
        <v-card-title
          >{{ dialog.index === null ? 'Adicionar' : 'Editar' }} — {{ title }}</v-card-title
        >
        <v-card-text>
          <SettingsForm ref="itemForm" v-model="dialog.value" :schema="itemSchema" compact />
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn variant="text" color="secondary" @click="dialog.open = false">Cancelar</v-btn>
          <v-btn color="primary" variant="flat" @click="confirm">OK</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-card>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { SECRET_MASK, defaultsOf, fieldsOf, type Schema } from '@/utils/pluginSettings'
import SettingsForm from './SettingsForm.vue'

type Item = Record<string, unknown>

const props = defineProps<{
  title: string
  hint?: string
  itemSchema: Schema
  modelValue: unknown[]
  disabled?: boolean
}>()

const emit = defineEmits<{ 'update:modelValue': [value: Item[]] }>()

const itemForm = ref<{ validate: () => Promise<boolean> } | null>(null)
const dialog = reactive({ open: false, index: null as number | null, value: {} as Item })

const items = computed<Item[]>(() =>
  props.modelValue.filter((item): item is Item => typeof item === 'object' && item !== null)
)
/** Até 6 colunas: os campos simples, sem os secretos. */
const columns = computed(() =>
  fieldsOf(props.itemSchema)
    .filter((field) => !field.secret && field.kind !== 'list' && field.kind !== 'objects')
    .slice(0, 6)
)

function display(value: unknown): string {
  if (value === '' || value === null || value === undefined) return '—'
  if (value === SECRET_MASK) return '••••••••'
  return String(value)
}

function openItem(index: number | null) {
  dialog.index = index
  dialog.value = index === null ? defaultsOf(props.itemSchema) : { ...items.value[index] }
  dialog.open = true
}

async function confirm() {
  if (!(await itemForm.value?.validate())) return
  const next = [...items.value]
  if (dialog.index === null) next.push(dialog.value)
  else next.splice(dialog.index, 1, dialog.value)
  emit('update:modelValue', next)
  dialog.open = false
}

function remove(index: number) {
  const next = [...items.value]
  next.splice(index, 1)
  emit('update:modelValue', next)
}
</script>

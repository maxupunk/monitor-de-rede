<template>
  <v-form ref="form" :disabled="disabled" @submit.prevent>
    <v-row dense>
      <SettingsField
        v-for="field in basic"
        :key="field.name"
        :field="field"
        :value="value[field.name]"
        :disabled="disabled"
        :compact="compact"
        :suggestions="suggestions?.[field.name]"
        @update="(next) => set(field.name, next)"
      />
    </v-row>
    <v-expansion-panels v-if="advanced.length > 0" variant="accordion" class="mt-2">
      <v-expansion-panel>
        <v-expansion-panel-title>
          <v-icon start color="primary">mdi-tune-variant</v-icon>
          Opções avançadas
        </v-expansion-panel-title>
        <!-- eager: os campos validam mesmo com o painel fechado -->
        <v-expansion-panel-text eager>
          <v-row dense>
            <SettingsField
              v-for="field in advanced"
              :key="field.name"
              :field="field"
              :value="value[field.name]"
              :disabled="disabled"
              :compact="compact"
              :suggestions="suggestions?.[field.name]"
              @update="(next) => set(field.name, next)"
            />
          </v-row>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </v-form>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { fieldsOf, type Schema } from '@/utils/pluginSettings'
import SettingsField from './SettingsField.vue'

const props = defineProps<{
  schema: Schema
  modelValue: Record<string, unknown>
  disabled?: boolean
  /** Uma coluna só (diálogos estreitos). */
  compact?: boolean
  /** Valores que o campo oferece (ex.: as redes que os roteadores têm). */
  suggestions?: Record<string, string[]>
  /** Só estes campos (ex.: os rádios que os equipamentos têm); sem lista, todos. */
  only?: string[]
}>()

const emit = defineEmits<{ 'update:modelValue': [value: Record<string, unknown>] }>()

const form = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
/** O que aparece: sem os ocultos e, com `only`, só os pedidos. */
const visible = computed(() =>
  fieldsOf(props.schema).filter(
    (field) => !field.hidden && (!props.only || props.only.includes(field.name))
  )
)
const basic = computed(() => visible.value.filter((field) => !field.advanced))
const advanced = computed(() => visible.value.filter((field) => field.advanced))
const value = computed(() => props.modelValue ?? {})

function set(name: string, next: unknown) {
  emit('update:modelValue', { ...value.value, [name]: next })
}

async function validate(): Promise<boolean> {
  const result = await form.value?.validate()
  return result?.valid ?? true
}

defineExpose({ validate })
</script>

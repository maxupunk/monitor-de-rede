<template>
  <v-form ref="form" :disabled="disabled" @submit.prevent>
    <SettingsSection
      v-for="section in sectionsOf(basic)"
      :key="section.title ?? ''"
      :section="section"
      :value="value"
      :disabled="disabled"
      :compact="compact"
      :suggestions="suggestions"
      @update="set"
    />
    <v-expansion-panels v-if="advanced.length > 0" variant="accordion" class="mt-2">
      <v-expansion-panel>
        <v-expansion-panel-title>
          <v-icon start color="primary">mdi-tune-variant</v-icon>
          Opções avançadas
        </v-expansion-panel-title>
        <!-- eager: os campos validam mesmo com o painel fechado -->
        <v-expansion-panel-text eager>
          <SettingsSection
            v-for="section in sectionsOf(advanced)"
            :key="section.title ?? ''"
            :section="section"
            :value="value"
            :disabled="disabled"
            :compact="compact"
            :suggestions="suggestions"
            @update="set"
          />
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </v-form>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { fieldsOf, isVisible, sectionsOf, type Schema } from '@/utils/pluginSettings'
import SettingsSection from './SettingsSection.vue'

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
const value = computed(() => props.modelValue ?? {})
/**
 * O que aparece: sem os ocultos, só os pedidos (`only`) e só os que a regra
 * `visibleWhen` deixa — a senha some quando a rede é aberta.
 */
const visible = computed(() =>
  fieldsOf(props.schema).filter(
    (field) =>
      !field.hidden &&
      (!props.only || props.only.includes(field.name)) &&
      isVisible(field, props.schema, value.value)
  )
)
const basic = computed(() => visible.value.filter((field) => !field.advanced))
const advanced = computed(() => visible.value.filter((field) => field.advanced))

function set(name: string, next: unknown) {
  emit('update:modelValue', { ...value.value, [name]: next })
}

async function validate(): Promise<boolean> {
  const result = await form.value?.validate()
  return result?.valid ?? true
}

defineExpose({ validate })
</script>

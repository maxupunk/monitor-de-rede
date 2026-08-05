<template>
  <v-dialog
    :model-value="modelValue"
    max-width="820"
    scrollable
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold pt-4 px-6">Regras Pré-configuradas</v-card-title>
      <v-card-subtitle class="px-6 pb-3">
        Marque as regras que deseja habilitar. As já configuradas aparecem bloqueadas e não são
        criadas novamente.
      </v-card-subtitle>
      <v-divider></v-divider>

      <v-card-text class="pa-0" style="max-height: 60vh">
        <div v-if="alertsStore.catalogLoading && groups.length === 0" class="pa-8 text-center">
          <v-progress-circular indeterminate color="primary"></v-progress-circular>
        </div>

        <div v-else-if="groups.length === 0" class="pa-8 text-center text-grey">
          Nenhuma regra pré-configurada disponível.
        </div>

        <div v-else>
          <div v-for="group in groups" :key="group.category">
            <div class="px-6 py-2 bg-grey-lighten-4 text-caption font-weight-bold text-uppercase">
              {{ group.label }}
            </div>
            <v-list density="comfortable" class="py-0">
              <v-list-item
                v-for="template in group.templates"
                :key="template.key"
                class="px-6 py-2 border-b"
                :disabled="template.applied"
                @click="toggle(template)"
              >
                <template #prepend>
                  <v-checkbox-btn
                    :model-value="template.applied || selected.includes(template.key)"
                    :disabled="template.applied"
                    color="primary"
                    @click.stop="toggle(template)"
                  ></v-checkbox-btn>
                </template>

                <v-list-item-title class="font-weight-medium d-flex align-center flex-wrap ga-2">
                  {{ template.name }}
                  <v-chip :color="severityColor(template.severity)" size="x-small" label>
                    {{ severityLabel(template.severity) }}
                  </v-chip>
                  <v-chip
                    v-if="template.recommended"
                    color="primary"
                    size="x-small"
                    variant="tonal"
                  >
                    Básica
                  </v-chip>
                  <v-chip v-if="template.applied" size="x-small" variant="outlined">
                    Já configurada
                  </v-chip>
                </v-list-item-title>

                <v-list-item-subtitle class="text-wrap">
                  {{ template.description }}
                </v-list-item-subtitle>
                <div class="text-caption text-grey-darken-1 mt-1">
                  {{ describeRule(template.condition, template.durationSeconds) }}
                </div>
              </v-list-item>
            </v-list>
          </div>
        </div>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="px-6 py-3">
        <v-btn
          variant="text"
          size="small"
          prepend-icon="mdi-star-outline"
          :disabled="pendingRecommended.length === 0"
          @click="selectRecommended"
        >
          Marcar básicas
        </v-btn>
        <v-spacer></v-spacer>
        <span class="text-caption text-grey mr-3">{{ selected.length }} selecionada(s)</span>
        <v-btn variant="text" @click="close">Cancelar</v-btn>
        <v-btn
          color="primary"
          :loading="alertsStore.catalogLoading"
          :disabled="selected.length === 0"
          @click="apply"
        >
          Aplicar Selecionadas
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAlertsStore, type AlertRuleTemplate } from '@/stores/alerts'
import { describeRule, severityColor, severityLabel } from '@/utils/alertPresentation'

const props = defineProps<{ modelValue: boolean }>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'applied', summary: { created: number; skipped: number }): void
}>()

const alertsStore = useAlertsStore()
const selected = ref<string[]>([])

interface TemplateGroup {
  category: string
  label: string
  templates: AlertRuleTemplate[]
}

/** Agrupa por categoria preservando a ordem em que o backend enviou */
const groups = computed<TemplateGroup[]>(() => {
  const byCategory = new Map<string, AlertRuleTemplate[]>()

  for (const template of alertsStore.ruleTemplates) {
    const bucket = byCategory.get(template.category)
    if (bucket) bucket.push(template)
    else byCategory.set(template.category, [template])
  }

  return [...byCategory.entries()].map(([category, templates]) => ({
    category,
    label: alertsStore.ruleCategories[category] ?? category,
    templates,
  }))
})

const pendingRecommended = computed(() =>
  alertsStore.ruleTemplates.filter((template) => template.recommended && !template.applied)
)

watch(
  () => props.modelValue,
  async (isOpen) => {
    if (!isOpen) return
    selected.value = []
    await alertsStore.fetchRuleCatalog()
  }
)

function toggle(template: AlertRuleTemplate) {
  if (template.applied) return
  const index = selected.value.indexOf(template.key)
  if (index === -1) selected.value.push(template.key)
  else selected.value.splice(index, 1)
}

function selectRecommended() {
  const keys = pendingRecommended.value.map((template) => template.key)
  selected.value = [...new Set([...selected.value, ...keys])]
}

function onUpdateModelValue(value: boolean) {
  emit('update:modelValue', value)
}

function close() {
  emit('update:modelValue', false)
}

async function apply() {
  const result = await alertsStore.applyCatalogRules(selected.value)
  if (!result) return

  emit('applied', { created: result.created.length, skipped: result.skipped.length })
  selected.value = []
  close()
}
</script>

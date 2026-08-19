<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 820"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold pt-4 px-6">Regras Pré-configuradas</v-card-title>
      <v-card-subtitle class="px-6 pb-3">
        <span v-if="scopeLabel">
          Escopo: <strong>{{ scopeLabel }}</strong
          >. As regras criadas ficam vinculadas a este dispositivo.
        </span>
        <span v-else>Marque as regras que deseja habilitar.</span>
        As já configuradas aparecem bloqueadas e não são criadas novamente; as que o equipamento
        escolhido não sabe medir ficam indisponíveis.
      </v-card-subtitle>

      <!--
        O escopo é escolhido **aqui**, com o catálogo à vista. Pedi-lo antes,
        num diálogo separado, escondia a lista atrás de uma decisão que o
        operador só consegue tomar depois de ver as opções — e não oferecia
        "todos", que é o escopo certo para indisponibilidade e latência.
      -->
      <div v-if="allowScopeChoice" class="px-6 pb-3">
        <v-select
          v-model="escopoEscolhido"
          :items="opcoesDeDispositivo"
          item-title="title"
          item-value="value"
          label="Aplicar a"
          density="comfortable"
          variant="outlined"
          hide-details
        />
      </div>
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
                :disabled="bloqueado(template)"
                @click="toggle(template)"
              >
                <template #prepend>
                  <v-checkbox-btn
                    :model-value="template.applied || selected.includes(template.key)"
                    :disabled="bloqueado(template)"
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
                  <v-chip
                    v-else-if="template.applicable === false"
                    size="x-small"
                    variant="outlined"
                    color="grey"
                  >
                    Indisponível neste equipamento
                  </v-chip>
                </v-list-item-title>

                <v-list-item-subtitle class="text-wrap">
                  {{ template.description }}
                </v-list-item-subtitle>
                <div class="text-caption text-grey-darken-1 mt-1">
                  {{
                    describeRule(
                      template.condition,
                      template.durationSeconds,
                      template.recoveryWindowSeconds ?? 0,
                      template.flapThreshold ?? 0,
                      template.flapWindowSeconds ?? 900,
                      {
                        notificationCooldownSeconds: template.notificationCooldownSeconds ?? 0,
                        inhibitWhenParentDown: template.inhibitWhenParentDown ?? false,
                      }
                    )
                  }}
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
import { useAlertsStore, type AlertRuleScope, type AlertRuleTemplate } from '@/stores/alerts'
import { useDevicesStore } from '@/stores/devices'
import { describeRule, severityColor, severityLabel } from '@/utils/alertPresentation'

/**
 * O **mesmo** dialogo nas duas telas.
 *
 * `scope` ausente = catalogo global, aberto por `/alerts`. Com `scope`, o
 * dispositivo ja vem fixado — e o que acontece ao abrir pela pagina do
 * equipamento — e os templates que ele nao sabe publicar aparecem
 * desabilitados, com o motivo, em vez de sumirem: esconder faria o operador
 * procurar uma regra que existe e nao entender por que nao a encontra.
 */
const props = defineProps<{
  modelValue: boolean
  /** Escopo fixo, quando o diálogo é aberto de dentro de um dispositivo. */
  scope?: AlertRuleScope
  /** Nome do dispositivo do escopo, para o subtítulo. */
  scopeLabel?: string
  /**
   * Deixa o operador escolher o escopo aqui dentro, incluindo "todos".
   * É como `/alerts` abre o catálogo; a página do dispositivo passa `scope` e
   * não oferece a escolha.
   */
  allowScopeChoice?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'applied', summary: { created: number; skipped: number }): void
}>()

const alertsStore = useAlertsStore()
const devicesStore = useDevicesStore()
const selected = ref<string[]>([])

/** `null` = todos os dispositivos, que é o padrão ao abrir por `/alerts`. */
const escopoEscolhido = ref<number | null>(null)

const opcoesDeDispositivo = computed(() => [
  { title: 'Todos os dispositivos', value: null },
  ...devicesStore.devices.map((device) => ({ title: device.name, value: device.id })),
])

/**
 * O escopo efetivo: o fixo, quando há; senão o escolhido aqui.
 *
 * Um `undefined` significa catálogo global — e é diferente de
 * `{ deviceId: null }`, que o backend leria como um campo informado.
 */
const escopoEfetivo = computed<AlertRuleScope | undefined>(() => {
  if (props.scope) return props.scope
  return escopoEscolhido.value == null ? undefined : { deviceId: escopoEscolhido.value }
})

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
  alertsStore.ruleTemplates.filter((template) => template.recommended && !bloqueado(template))
)

watch(
  () => props.modelValue,
  async (isOpen) => {
    if (!isOpen) return
    selected.value = []
    if (props.allowScopeChoice) {
      escopoEscolhido.value = null
      if (devicesStore.devices.length === 0) void devicesStore.fetchDevices()
    }
    await alertsStore.fetchRuleCatalog(escopoEfetivo.value)
  }
)

// Trocar o dispositivo recarrega o catálogo: "já configurada" e "indisponível
// neste equipamento" são respostas **por escopo**, não propriedades do
// template.
watch(escopoEscolhido, async () => {
  if (!props.modelValue || !props.allowScopeChoice) return
  selected.value = []
  await alertsStore.fetchRuleCatalog(escopoEfetivo.value)
})

/**
 * Um template que nao pode ser marcado.
 *
 * Ou ja existe para este escopo, ou o dispositivo nao publica o campo que a
 * condicao compara — e nesse caso a regra nasceria sem nunca disparar.
 */
function bloqueado(template: AlertRuleTemplate): boolean {
  return template.applied || template.applicable === false
}

function toggle(template: AlertRuleTemplate) {
  if (bloqueado(template)) return
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
  const result = await alertsStore.applyCatalogRules(selected.value, escopoEfetivo.value)
  if (!result) return

  emit('applied', { created: result.created.length, skipped: result.skipped.length })
  selected.value = []
  close()
}
</script>

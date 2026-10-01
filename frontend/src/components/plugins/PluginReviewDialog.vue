<template>
  <v-dialog :model-value="modelValue" max-width="760" scrollable @update:model-value="close">
    <v-card v-if="detail" class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2 flex-wrap">
        <v-icon :color="risk.color">{{ risk.icon }}</v-icon>
        Revisão de segurança — {{ detail.summary.name }}
        <v-chip :color="risk.color" variant="flat" size="small">Risco {{ risk.label }}</v-chip>
      </v-card-title>
      <v-card-subtitle>
        v{{ detail.summary.version }} · revisado {{ formatRelativeTime(review?.reviewedAt) }}
      </v-card-subtitle>

      <v-card-text>
        <v-alert v-if="!review" type="warning" variant="tonal" class="mb-4">
          Este plugin ainda não foi revisado. Rode a revisão antes de instalar.
        </v-alert>

        <template v-else>
          <v-alert
            v-if="review.ai"
            :type="review.ai.mismatchWithUsage ? 'error' : 'info'"
            variant="tonal"
            class="mb-4"
          >
            <div class="font-weight-bold mb-1">
              Revisão pela IA{{ review.ai.model ? ` (${review.ai.model})` : '' }}
            </div>
            <div class="text-body-2">{{ review.ai.summary }}</div>
            <div v-if="review.ai.mismatchWithUsage" class="text-body-2 font-weight-bold mt-2">
              O script faz algo que a documentação de uso não descreve.
            </div>
          </v-alert>
          <v-alert v-else type="warning" variant="tonal" class="mb-4">
            <div class="font-weight-bold mb-1">Revisão por IA indisponível</div>
            <div class="text-body-2">{{ review.aiError }}</div>
            <div class="text-body-2 mt-1">
              Só a análise estática foi feita. Leia o script antes de instalar.
            </div>
          </v-alert>

          <div class="text-subtitle-2 font-weight-bold mb-2">Achados ({{ findings.length }})</div>
          <div v-if="findings.length === 0" class="text-body-2 mb-4">
            Nenhum padrão perigoso encontrado.
          </div>
          <v-list v-else density="compact" border class="rounded-lg mb-4">
            <v-list-item v-for="(finding, index) in findings" :key="index">
              <template #prepend>
                <v-icon :color="severityPresentation(finding.severity).color">
                  {{ severityPresentation(finding.severity).icon }}
                </v-icon>
              </template>
              <v-list-item-title class="text-wrap">{{ finding.message }}</v-list-item-title>
              <v-list-item-subtitle class="text-wrap">
                {{ severityPresentation(finding.severity).label }}
                {{ finding.line ? `· linha ${finding.line}` : '' }}
                {{ finding.rule === 'ai' ? '· IA' : `· ${finding.rule}` }}
              </v-list-item-subtitle>
              <code v-if="finding.excerpt" class="d-block font-mono text-body-small mt-1">{{
                finding.excerpt
              }}</code>
            </v-list-item>
          </v-list>

          <v-alert v-if="blocked" type="error" variant="flat" class="mb-2">
            Risco crítico: a instalação está bloqueada. Edite o plugin para remover o problema e
            revise de novo.
          </v-alert>
          <v-checkbox
            v-else-if="needsAcknowledgement && quarantined"
            v-model="acknowledge"
            color="error"
            hide-details
            label="Revisei os riscos apontados e quero instalar este plugin como rascunho"
          ></v-checkbox>
        </template>
      </v-card-text>

      <v-card-actions class="flex-wrap">
        <v-btn
          variant="tonal"
          color="primary"
          prepend-icon="mdi-shield-refresh-outline"
          :loading="busy === 'review'"
          @click="emit('review')"
        >
          Revisar de novo
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn variant="text" color="secondary" @click="close(false)">Fechar</v-btn>
        <v-btn
          v-if="quarantined"
          color="warning"
          variant="flat"
          prepend-icon="mdi-shield-check-outline"
          :disabled="!review || blocked || (needsAcknowledgement && !acknowledge)"
          :loading="busy === 'accept'"
          @click="emit('accept', acknowledge)"
        >
          Instalar como rascunho
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { PluginDetail } from '@/bindings/PluginDetail'
import { formatRelativeTime } from '@/utils/formatters'
import { severityPresentation } from '@/utils/pluginPresentation'

const props = defineProps<{
  modelValue: boolean
  detail: PluginDetail | null
  busy?: 'review' | 'accept' | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  review: []
  accept: [acknowledge: boolean]
}>()

const acknowledge = ref(false)
const review = computed(() => props.detail?.review ?? null)
const findings = computed(() => [
  ...(review.value?.findings ?? []),
  ...(review.value?.ai?.findings ?? []),
])
const risk = computed(() => severityPresentation(review.value?.risk ?? 'medium'))
const blocked = computed(() => review.value?.risk === 'critical')
const needsAcknowledgement = computed(
  () => !review.value?.ai || review.value.risk === 'high' || review.value.risk === 'critical'
)
const quarantined = computed(() => props.detail?.summary.status === 'quarantine')

watch(
  () => props.modelValue,
  () => {
    acknowledge.value = false
  }
)

function close(value = false) {
  emit('update:modelValue', value)
}
</script>

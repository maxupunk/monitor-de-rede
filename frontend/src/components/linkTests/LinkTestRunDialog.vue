<template>
  <v-dialog
    :model-value="modelValue"
    :fullscreen="smAndDown || maximized"
    max-width="1100"
    scrollable
    @update:model-value="(open) => !open && dismiss()"
  >
    <v-card v-if="test" class="run-dialog" :rounded="smAndDown ? 0 : 'lg'">
      <header class="run-header">
        <div class="d-flex align-start ga-3">
          <v-avatar :color="kind.color" size="44" rounded="lg">
            <v-icon color="white">{{ kind.icon }}</v-icon>
          </v-avatar>
          <div class="flex-grow-1 overflow-hidden">
            <h2
              class="font-weight-bold title"
              :class="smAndDown ? 'text-title-medium' : 'text-title-large'"
            >
              {{ routeTitle }}
            </h2>
            <div class="d-flex align-center flex-wrap ga-2 mt-1">
              <v-chip :color="kind.color" size="small" variant="flat" label>{{
                kind.label
              }}</v-chip>
              <LinkTestStatusChip :status="test.status" />
              <span class="text-body-small">{{ timing }}</span>
            </div>
          </div>
          <div class="d-flex align-center flex-wrap justify-end ga-1 flex-shrink-0">
            <slot name="actions" :test="test"></slot>
            <v-tooltip v-if="active" :text="READ_ONLY_REASON" :disabled="auth.canWrite">
              <template #activator="{ props: tip }">
                <span v-bind="tip">
                  <v-btn
                    color="error"
                    variant="flat"
                    size="small"
                    prepend-icon="mdi-stop"
                    :loading="cancelling"
                    :disabled="!auth.canWrite"
                    @click="cancel"
                  >
                    Cancelar teste
                  </v-btn>
                </span>
              </template>
            </v-tooltip>
            <MaximizeToggle v-if="!smAndDown" v-model="maximized" />
            <v-btn
              v-if="minimizable"
              icon="mdi-window-minimize"
              variant="text"
              size="small"
              aria-label="Minimizar — o teste continua e fica no mapa"
              title="Minimizar — o teste continua e fica no mapa"
              @click="$emit('minimize')"
            ></v-btn>
            <v-btn
              icon="mdi-close"
              variant="text"
              size="small"
              :aria-label="active ? 'Fechar a tela (o teste continua)' : 'Fechar'"
              @click="dismiss"
            ></v-btn>
          </div>
        </div>
        <v-progress-linear
          v-if="active"
          :model-value="progress * 100"
          color="primary"
          height="6"
          rounded
          class="mt-3"
          :aria-label="`Andamento do teste: ${Math.round(progress * 100)}%`"
        ></v-progress-linear>
        <v-alert v-if="cancelError" type="error" variant="tonal" density="compact" class="mt-3">
          {{ cancelError }}
        </v-alert>
      </header>

      <v-card-text class="run-body">
        <LinkTestRunView :test="test" @requested="dismiss" />
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * A tela do teste de velocidade: em tela cheia (no celular, a tela toda),
 * com o caminho animado, a medição e o resultado (`LinkTestRunView`).
 * "Minimizar" devolve o mapa e deixa a pílula do teste nele; "Fechar" sai.
 * "Cancelar teste" fica sempre à vista enquanto mede.
 */
import { computed, ref } from 'vue'
import { useDisplay } from 'vuetify'
import { confirm } from '@/composables/useConfirm'
import { useLinkTestContext } from '@/composables/linkTests/useLinkTestContext'
import { useAuthStore } from '@/stores/auth'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTest } from '@/types/linkTests'
import { formatElapsedMs, formatShortDateTime } from '@/utils/formatters'
import { TEST_KINDS, isActiveStatus, testRouteTitle } from '@/utils/linkTests/presentation'
import { testElapsedMs, testProgress, testRemainingMs } from '@/utils/linkTests/progress'
import { READ_ONLY_REASON } from '@/utils/topology/inspectorRegistry'
import MaximizeToggle from '@/components/MaximizeToggle.vue'
import LinkTestRunView from './LinkTestRunView.vue'
import LinkTestStatusChip from './LinkTestStatusChip.vue'

const props = defineProps<{
  modelValue: boolean
  test: LinkTest | null
  /** Na topologia: minimizar deixa a pílula no mapa. */
  minimizable?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  minimize: []
  close: []
}>()

const { smAndDown } = useDisplay()
const auth = useAuthStore()
const store = useLinkTestsStore()
const ctx = useLinkTestContext()
/** Tela cheia no PC (o pedido do dono); "Restaurar" vira janela. */
const maximized = ref(true)
const cancelling = ref(false)
const cancelError = ref<string | null>(null)

const kind = computed(() => TEST_KINDS[props.test?.kind ?? 'link'] ?? TEST_KINDS.link)
const routeTitle = computed(() => (props.test ? testRouteTitle(props.test) : ''))
const active = computed(() => (props.test ? isActiveStatus(props.test.status) : false))
const now = computed(() => (props.test ? ctx.nowFor(props.test) : Date.now()))
const progress = computed(() => (props.test ? testProgress(props.test, now.value) : 0))

const timing = computed(() => {
  const test = props.test
  if (!test) return ''
  if (test.status === 'queued') return 'aguardando a vez'
  const elapsed = testElapsedMs(test, now.value)
  if (active.value) {
    const remaining = testRemainingMs(test, now.value)
    const parts = [`${formatElapsedMs(elapsed)} decorridos`]
    if (remaining !== null && remaining > 0) parts.push(`faltam ~${formatElapsedMs(remaining)}`)
    return parts.join(' · ')
  }
  // Hora absoluta: a tela não tem relógio próprio, e um "há 0s" congelaria.
  const when = test.finishedAt ?? test.createdAt
  return `terminou em ${formatShortDateTime(when)}${elapsed !== null ? ` · durou ${formatElapsedMs(elapsed)}` : ''}`
})

function dismiss() {
  emit('update:modelValue', false)
  emit('close')
}

async function cancel() {
  const test = props.test
  if (!test) return
  const ok = await confirm({
    title: 'Cancelar o teste',
    message:
      'Interromper a medição agora? O que já foi medido fica registrado e os servidores de teste são desligados nos equipamentos.',
    confirmText: 'Cancelar teste',
    cancelText: 'Continuar medindo',
    confirmColor: 'error',
  })
  if (!ok) return
  cancelling.value = true
  cancelError.value = null
  try {
    await store.cancel(test.id)
  } catch (err: unknown) {
    cancelError.value =
      err instanceof Error && err.message ? err.message : 'Não foi possível cancelar o teste.'
  } finally {
    cancelling.value = false
  }
}
</script>

<style scoped>
.run-dialog {
  color: rgb(var(--v-theme-on-surface));
}
.run-header {
  padding: 16px 20px 12px;
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.14);
}
.title {
  word-break: break-word;
  line-height: 1.25;
}
.run-body {
  padding: 16px 20px 20px;
}
</style>

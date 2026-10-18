<template>
  <section
    v-if="items.length > 0"
    class="fix-list mb-3"
    :class="{ complete: allDone }"
    aria-labelledby="fix-list-title"
  >
    <div class="d-flex align-center ga-2 mb-2">
      <v-icon :color="allDone ? 'success' : 'warning'">
        {{ allDone ? 'mdi-check-circle' : 'mdi-format-list-checks' }}
      </v-icon>
      <h3 id="fix-list-title" class="text-title-small font-weight-bold flex-grow-1">
        {{ allDone ? 'Pronto: o que faltava foi feito' : 'Antes de testar, falta' }}
      </h3>
      <v-progress-circular
        v-if="refreshing"
        indeterminate
        color="primary"
        size="18"
        width="2"
        aria-label="Refazendo o plano"
      ></v-progress-circular>
    </div>
    <ul class="fixes" aria-live="polite">
      <li v-for="item in items" :key="item.key" class="fix" :class="item.state">
        <div class="fix-main">
          <span class="fix-state" aria-hidden="true">
            <v-progress-circular
              v-if="item.state === 'working'"
              indeterminate
              color="primary"
              size="20"
              width="2"
            ></v-progress-circular>
            <v-icon v-else-if="item.state === 'done'" color="success">mdi-check-circle</v-icon>
            <v-icon v-else-if="item.state === 'error'" color="error">mdi-alert-circle</v-icon>
            <v-icon v-else color="warning">mdi-checkbox-blank-circle-outline</v-icon>
          </span>
          <div class="fix-text">
            <div class="font-weight-bold">{{ item.label }}</div>
            <div v-if="item.state === 'error'" class="text-body-small text-error">
              {{ item.error }}
            </div>
            <div v-else-if="item.state === 'done'" class="text-body-small">Feito.</div>
          </div>
        </div>
        <template v-if="item.state !== 'done'">
          <PluginInstallButton
            v-if="item.fix.kind === 'install_plugin'"
            :device-id="item.fix.deviceId"
            :plugin-id="item.fix.pluginId"
            :plugin-name="item.fix.pluginName"
            :disabled="refreshing"
            @started="setState(item.fix, 'working')"
            @installed="finish(item.fix)"
            @failed="(message) => fail(item.fix, message)"
          />
          <DeviceAccessButton
            v-else
            :device-id="item.fix.deviceId"
            :transport="item.fix.transport"
            :reason="item.fix.message"
            label="Cadastrar acesso"
            :disabled="refreshing"
            @saved="finish(item.fix)"
          />
        </template>
      </li>
    </ul>
  </section>
</template>

<script setup lang="ts">
/**
 * "Antes de testar, falta": os passos do plano (instalar o plugin, cadastrar
 * o acesso), cada um com o botão que o resolve ali mesmo. Feito o passo, o
 * assistente refaz o plano (`fixed`); o feito fica marcado na lista.
 */
import { computed, reactive } from 'vue'
import type { LinkTestFix } from '@/types/linkTests'
import { fixKey, fixLabel } from '@/utils/linkTests/wizard'
import DeviceAccessButton from '@/components/devices/access/DeviceAccessButton.vue'
import PluginInstallButton from '@/components/plugins/PluginInstallButton.vue'

type FixState = 'pending' | 'working' | 'done' | 'error'

const props = defineProps<{
  fixes: LinkTestFix[]
  /** O plano está sendo refeito depois de um passo. */
  refreshing?: boolean
}>()

const emit = defineEmits<{ fixed: [fix: LinkTestFix] }>()

const states = reactive<Record<string, { state: FixState; error: string }>>({})
/** Os feitos, para continuarem marcados depois que o plano novo os tira. */
const completed = reactive<LinkTestFix[]>([])

const items = computed(() => {
  const pending = props.fixes.filter(
    (fix) => !completed.some((done) => fixKey(done) === fixKey(fix))
  )
  return [...completed, ...pending].map((fix) => {
    const key = fixKey(fix)
    const current = states[key]
    return {
      key,
      fix,
      label: fixLabel(fix),
      state: current?.state ?? 'pending',
      error: current?.error ?? '',
    }
  })
})

const allDone = computed(
  () => items.value.length > 0 && items.value.every((item) => item.state === 'done')
)

function setState(fix: LinkTestFix, state: FixState, error = '') {
  states[fixKey(fix)] = { state, error }
}

function finish(fix: LinkTestFix) {
  setState(fix, 'done')
  completed.push(fix)
  emit('fixed', fix)
}

function fail(fix: LinkTestFix, message: string) {
  setState(fix, 'error', message)
}
</script>

<style scoped>
.fix-list {
  padding: 12px;
  border: 1px solid rgb(var(--v-theme-warning));
  border-radius: 10px;
  color: rgb(var(--v-theme-on-surface));
}
.fix-list.complete {
  border-color: rgb(var(--v-theme-success));
}
.fixes {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.fix {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  border-radius: 8px;
}
.fix.done {
  border-color: rgb(var(--v-theme-success));
}
.fix.error {
  border-color: rgb(var(--v-theme-error));
}
.fix-main {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  flex: 1 1 240px;
  min-width: 0;
}
.fix-text {
  min-width: 0;
  overflow-wrap: anywhere;
}
.fix-state {
  display: inline-flex;
  width: 24px;
  justify-content: center;
}
</style>

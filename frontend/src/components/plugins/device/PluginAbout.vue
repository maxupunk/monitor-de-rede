<template>
  <div>
    <v-progress-linear v-if="!detail" indeterminate color="primary" />
    <template v-else>
      <v-tabs v-model="tab" density="compact" color="primary">
        <v-tab value="usage">Como usar</v-tab>
        <v-tab value="compat"> Compatibilidade ({{ detail.package.compatibility.length }}) </v-tab>
        <v-tab value="tests">Testes</v-tab>
      </v-tabs>
      <v-window v-model="tab" class="pt-3">
        <v-window-item value="usage">
          <div
            class="markdown-body text-body-2"
            v-html="renderMarkdown(detail.package.usage, { headings: true })"
          ></div>
        </v-window-item>
        <v-window-item value="compat">
          <div v-if="detail.package.compatibility.length === 0" class="text-body-2">
            Ainda não validado em nenhum equipamento.
          </div>
          <v-table v-else density="compact" class="border rounded-lg">
            <thead>
              <tr>
                <th>Sistema</th>
                <th>Modelo</th>
                <th>Firmware</th>
                <th>Resultado</th>
                <th>Quando</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(entry, index) in detail.package.compatibility" :key="index">
                <td>{{ entry.platform ?? '—' }}</td>
                <td>{{ entry.model ?? '—' }}</td>
                <td>{{ entry.firmware ?? '—' }}</td>
                <td>
                  <v-chip
                    size="x-small"
                    :color="entry.status === 'passed' ? 'success' : 'error'"
                    variant="flat"
                  >
                    {{ entry.status === 'passed' ? 'Passou' : 'Falhou' }}
                  </v-chip>
                </td>
                <td>{{ formatRelativeTime(entry.validatedAt) }}</td>
              </tr>
            </tbody>
          </v-table>
        </v-window-item>
        <v-window-item value="tests">
          <div class="d-flex align-center flex-wrap ga-2 mb-2">
            <span class="text-body-2">
              {{ detail.package.tests.unit.length }} unitário(s) ·
              {{ detail.package.tests.functional.length }} funcional(is)
            </span>
            <v-chip
              v-if="lastTestOk !== null"
              size="x-small"
              :color="lastTestOk ? 'success' : 'error'"
              variant="flat"
            >
              {{ lastTestOk ? 'Último teste passou' : 'Último teste falhou' }}
            </v-chip>
            <v-spacer></v-spacer>
            <v-btn
              size="small"
              color="info"
              variant="flat"
              prepend-icon="mdi-test-tube"
              :loading="testing"
              @click="runTests"
            >
              Rodar testes unitários
            </v-btn>
          </div>
          <v-alert
            v-if="report"
            :type="report.passed ? 'success' : 'error'"
            variant="tonal"
            density="compact"
          >
            <div v-for="problem in report.problems" :key="problem">{{ problem }}</div>
            <div v-for="testCase in report.cases" :key="testCase.name" class="text-body-2">
              <v-icon size="14" :color="testCase.passed ? 'success' : 'error'">
                {{ testCase.passed ? 'mdi-check' : 'mdi-close' }}
              </v-icon>
              {{ testCase.name }}{{ testCase.message ? ` — ${testCase.message}` : '' }}
            </div>
          </v-alert>
        </v-window-item>
      </v-window>
    </template>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { PluginDetail } from '@/bindings/PluginDetail'
import type { TestReport } from '@/bindings/TestReport'
import { usePluginsStore } from '@/stores/plugins'
import { formatRelativeTime } from '@/utils/formatters'
import { renderMarkdown } from '@/utils/markdown'

const props = defineProps<{
  pluginId: number
  /** O equipamento cuja aba é atualizada depois dos testes, quando há um. */
  deviceId?: number | null
  lastTestOk: boolean | null
}>()

const emit = defineEmits<{ error: [message: string] }>()

const store = usePluginsStore()
const tab = ref('usage')
const detail = ref<PluginDetail | null>(null)
const report = ref<TestReport | null>(null)
const testing = ref(false)

onMounted(async () => {
  try {
    detail.value = await store.fetchDetail(props.pluginId)
  } catch (err: unknown) {
    emit('error', err instanceof Error ? err.message : 'Falha ao carregar o plugin')
  }
})

async function runTests() {
  testing.value = true
  try {
    report.value = await store.runTests(props.pluginId, props.deviceId)
  } catch (err: unknown) {
    emit('error', err instanceof Error ? err.message : 'Falha ao rodar os testes')
  } finally {
    testing.value = false
  }
}
</script>

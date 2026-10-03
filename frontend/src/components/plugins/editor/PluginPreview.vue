<template>
  <div class="d-flex flex-column ga-4">
    <div class="d-flex align-center flex-wrap ga-3">
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-eye-refresh-outline"
        :loading="loading"
        :disabled="!pkg"
        @click="refresh"
      >
        Atualizar prévia
      </v-btn>
      <span class="text-body-2">
        A tela desenhada com as respostas gravadas nos testes — nada vai ao equipamento e nada é
        salvo.
      </span>
    </div>

    <v-alert v-if="!pkg" type="warning" variant="tonal">
      Corrija o JSON do manifesto e dos testes para ver a prévia.
    </v-alert>
    <v-alert v-if="error" type="error" variant="tonal">{{ error }}</v-alert>
    <v-alert v-if="loading" type="info" variant="tonal" icon="mdi-test-tube">
      Rodando {{ testCount }} teste(s) do plugin no sandbox para desenhar a tela — leva alguns
      segundos.
      <v-progress-linear indeterminate color="primary" class="mt-2" />
    </v-alert>

    <template v-if="preview">
      <!-- Problemas e sugestões -->
      <v-alert
        v-if="report && (report.problems.length > 0 || failed.length > 0)"
        type="error"
        variant="tonal"
        title="Antes da tela, o pacote precisa passar"
      >
        <ul class="ml-4 text-body-2">
          <li v-for="problem in report.problems" :key="problem">{{ problem }}</li>
          <li v-for="item in failed" :key="item.name">
            <strong>{{ item.name }}</strong
            >: {{ item.message }}
          </li>
        </ul>
      </v-alert>
      <v-alert
        v-if="hints.length > 0"
        type="warning"
        variant="tonal"
        icon="mdi-lightbulb-on-outline"
        :title="`Sugestões de usabilidade (${hints.length})`"
      >
        <ul class="ml-4 text-body-2">
          <li v-for="hint in hints" :key="hint.at + hint.message">
            <code>{{ hint.at }}</code> — {{ hint.message }}
          </li>
        </ul>
      </v-alert>
      <v-alert
        v-else-if="report?.passed"
        type="success"
        variant="tonal"
        icon="mdi-check-decagram-outline"
      >
        Nenhuma sugestão: a tela segue o padrão do sistema.
      </v-alert>

      <!-- A lista de itens, como a aba e a página vão mostrar -->
      <section v-if="preview.list">
        <div class="text-subtitle-1 font-weight-bold mb-2">Aba do equipamento</div>
        <v-card border flat class="rounded-lg pa-4">
          <PluginItemList
            :list="preview.list"
            :rows="deviceListRows"
            :actions="deviceActions"
            :presentation="actionOf(preview.list.source)"
            can-write
            @add="inert"
            @edit="inert"
            @remove="inert"
            @action="inert"
            @toolbar="inert"
            @search="inert"
          />
        </v-card>
      </section>
      <section v-if="preview.fleetList">
        <div class="text-subtitle-1 font-weight-bold mb-2">Página da frota</div>
        <v-card border flat class="rounded-lg pa-4">
          <PluginItemList
            :list="preview.fleetList"
            :rows="fleetListRows"
            :actions="fleetActions"
            :members="fleet.members.length"
            :presentation="actionOf(manifest?.fleet?.statusAction)"
            can-write
            @add="inert"
            @edit="inert"
            @remove="inert"
            @action="inert"
            @toolbar="inert"
          />
          <div class="text-body-2 mt-2">
            Cada teste de “{{ actionOf(manifest?.fleet?.statusAction)?.title }}” aparece como um
            equipamento.
          </div>
        </v-card>
      </section>

      <!-- Formulários -->
      <section v-if="forms.length > 0">
        <div class="text-subtitle-1 font-weight-bold mb-2">Formulários</div>
        <v-card border flat class="rounded-lg pa-4">
          <v-select
            v-model="formActionId"
            :items="forms"
            item-title="title"
            item-value="id"
            label="Ação"
            variant="outlined"
            density="comfortable"
            prepend-inner-icon="mdi-form-select"
          ></v-select>
          <template v-if="formAction">
            <p v-if="formAction.description" class="text-body-2 mb-3">
              {{ formAction.description }}
            </p>
            <SettingsForm v-model="formValues" :schema="formAction.params ?? {}" />
            <div class="text-caption font-weight-bold mt-3">O que o script recebe</div>
            <pre class="sent pa-3 rounded-lg font-mono text-body-small">{{ sent }}</pre>
          </template>
        </v-card>
      </section>

      <!-- Resultados -->
      <section v-if="outputs.length > 0">
        <div class="text-subtitle-1 font-weight-bold mb-2">Resultados</div>
        <v-expansion-panels variant="accordion">
          <v-expansion-panel v-for="item in outputs" :key="item.name">
            <v-expansion-panel-title>
              <v-icon start color="primary">mdi-text-box-check-outline</v-icon>
              <span class="font-weight-bold mr-2">{{
                testedAction(manifest, item.action)?.title ?? item.action
              }}</span>
              <span class="text-body-2">{{ item.name }}</span>
            </v-expansion-panel-title>
            <v-expansion-panel-text>
              <PluginOutput
                :output="item.output"
                :kind="actionOf(item.action)?.output ?? 'report'"
                :presentation="testedAction(manifest, item.action)"
              />
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>
      </section>
    </template>

    <v-snackbar v-model="notice" color="info" timeout="2500">
      Na prévia os botões não executam: aqui só se vê a tela.
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginManifest } from '@/bindings/PluginManifest'
import type { PluginPackage } from '@/bindings/PluginPackage'
import type { PluginPreview } from '@/bindings/PluginPreview'
import PluginOutput from '@/components/plugins/PluginOutput.vue'
import PluginItemList from '@/components/plugins/items/PluginItemList.vue'
import SettingsForm from '@/components/plugins/settings/SettingsForm.vue'
import { usePluginsStore } from '@/stores/plugins'
import { deviceRows, fleetRows, type ListActionInfo } from '@/utils/itemList'
import {
  actionsWithForm,
  firstOutputOf,
  previewFleet,
  previewOutputs,
  testedAction,
} from '@/utils/pluginPreview'
import { defaultsOf, paramsOf } from '@/utils/pluginSettings'

/**
 * A aba "Prévia" do editor: a tela do plugin desenhada com os componentes de
 * verdade e as saídas dos testes — o autor (ou a IA) vê a lista, os
 * formulários e os resultados antes de instalar.
 */
const props = defineProps<{ pkg: PluginPackage | null }>()

const store = usePluginsStore()
const preview = ref<PluginPreview | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const notice = ref(false)
const formActionId = ref<string | null>(null)
const formValues = ref<Record<string, unknown>>({})

const manifest = computed<PluginManifest | null>(() => props.pkg?.manifest ?? null)
const testCount = computed(() => props.pkg?.tests.unit.length ?? 0)
const report = computed(() => preview.value?.report ?? null)
const failed = computed(() => (report.value?.cases ?? []).filter((item) => !item.passed))
const hints = computed(() => report.value?.hints ?? [])
const outputs = computed(() => previewOutputs(report.value))
const actions = computed(() => manifest.value?.actions ?? [])
const forms = computed(() => actionsWithForm(manifest.value))
const formAction = computed(() => actionOf(formActionId.value))
const sent = computed(() =>
  JSON.stringify(paramsOf(formAction.value?.params ?? {}, formValues.value), null, 2)
)

function actionOf(id: string | null | undefined): PluginAction | null {
  return actions.value.find((action) => action.id === id) ?? null
}

const deviceActions = computed<ListActionInfo[]>(() =>
  actions.value.map((action) => ({ id: action.id, title: action.title, effect: action.effect }))
)
const fleetActions = computed<ListActionInfo[]>(() =>
  (manifest.value?.fleet?.actions ?? []).map((action) => ({
    id: action.id,
    title: action.title,
    icon: action.icon,
    effect: actionOf(action.action)?.effect ?? 'read',
  }))
)
const deviceListRows = computed(() =>
  preview.value?.list
    ? deviceRows(firstOutputOf(report.value, preview.value.list.source), preview.value.list)
    : []
)
const fleet = computed(() => previewFleet(report.value, manifest.value?.fleet?.statusAction))
const fleetListRows = computed(() =>
  preview.value?.fleetList
    ? fleetRows(fleet.value.outputs, preview.value.fleetList, fleet.value.members)
    : []
)

function inert() {
  notice.value = true
}

async function refresh() {
  if (!props.pkg) return
  loading.value = true
  error.value = null
  try {
    preview.value = await store.previewPackage(props.pkg)
  } catch (err: unknown) {
    error.value = err instanceof Error ? err.message : 'Falha ao montar a prévia'
  } finally {
    loading.value = false
  }
}

// Ação nova no seletor: o formulário começa dos padrões dela.
watch(
  formAction,
  (action) => {
    formValues.value = defaultsOf(action?.params ?? {})
  },
  { immediate: true }
)
watch(
  forms,
  (list) => {
    if (!list.some((action) => action.id === formActionId.value)) {
      formActionId.value = list[0]?.id ?? null
    }
  },
  { immediate: true }
)

defineExpose({ refresh })
</script>

<style scoped>
.sent {
  background-color: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  white-space: pre-wrap;
  max-height: 200px;
  overflow: auto;
}
</style>

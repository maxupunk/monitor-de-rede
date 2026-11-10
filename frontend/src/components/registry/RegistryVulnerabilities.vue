<template>
  <div>
    <v-card elevation="2" class="pa-4 mb-4">
      <div class="d-flex align-center ga-2 mb-3">
        <div>
          <div class="text-subtitle-1 font-weight-bold">Base de vulnerabilidades (OSV.dev)</div>
          <div class="text-body-2">
            Uma cópia local, só dos ecossistemas que aparecem nas imagens, atualizada uma vez por
            dia. A lista de pacotes das imagens nunca sai da central.
          </div>
        </div>
        <v-spacer></v-spacer>
        <v-btn
          v-if="canAdmin"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-refresh"
          :loading="syncing"
          @click="sync"
        >
          Atualizar agora
        </v-btn>
      </div>
      <v-alert
        v-if="syncNotice"
        :type="syncNotice.type"
        variant="tonal"
        density="compact"
        class="mb-3"
        closable
        @click:close="syncNotice = null"
        >{{ syncNotice.text }}</v-alert
      >
      <v-list v-if="feeds.length" density="compact" class="bg-transparent pa-0">
        <v-list-item
          v-for="feed in feeds"
          :key="feed.ecosystem"
          class="px-0"
          :title="feed.ecosystem"
          :subtitle="feedSubtitle(feed)"
        >
          <template #prepend>
            <v-icon :color="feed.error ? 'error' : 'success'" class="mr-3">
              {{ feed.error ? 'mdi-alert-circle' : 'mdi-database-check' }}
            </v-icon>
          </template>
        </v-list-item>
      </v-list>
      <div v-else class="text-body-1">
        Nenhum ecossistema ainda: a base é baixada na primeira análise de uma imagem.
      </div>
    </v-card>

    <v-card elevation="2" class="pa-4">
      <div class="d-flex align-center ga-2 mb-3">
        <div>
          <div class="text-subtitle-1 font-weight-bold">Exceções</div>
          <div class="text-body-2">
            Uma vulnerabilidade aceita por um tempo (sem correção ainda, ou fora do caminho usado):
            deixa de bloquear o deploy até vencer. Fica na auditoria com o motivo.
          </div>
        </div>
        <v-spacer></v-spacer>
        <v-btn
          v-if="canAdmin"
          color="primary"
          variant="flat"
          prepend-icon="mdi-plus"
          @click="openForm"
        >
          Nova exceção
        </v-btn>
      </div>
      <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">{{
        error
      }}</v-alert>
      <v-list v-if="exceptions.length" density="compact" class="bg-transparent pa-0">
        <v-list-item
          v-for="item in exceptions"
          :key="item.id"
          class="px-0"
          :title="item.advisory"
          :subtitle="`${item.application ?? 'Todas as aplicações'} · ${item.reason} · ${item.expired ? 'vencida' : `vale até ${formatDate(item.expiresAt)}`} · ${item.createdBy}`"
        >
          <template #prepend>
            <v-icon :color="item.expired ? 'secondary' : 'info'" class="mr-3"
              >mdi-shield-check</v-icon
            >
          </template>
          <template v-if="canAdmin" #append>
            <IconAction
              icon="mdi-delete"
              label="Remover exceção"
              color="error"
              @click="remove(item.id)"
            />
          </template>
        </v-list-item>
      </v-list>
      <div v-else class="text-body-1">Nenhuma exceção.</div>
    </v-card>

    <v-dialog v-model="formOpen" max-width="560">
      <v-card>
        <v-card-title class="px-6 pt-5 font-weight-bold">Nova exceção</v-card-title>
        <v-card-text class="px-6">
          <v-form ref="formRef" @submit.prevent="save">
            <v-text-field
              v-model="form.advisory"
              label="Identificador *"
              placeholder="CVE-2026-1234, GHSA-…, DSA-…"
              variant="outlined"
              :rules="[requiredRule('Informe o identificador')]"
              class="mb-2"
            ></v-text-field>
            <v-select
              v-model="form.applicationId"
              :items="applicationItems"
              item-title="title"
              item-value="value"
              label="Aplicação"
              variant="outlined"
              class="mb-2"
            ></v-select>
            <v-textarea
              v-model="form.reason"
              label="Motivo *"
              placeholder="Ex.: sem correção do fornecedor; o recurso afetado não é usado"
              variant="outlined"
              rows="2"
              :rules="[
                (value: string) =>
                  value.trim().length >= 10 || 'Explique em ao menos 10 caracteres',
              ]"
              class="mb-2"
            ></v-textarea>
            <v-select
              v-model="form.days"
              :items="[7, 30, 90, 180]"
              label="Vale por (dias)"
              variant="outlined"
            ></v-select>
          </v-form>
        </v-card-text>
        <v-card-actions class="px-6 pb-4">
          <v-spacer></v-spacer>
          <v-btn variant="text" color="primary" @click="formOpen = false">Cancelar</v-btn>
          <v-btn color="primary" variant="flat" :loading="saving" @click="save">Registrar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import IconAction from '@/components/IconAction.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { RegistryExceptionView } from '@/bindings/RegistryExceptionView'
import type { RegistryFeedView } from '@/bindings/RegistryFeedView'
import { useDeploysStore } from '@/stores/deploys'
import { ApiError } from '@/services/apiService'
import { useRegistryStore } from '@/stores/registry'
import { requiredRule } from '@/utils/formRules'
import { countNoun, formatDate, formatRelativeTime } from '@/utils/formatters'

defineProps<{ canAdmin: boolean }>()

const registry = useRegistryStore()
const deploys = useDeploysStore()
const { confirm } = useConfirm()
const exceptions = ref<RegistryExceptionView[]>([])
const error = ref<string | null>(null)
const syncing = ref(false)
const syncNotice = ref<{ type: 'info' | 'error'; text: string } | null>(null)
const saving = ref(false)
const formOpen = ref(false)
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
/** 0 é "todas" (o `v-select` não lida bem com `null`). */
const form = reactive({ advisory: '', applicationId: 0, reason: '', days: 30 })

const feeds = computed<RegistryFeedView[]>(() => registry.overview?.feeds ?? [])

const applicationItems = computed(() => [
  { title: 'Todas as aplicações', value: 0 },
  ...deploys.applications.map((application) => ({
    title: application.name,
    value: application.id,
  })),
])

function feedSubtitle(feed: RegistryFeedView): string {
  if (feed.error) return feed.error
  const synced = feed.syncedAt ? `atualizada ${formatRelativeTime(feed.syncedAt)}` : 'baixando…'
  return `${countNoun(feed.entries, 'aviso')} · ${synced}`
}

async function load() {
  try {
    exceptions.value = await registry.exceptions()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível ler as exceções'
  }
}

async function sync() {
  syncing.value = true
  try {
    await registry.syncAdvisories()
    syncNotice.value = {
      type: 'info',
      text: 'Atualização iniciada. A lista muda aqui sozinha quando terminar; as imagens já analisadas são reanalisadas se algo mudou.',
    }
  } catch (err) {
    syncNotice.value =
      err instanceof ApiError && err.status === 409
        ? { type: 'info', text: 'A base já está sendo atualizada: aguarde, a lista muda sozinha.' }
        : { type: 'error', text: err instanceof Error ? err.message : 'Não foi possível atualizar' }
  } finally {
    syncing.value = false
  }
}

function openForm() {
  form.advisory = ''
  form.applicationId = 0
  form.reason = ''
  form.days = 30
  if (!deploys.overview) void deploys.fetchOverview()
  formOpen.value = true
}

async function save() {
  const result = await formRef.value?.validate()
  if (result && !result.valid) return
  saving.value = true
  error.value = null
  try {
    await registry.addException({
      applicationId: form.applicationId === 0 ? null : form.applicationId,
      advisory: form.advisory.trim(),
      reason: form.reason.trim(),
      days: form.days,
    })
    formOpen.value = false
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível registrar'
  } finally {
    saving.value = false
  }
}

async function remove(id: number) {
  const ok = await confirm({
    title: 'Remover a exceção?',
    message: 'A vulnerabilidade volta a contar na política dos alvos.',
    confirmText: 'Remover',
    confirmColor: 'error',
    icon: 'mdi-delete',
  })
  if (!ok) return
  try {
    await registry.removeException(id)
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível remover'
  }
}

onMounted(load)
</script>

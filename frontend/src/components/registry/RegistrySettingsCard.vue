<template>
  <v-card elevation="2" class="pa-4">
    <div class="text-subtitle-1 font-weight-bold mb-1">Espaço e retenção</div>
    <div class="text-body-2 mb-4">
      Por imagem ficam as versões mais novas, mais toda versão no ar ou guardada para o rollback de
      algum alvo, mais as fixadas. A limpeza roda sozinha a cada 6 horas.
    </div>
    <v-alert v-if="message" :type="messageType" variant="tonal" density="compact" class="mb-3">
      {{ message }}
    </v-alert>
    <v-form ref="formRef" @submit.prevent="save">
      <v-row dense>
        <v-col cols="12" sm="6">
          <v-text-field
            v-model.number="form.keepVersions"
            type="number"
            label="Versões guardadas por imagem"
            variant="outlined"
            :rules="[(value: number) => (value >= 1 && value <= 200) || 'De 1 a 200']"
          ></v-text-field>
        </v-col>
        <v-col cols="12" sm="6">
          <v-text-field
            v-model.number="form.quotaGib"
            type="number"
            label="Cota total (GiB)"
            hint="Aviso aos 80%; acima de 95% o registry recusa imagens novas"
            persistent-hint
            variant="outlined"
            :rules="[(value: number) => (value >= 1 && value <= 10000) || 'De 1 a 10.000']"
          ></v-text-field>
        </v-col>
      </v-row>
      <div class="d-flex ga-2 mt-4">
        <v-btn color="primary" variant="flat" :loading="saving" @click="save">Salvar</v-btn>
        <v-spacer></v-spacer>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-broom"
          :loading="collecting"
          :disabled="dirty"
          :title="dirty ? 'Salve antes: a limpeza usa a configuração salva' : undefined"
          @click="collect"
        >
          Limpar agora
        </v-btn>
      </div>
    </v-form>
  </v-card>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useConfirm } from '@/composables/useConfirm'
import { useRegistryStore } from '@/stores/registry'
import { formatBinaryBytes } from '@/utils/formatters'

const registry = useRegistryStore()
const { confirm } = useConfirm()
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const form = reactive({ keepVersions: 10, quotaGib: 50 })
const saving = ref(false)
const collecting = ref(false)
const message = ref<string | null>(null)
const messageType = ref<'success' | 'error'>('success')

/** O formulário difere do que está salvo (a limpeza usa o salvo). */
const dirty = computed(() => {
  const saved = registry.overview?.settings
  return !!saved && (saved.keepVersions !== form.keepVersions || saved.quotaGib !== form.quotaGib)
})

watch(
  () => registry.overview?.settings,
  (settings) => {
    if (settings) Object.assign(form, settings)
  },
  { immediate: true }
)

async function save() {
  const result = await formRef.value?.validate()
  if (result && !result.valid) return
  saving.value = true
  try {
    await registry.saveSettings({ keepVersions: form.keepVersions, quotaGib: form.quotaGib })
    messageType.value = 'success'
    message.value = 'Configuração salva.'
  } catch (err) {
    messageType.value = 'error'
    message.value = err instanceof Error ? err.message : 'Não foi possível salvar'
  } finally {
    saving.value = false
  }
}

async function collect() {
  const ok = await confirm({
    title: 'Limpar o registry agora?',
    message: `Saem as versões além das ${form.keepVersions} mais recentes de cada repositório e os blobs que nada usa. O que está no ar, guardado para rollback ou fixado fica.`,
    confirmText: 'Limpar',
    confirmColor: 'warning',
    icon: 'mdi-broom',
  })
  if (!ok) return
  collecting.value = true
  try {
    const { freedBytes } = await registry.collect()
    messageType.value = 'success'
    message.value =
      freedBytes > 0 ? `Liberados ${formatBinaryBytes(freedBytes)}.` : 'Nada a liberar agora.'
    await registry.fetchOverview()
  } catch (err) {
    messageType.value = 'error'
    message.value = err instanceof Error ? err.message : 'A limpeza falhou'
  } finally {
    collecting.value = false
  }
}
</script>

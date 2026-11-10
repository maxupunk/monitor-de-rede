<template>
  <v-dialog
    :model-value="modelValue"
    max-width="620"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="application">
      <v-card-item class="px-6 pt-5">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-key-chain</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Segredos de build</v-card-title>
        <v-card-subtitle class="text-high-emphasis">{{ application.name }}</v-card-subtitle>
      </v-card-item>
      <v-card-text class="px-6">
        <v-alert type="info" variant="tonal" density="compact" class="mb-4">
          Ficam cifrados na central e chegam ao builder só como <code>--secret</code>: não entram na
          imagem nem no log. No Dockerfile:
          <code>RUN --mount=type=secret,id=NOME …</code>
        </v-alert>
        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">
          {{ error }}
        </v-alert>
        <v-list v-if="items.length" density="compact" class="bg-transparent pa-0 mb-3">
          <v-list-item
            v-for="item in items"
            :key="item.name"
            :title="item.name"
            :subtitle="`Atualizado ${formatRelativeTime(item.updatedAt)}`"
            class="px-0"
          >
            <template #prepend>
              <v-icon color="primary" class="mr-3">mdi-key</v-icon>
            </template>
            <template #append>
              <IconAction
                icon="mdi-pencil"
                label="Trocar o valor"
                color="primary"
                @click="form.name = item.name"
              />
              <IconAction
                icon="mdi-delete"
                label="Apagar"
                color="error"
                @click="remove(item.name)"
              />
            </template>
          </v-list-item>
        </v-list>
        <div v-else-if="!loading" class="text-body-2 text-high-emphasis mb-3">Nenhum segredo.</div>

        <v-form ref="formRef" autocomplete="off" @submit.prevent="save">
          <v-row dense>
            <v-col cols="12" sm="5">
              <v-text-field
                v-model="form.name"
                label="Nome"
                placeholder="NPM_TOKEN"
                variant="outlined"
                :rules="[requiredRule('Informe o nome'), envVariableRule()]"
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="7">
              <v-text-field
                v-model="form.value"
                label="Valor"
                :type="show ? 'text' : 'password'"
                :append-inner-icon="show ? 'mdi-eye-off' : 'mdi-eye'"
                variant="outlined"
                :rules="[requiredRule('Informe o valor')]"
                @click:append-inner="show = !show"
              ></v-text-field>
            </v-col>
          </v-row>
          <v-btn
            color="primary"
            variant="flat"
            prepend-icon="mdi-content-save"
            :loading="saving"
            @click="save"
          >
            Salvar segredo
          </v-btn>
        </v-form>
      </v-card-text>
      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" @click="emit('update:modelValue', false)"
          >Fechar</v-btn
        >
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import IconAction from '@/components/IconAction.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployBuildSecretView } from '@/bindings/DeployBuildSecretView'
import { useDeployBuildsStore } from '@/stores/deployBuilds'
import { envVariableRule, requiredRule } from '@/utils/formRules'
import { formatRelativeTime } from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean; application: DeployApplicationView | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const builds = useDeployBuildsStore()
const { confirm } = useConfirm()
const items = ref<DeployBuildSecretView[]>([])
const loading = ref(false)
const saving = ref(false)
const show = ref(false)
const error = ref<string | null>(null)
const formRef = ref<{
  validate: () => Promise<{ valid: boolean }>
  resetValidation: () => void
} | null>(null)
const form = reactive({ name: '', value: '' })

async function load() {
  if (!props.application) return
  loading.value = true
  error.value = null
  try {
    items.value = await builds.secrets(props.application.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível ler os segredos'
  } finally {
    loading.value = false
  }
}

async function save() {
  if (!props.application) return
  const result = await formRef.value?.validate()
  if (result && !result.valid) return
  saving.value = true
  error.value = null
  try {
    items.value = await builds.saveSecret(props.application.id, form.name.trim(), form.value)
    form.name = ''
    form.value = ''
    formRef.value?.resetValidation()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível salvar'
  } finally {
    saving.value = false
  }
}

async function remove(name: string) {
  if (!props.application) return
  const ok = await confirm({
    title: `Apagar ${name}?`,
    message: 'A próxima compilação não terá mais este segredo.',
    confirmText: 'Apagar',
    confirmColor: 'error',
    icon: 'mdi-delete',
  })
  if (!ok) return
  try {
    await builds.removeSecret(props.application.id, name)
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível apagar'
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      form.name = ''
      form.value = ''
      void load()
    }
  }
)
</script>

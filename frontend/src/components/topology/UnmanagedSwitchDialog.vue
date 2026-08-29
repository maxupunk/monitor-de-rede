<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 560"
    :fullscreen="$vuetify.display.xs"
    persistent
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card class="rounded-xl overflow-hidden elevation-12">
      <!-- Header do Modal -->
      <v-card-item class="bg-indigo-darken-2 text-white py-4 px-6">
        <div class="d-flex align-center justify-space-between w-100">
          <div class="d-flex align-center">
            <v-avatar color="white" variant="flat" size="38" class="mr-3 text-indigo-darken-2">
              <v-icon size="24">mdi-hub</v-icon>
            </v-avatar>
            <div>
              <v-card-title class="text-h6 font-weight-bold pa-0 text-white">
                Adicionar Switch
              </v-card-title>
              <div class="text-caption text-white opacity-80">
                Cadastre switches e multiplicadores de portas na topologia
              </div>
            </div>
          </div>
          <v-btn
            icon="mdi-close"
            variant="text"
            density="comfortable"
            color="white"
            @click="close"
          ></v-btn>
        </div>
      </v-card-item>

      <v-card-text class="pa-6">
        <!-- Explicação do papel do Switch -->
        <v-alert
          type="info"
          variant="tonal"
          density="comfortable"
          class="mb-4 rounded-lg text-caption"
          icon="mdi-information-outline"
        >
          Switches não gerenciáveis não requerem IP ou SNMP. Eles atuam como nós de derivação de
          portas para que você possa mapear com precisão de onde vêm e para onde vão os links
          ramificados.
        </v-alert>

        <v-form ref="formRef" @submit.prevent="save">
          <v-row dense>
            <v-col cols="12">
              <v-text-field
                v-model="form.name"
                label="Nome de Identificação do Switch *"
                placeholder="Ex: Switch - Balcão / Recepção"
                variant="outlined"
                density="comfortable"
                :rules="[rules.required]"
                prepend-inner-icon="mdi-label-outline"
                autofocus
              ></v-text-field>
            </v-col>

            <v-col cols="12">
              <div class="text-caption font-weight-bold mb-2 text-medium-emphasis">
                Quantidade de Portas Físicas *
              </div>
              <v-btn-toggle
                v-model="form.portCount"
                mandatory
                color="indigo"
                variant="outlined"
                density="comfortable"
                class="w-100 mb-3 d-flex flex-wrap"
              >
                <v-btn :value="5" class="flex-grow-1">5 Portas</v-btn>
                <v-btn :value="8" class="flex-grow-1">8 Portas</v-btn>
                <v-btn :value="16" class="flex-grow-1">16 Portas</v-btn>
                <v-btn :value="24" class="flex-grow-1">24 Portas</v-btn>
                <v-btn :value="48" class="flex-grow-1">48 Portas</v-btn>
              </v-btn-toggle>
            </v-col>

            <!-- Prévia visual das portas físicas geradas -->
            <v-col cols="12" class="mb-3">
              <div class="ports-preview-container pa-3 rounded-lg">
                <div class="d-flex align-center justify-space-between mb-2">
                  <span class="text-caption font-weight-bold text-medium-emphasis">
                    Portas Virtuais Criadas Automaticamente:
                  </span>
                  <v-chip size="x-small" color="indigo" variant="flat">
                    {{ form.portCount }} portas disponíveis
                  </v-chip>
                </div>
                <div class="d-flex flex-wrap gap-1">
                  <div
                    v-for="p in Math.min(form.portCount, 24)"
                    :key="p"
                    class="port-badge text-caption pa-1 rounded text-center"
                  >
                    <v-icon size="12" color="indigo">mdi-ethernet</v-icon>
                    <span>P{{ p }}</span>
                  </div>
                  <div
                    v-if="form.portCount > 24"
                    class="port-badge text-caption pa-1 rounded text-center text-medium-emphasis"
                  >
                    +{{ form.portCount - 24 }} portas...
                  </div>
                </div>
              </div>
            </v-col>

            <v-col cols="12" sm="6">
              <v-text-field
                v-model="form.vendor"
                label="Fabricante (Opcional)"
                placeholder="Ex: TP-Link, Intelbras, D-Link"
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-domain"
              ></v-text-field>
            </v-col>

            <v-col cols="12" sm="6">
              <v-text-field
                v-model="form.model"
                label="Modelo (Opcional)"
                placeholder="Ex: TL-SG108, SG 2404"
                variant="outlined"
                density="comfortable"
                prepend-inner-icon="mdi-tag-outline"
              ></v-text-field>
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 px-6 justify-end bg-surface">
        <v-btn variant="text" :disabled="saving" @click="close">Cancelar</v-btn>
        <v-btn
          color="indigo-darken-2"
          variant="elevated"
          prepend-icon="mdi-plus-box"
          :loading="saving"
          @click="save"
        >
          Criar Switch
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import { useTopologyStore } from '@/stores/topology'

const props = defineProps<{
  modelValue: boolean
  defaultSiteId?: number | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'created'): void
}>()

const topologyStore = useTopologyStore()
const formRef = ref()
const saving = ref(false)

const form = reactive({
  name: '',
  vendor: '',
  model: '',
  portCount: 8,
  siteId: null as number | null,
})

const rules = {
  required: (v: string) => !!v?.trim() || 'Campo obrigatório',
}

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) {
      form.name = ''
      form.vendor = ''
      form.model = ''
      form.portCount = 8
      form.siteId = props.defaultSiteId ?? null
    }
  }
)

function close() {
  emit('update:modelValue', false)
}

async function save() {
  const isValid = await formRef.value?.validate()
  if (!isValid?.valid) return

  saving.value = true
  try {
    const success = await topologyStore.createUnmanagedSwitch({
      name: form.name.trim(),
      vendor: form.vendor?.trim() || undefined,
      model: form.model?.trim() || undefined,
      portCount: form.portCount,
      siteId: form.siteId,
    })
    if (success) {
      emit('created')
      close()
    }
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.ports-preview-container {
  background: rgba(var(--v-theme-surface-variant), 0.35);
  border: 1px solid rgba(var(--v-theme-outline), 0.2);
}
.port-badge {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-theme-indigo), 0.3);
  font-size: 11px;
  min-width: 38px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
}
.gap-1 {
  gap: 4px;
}
</style>

<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 560"
    :fullscreen="$vuetify.display.xs"
    scrollable
    persistent
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card class="rounded-xl overflow-hidden elevation-12 dialog-card-container">
      <!-- Header do Modal -->
      <v-card-item class="bg-indigo-darken-2 text-white py-4 px-6 flex-shrink-0">
        <div class="d-flex align-center justify-space-between w-100">
          <div class="d-flex align-center">
            <v-avatar color="white" variant="flat" size="38" class="mr-3 text-indigo-darken-2">
              <v-icon size="24">mdi-hub</v-icon>
            </v-avatar>
            <div>
              <v-card-title class="text-h6 font-weight-bold pa-0 text-white">
                {{ isEditMode ? 'Editar Switch' : 'Adicionar Switch' }}
              </v-card-title>
              <div class="text-caption text-white opacity-80">
                {{
                  isEditMode
                    ? 'Altere as informações do switch não gerenciável'
                    : 'Cadastre switches e multiplicadores de portas na topologia'
                }}
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

      <v-card-text class="pa-4 pa-sm-6 flex-grow-1 overflow-y-auto">
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

            <v-col v-if="!isEditMode" cols="12">
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
            <v-col v-if="!isEditMode" cols="12" class="mb-3">
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

            <v-col cols="12">
              <v-select
                v-model="form.siteId"
                :items="sitesStore.sites"
                item-title="name"
                item-value="id"
                label="Site / Localidade (Opcional)"
                placeholder="Selecione o local do switch"
                variant="outlined"
                density="comfortable"
                clearable
                prepend-inner-icon="mdi-map-marker-outline"
                hint="Associa este switch ao local físico, facilitando filtros e identificação do pai."
                persistent-hint
              ></v-select>
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 px-6 justify-end bg-surface border-t flex-shrink-0">
        <v-btn variant="text" :disabled="saving" @click="close">Cancelar</v-btn>
        <v-btn
          color="indigo-darken-2"
          variant="elevated"
          :prepend-icon="isEditMode ? 'mdi-check' : 'mdi-plus-box'"
          :loading="saving"
          @click="save"
        >
          {{ isEditMode ? 'Salvar Alterações' : 'Criar Switch' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch } from 'vue'
import { useTopologyStore, type TopologyNode } from '@/stores/topology'
import { useSitesStore } from '@/stores/sites'
import { useDevicesStore } from '@/stores/devices'

const props = defineProps<{
  modelValue: boolean
  defaultSiteId?: number | null
  switchToEdit?: TopologyNode | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'created'): void
}>()

const topologyStore = useTopologyStore()
const sitesStore = useSitesStore()
const devicesStore = useDevicesStore()
const formRef = ref()
const saving = ref(false)

const isEditMode = computed(() => Boolean(props.switchToEdit?.id))

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
      if (sitesStore.sites.length === 0) void sitesStore.fetchSites()
      if (props.switchToEdit) {
        form.name = props.switchToEdit.name || ''
        form.vendor = props.switchToEdit.vendor || ''
        form.model = props.switchToEdit.model || ''
        form.portCount = props.switchToEdit.interfaceCount || 8
        form.siteId = props.switchToEdit.siteId ?? props.defaultSiteId ?? null
      } else {
        form.name = ''
        form.vendor = ''
        form.model = ''
        form.portCount = 8
        form.siteId = props.defaultSiteId ?? null
      }
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
    if (isEditMode.value && props.switchToEdit?.id) {
      const updated = await devicesStore.updateDevice(props.switchToEdit.id, {
        name: form.name.trim(),
        vendor: form.vendor?.trim() || undefined,
        model: form.model?.trim() || undefined,
        siteId: form.siteId,
      })
      if (updated) {
        await topologyStore.fetchTopology(null, false)
        emit('created')
        close()
      }
    } else {
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
    }
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.dialog-card-container {
  display: flex;
  flex-direction: column;
  max-height: 90vh;
}
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
@media (max-width: 600px) {
  .dialog-card-container {
    max-height: 100vh;
    height: 100%;
    border-radius: 0 !important;
  }
  .port-badge {
    min-width: 32px;
    font-size: 10px;
  }
}
</style>

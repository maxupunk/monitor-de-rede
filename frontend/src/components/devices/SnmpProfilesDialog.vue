<script setup lang="ts">
import { ref, watch } from 'vue'
import { useSnmpProfilesStore, type SnmpDeviceProfile } from '@/stores/snmpProfiles'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const profilesStore = useSnmpProfilesStore()
const activeTab = ref<'list' | 'create'>('list')
const jsonEditorContent = ref('')
const jsonError = ref<string | null>(null)
const successMessage = ref<string | null>(null)

const sampleTemplate: SnmpDeviceProfile = {
  id: 'meu_controlador_solar',
  name: 'Controlador Solar Personalizado',
  vendor: 'Fabricante Exemplo',
  category: 'solar_mppt',
  sysObjectIdPrefix: '1.3.6.1.4.1.99999.1',
  sysDescrPattern: 'Solar Controller',
  sensors: [
    {
      key: 'pv_voltage',
      label: 'Tensão dos Painéis (PV)',
      oid: '1.3.6.1.4.1.99999.1.1.0',
      unit: 'V',
      scale: 0.1,
      category: 'solar',
      dataType: 'float',
      description: 'Tensão gerada pelas placas solares',
      icon: 'mdi-solar-power',
      color: 'amber',
    },
    {
      key: 'charge_status',
      label: 'Status de Carga',
      oid: '1.3.6.1.4.1.99999.1.2.0',
      unit: '',
      scale: 1.0,
      category: 'status',
      dataType: 'state',
      description: 'Estado operacional de carga do controlador',
      icon: 'mdi-battery-charging',
      color: 'purple',
      states: {
        '0': 'Inativo',
        '1': 'Carga',
        '2': 'Flutuação',
        '3': 'Equalização',
      },
    },
    {
      key: 'load_status',
      label: 'Status da Saída',
      oid: '1.3.6.1.4.1.99999.1.3.0',
      unit: '',
      scale: 1.0,
      category: 'load',
      dataType: 'boolean',
      description: 'Estado da saída controlada de carga',
      icon: 'mdi-power',
      color: 'indigo',
      states: {
        '0': 'Desligada',
        '1': 'Ligada',
      },
    },
  ],
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      profilesStore.fetchProfiles()
      activeTab.value = 'list'
      successMessage.value = null
      jsonError.value = null
    }
  }
)

function closeDialog() {
  emit('update:modelValue', false)
}

function loadSampleTemplate() {
  jsonEditorContent.value = JSON.stringify(sampleTemplate, null, 2)
  jsonError.value = null
}

function editProfile(profile: SnmpDeviceProfile) {
  jsonEditorContent.value = JSON.stringify(profile, null, 2)
  activeTab.value = 'create'
}

async function saveJsonProfile() {
  jsonError.value = null
  successMessage.value = null
  try {
    const parsed = JSON.parse(jsonEditorContent.value)
    if (!parsed.id || typeof parsed.id !== 'string') {
      jsonError.value = 'O campo "id" é obrigatório (ex: "meu_mppt").'
      return
    }
    if (!parsed.name || typeof parsed.name !== 'string') {
      jsonError.value = 'O campo "name" é obrigatório.'
      return
    }
    if (!Array.isArray(parsed.sensors) || parsed.sensors.length === 0) {
      jsonError.value = 'A lista de "sensors" deve conter pelo menos um sensor com OID.'
      return
    }

    const ok = await profilesStore.saveProfile(parsed)
    if (ok) {
      successMessage.value = `Perfil "${parsed.name}" salvo com sucesso!`
      activeTab.value = 'list'
    } else {
      jsonError.value = profilesStore.error || 'Erro ao gravar o perfil no servidor.'
    }
  } catch (err: unknown) {
    jsonError.value =
      err instanceof Error ? `JSON Inválido: ${err.message}` : 'Erro de sintaxe no JSON'
  }
}

async function deleteProfile(profile: SnmpDeviceProfile) {
  if (confirm(`Tem certeza que deseja excluir o perfil "${profile.name}"?`)) {
    const ok = await profilesStore.deleteProfile(profile.id)
    if (ok) {
      successMessage.value = `Perfil "${profile.name}" removido.`
    }
  }
}
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    max-width="900"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-surface">
        <div class="d-flex align-center ga-2" style="gap: 8px">
          <v-icon color="primary">mdi-solar-power-variant-outline</v-icon>
          <span class="text-h6 font-weight-bold">Perfis SNMP de Dispositivos e Sensores</span>
        </div>
        <v-btn icon="mdi-close" variant="text" size="small" @click="closeDialog"></v-btn>
      </v-card-title>

      <v-tabs v-model="activeTab" bg-color="surface" color="primary">
        <v-tab value="list" prepend-icon="mdi-format-list-bulleted"> Perfis Disponíveis </v-tab>
        <v-tab value="create" prepend-icon="mdi-code-json">
          Cadastrar / Importar Perfil (JSON)
        </v-tab>
      </v-tabs>

      <v-divider></v-divider>

      <v-card-text class="pa-4" style="max-height: 70vh">
        <v-alert
          v-if="successMessage"
          type="success"
          variant="tonal"
          closable
          density="compact"
          class="mb-4"
          @click:close="successMessage = null"
        >
          {{ successMessage }}
        </v-alert>

        <v-alert
          v-if="profilesStore.error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-4"
        >
          {{ profilesStore.error }}
        </v-alert>

        <v-window v-model="activeTab">
          <!-- LISTA DE PERFIS -->
          <v-window-item value="list">
            <div v-if="profilesStore.loading" class="text-center py-8">
              <v-progress-circular indeterminate color="primary"></v-progress-circular>
              <div class="text-caption text-grey mt-2">Carregando catálogo de perfis...</div>
            </div>

            <div v-else-if="profilesStore.profiles.length === 0" class="text-center py-8 text-grey">
              Nenhum perfil cadastrado.
            </div>

            <div v-else class="d-flex flex-column ga-3" style="gap: 12px">
              <v-card
                v-for="profile in profilesStore.profiles"
                :key="profile.id"
                variant="outlined"
                class="pa-4 rounded-lg"
              >
                <div class="d-flex align-center justify-space-between mb-2">
                  <div class="d-flex align-center ga-2" style="gap: 8px">
                    <span class="text-subtitle-1 font-weight-bold">{{ profile.name }}</span>
                    <v-chip
                      size="x-small"
                      :color="profile.isBuiltin ? 'primary' : 'secondary'"
                      variant="tonal"
                    >
                      {{ profile.isBuiltin ? 'Nativo do Sistema' : 'Customizado' }}
                    </v-chip>
                    <v-chip size="x-small" variant="outlined" color="grey">
                      {{ profile.vendor }}
                    </v-chip>
                  </div>
                  <div class="d-flex align-center ga-1" style="gap: 4px">
                    <v-btn
                      size="small"
                      variant="text"
                      color="primary"
                      prepend-icon="mdi-code-json"
                      @click="editProfile(profile)"
                    >
                      Ver / Editar JSON
                    </v-btn>
                    <v-btn
                      v-if="!profile.isBuiltin"
                      size="small"
                      variant="text"
                      color="error"
                      icon="mdi-delete-outline"
                      @click="deleteProfile(profile)"
                    ></v-btn>
                  </div>
                </div>

                <div class="text-caption text-grey mb-3">
                  <span v-if="profile.sysObjectIdPrefix">
                    <strong>sysObjectID Prefixo:</strong>
                    <code>{{ profile.sysObjectIdPrefix }}</code>
                  </span>
                  <span v-if="profile.sysDescrPattern" class="ml-3">
                    <strong>sysDescr Padrão:</strong> <em>"{{ profile.sysDescrPattern }}"</em>
                  </span>
                </div>

                <!-- Tabela de Sensores do Perfil -->
                <div class="text-caption font-weight-bold mb-1">
                  Sensores & Métricas Mapeadas ({{ profile.sensors.length }}):
                </div>
                <v-table density="compact" class="rounded-lg border">
                  <thead>
                    <tr>
                      <th>Identificador</th>
                      <th>Métrica / Rótulo</th>
                      <th>OID</th>
                      <th>Escala</th>
                      <th>Unidade</th>
                      <th>Tipo / Estados</th>
                      <th>Categoria</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="sensor in profile.sensors" :key="sensor.key">
                      <td>
                        <code>{{ sensor.key }}</code>
                      </td>
                      <td class="font-weight-medium">{{ sensor.label }}</td>
                      <td>
                        <code class="text-caption">{{ sensor.oid }}</code>
                      </td>
                      <td>{{ sensor.scale ?? 1.0 }}</td>
                      <td>{{ sensor.unit || '-' }}</td>
                      <td>
                        <div class="d-flex align-center flex-wrap ga-1" style="gap: 4px">
                          <v-chip
                            size="x-small"
                            variant="tonal"
                            :color="
                              sensor.dataType === 'state'
                                ? 'purple'
                                : sensor.dataType === 'boolean'
                                  ? 'indigo'
                                  : 'info'
                            "
                          >
                            {{ sensor.dataType || 'float' }}
                          </v-chip>
                          <template v-if="sensor.states">
                            <v-chip
                              v-for="(st, k) in sensor.states"
                              :key="k"
                              size="x-small"
                              variant="outlined"
                              class="text-xxs"
                            >
                              {{ k }}: {{ typeof st === 'string' ? st : st.label }}
                            </v-chip>
                          </template>
                        </div>
                      </td>
                      <td>
                        <v-chip size="x-small" variant="tonal" color="primary">
                          {{ sensor.category || 'geral' }}
                        </v-chip>
                      </td>
                    </tr>
                  </tbody>
                </v-table>
              </v-card>
            </div>
          </v-window-item>

          <!-- CRIAR / IMPORTAR PERFIL -->
          <v-window-item value="create">
            <div class="d-flex align-center justify-space-between mb-3">
              <div class="text-body-2 text-grey">
                Cole a definição do perfil SNMP em formato JSON ou use o modelo pré-formatado:
              </div>
              <v-btn
                size="small"
                variant="tonal"
                color="secondary"
                prepend-icon="mdi-file-document-outline"
                @click="loadSampleTemplate"
              >
                Carregar Modelo de Exemplo
              </v-btn>
            </div>

            <v-alert
              v-if="jsonError"
              type="error"
              variant="tonal"
              density="compact"
              class="mb-3"
              closable
              @click:close="jsonError = null"
            >
              {{ jsonError }}
            </v-alert>

            <v-textarea
              v-model="jsonEditorContent"
              variant="outlined"
              rows="16"
              class="font-monospace"
              placeholder="Cole o JSON do perfil aqui..."
              hide-details
            ></v-textarea>

            <div class="d-flex justify-end ga-2 mt-4" style="gap: 8px">
              <v-btn variant="text" @click="activeTab = 'list'"> Cancelar </v-btn>
              <v-btn
                color="primary"
                prepend-icon="mdi-content-save"
                :loading="profilesStore.saving"
                :disabled="!jsonEditorContent.trim()"
                @click="saveJsonProfile"
              >
                Salvar Perfil SNMP
              </v-btn>
            </div>
          </v-window-item>
        </v-window>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 justify-end">
        <v-btn variant="text" @click="closeDialog">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<style scoped>
.font-monospace :deep(textarea) {
  font-family:
    SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace !important;
  font-size: 0.85rem;
}

.text-xxs {
  font-size: 0.7rem;
}
</style>

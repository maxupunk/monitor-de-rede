<template>
  <div>
    <PageHeader
      title="Templates Zabbix"
      subtitle="Importe templates oficiais do Zabbix (export JSON) para que novos dispositivos herdem os itens SNMP monitorados."
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-upload" @click="openImportDialog">
          <span class="hidden-sm-and-down">Importar Template</span>
          <span class="hidden-md-and-up">Importar</span>
        </v-btn>
      </template>
    </PageHeader>

    <v-card elevation="2" class="mobile-full-bleed">
      <ResponsiveDataTable
        :headers="headers"
        :items="templatesStore.templates"
        :loading="templatesStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum template Zabbix importado ainda"
        :clickable="false"
      >
        <template #item.name="{ item }">
          <div class="py-2">
            <div class="text-subtitle-1 font-weight-bold">{{ item.name }}</div>
            <div v-if="item.description" class="text-caption text-grey">{{ item.description }}</div>
          </div>
        </template>

        <template #item.zabbixVersion="{ item }">
          <v-chip size="small" color="info" variant="tonal">
            Zabbix {{ item.zabbixVersion || '?' }}
          </v-chip>
        </template>

        <template #item.items="{ item }">
          <v-btn size="small" variant="text" color="primary" @click="openItemsDialog(item)">
            {{ item.items.length }} {{ item.items.length === 1 ? 'item' : 'itens' }}
          </v-btn>
        </template>

        <template #item.deviceCount="{ item }">
          <v-chip size="small" :color="item.deviceCount > 0 ? 'success' : 'grey'" variant="tonal">
            {{ item.deviceCount }} {{ item.deviceCount === 1 ? 'dispositivo' : 'dispositivos' }}
          </v-chip>
        </template>

        <template #item.actions="{ item }">
          <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-2">
            <div class="flex-grow-1 text-break">
              <div class="text-subtitle-2 font-weight-bold">{{ item.name }}</div>
              <div v-if="item.description" class="text-caption text-grey">
                {{ item.description }}
              </div>
              <div class="d-flex flex-wrap align-center ga-2 mt-2">
                <v-chip size="x-small" color="info" variant="tonal">
                  Zabbix {{ item.zabbixVersion || '?' }}
                </v-chip>
                <v-btn size="x-small" variant="text" color="primary" @click="openItemsDialog(item)">
                  {{ item.items.length }} {{ item.items.length === 1 ? 'item' : 'itens' }}
                </v-btn>
                <v-chip
                  size="x-small"
                  :color="item.deviceCount > 0 ? 'success' : 'grey'"
                  variant="tonal"
                >
                  {{ item.deviceCount }}
                  {{ item.deviceCount === 1 ? 'dispositivo' : 'dispositivos' }}
                </v-chip>
              </div>
            </div>
            <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item)">
              <v-icon>mdi-delete</v-icon>
            </v-btn>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <!-- Dialog de Importação -->
    <v-dialog
      v-model="importDialog"
      :max-width="$vuetify.display.xs ? undefined : 560"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">Importar Template Zabbix</v-card-title>
        <v-card-text>
          <p class="text-body-2 text-grey-darken-1 mb-4">
            Selecione o arquivo JSON exportado do Zabbix (Data collection → Templates → Export).
            Apenas itens SNMP_AGENT (com OID) são importados — outros tipos de item são listados
            como ignorados após a importação.
          </p>
          <v-file-input
            v-model="selectedFile"
            label="Arquivo .json do template"
            accept=".json,application/json"
            variant="outlined"
            prepend-icon="mdi-file-code-outline"
            show-size
            :error-messages="fileError"
            @update:model-value="fileError = ''"
          ></v-file-input>

          <v-alert
            v-if="templatesStore.error"
            type="error"
            variant="tonal"
            density="compact"
            class="mt-2"
          >
            {{ templatesStore.error }}
          </v-alert>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="importDialog = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            :loading="templatesStore.importing"
            :disabled="!selectedFile"
            @click="doImport"
          >
            Importar
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Dialog de Resultado da Importação -->
    <v-dialog
      v-model="resultDialog"
      :max-width="$vuetify.display.xs ? undefined : 560"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold d-flex align-center ga-2">
          <v-icon color="success">mdi-check-circle</v-icon>
          Importação concluída
        </v-card-title>
        <v-card-text>
          <div v-for="result in importResults" :key="result.id" class="mb-4">
            <div class="text-subtitle-1 font-weight-bold">{{ result.name }}</div>
            <div class="text-body-2 text-grey-darken-1">
              {{ result.itemCount }}
              {{ result.itemCount === 1 ? 'item importado' : 'itens importados' }}
            </div>
            <v-alert
              v-if="result.skippedItems.length > 0"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-2"
            >
              {{ result.skippedItems.length }}
              {{ result.skippedItems.length === 1 ? 'item ignorado' : 'itens ignorados' }} (tipo não
              suportado, apenas SNMP_AGENT é importado):
              <div class="text-caption mt-1">
                {{ result.skippedItems.map((s) => `${s.name} (${s.type})`).join(', ') }}
              </div>
            </v-alert>
          </div>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn color="primary" variant="flat" @click="resultDialog = false">Fechar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Dialog de Itens do Template -->
    <v-dialog
      v-model="itemsDialog"
      :max-width="$vuetify.display.xs ? undefined : 700"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">{{ selectedTemplate?.name }}</v-card-title>
        <v-card-text>
          <v-table density="compact">
            <thead>
              <tr>
                <th>Nome</th>
                <th>Key</th>
                <th>OID</th>
                <th>Unidade</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in selectedTemplate?.items" :key="item.id">
                <td>{{ item.name }}</td>
                <td class="text-caption">{{ item.key }}</td>
                <td class="text-caption">{{ item.snmpOid }}</td>
                <td>{{ item.units || '-' }}</td>
              </tr>
            </tbody>
          </v-table>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="itemsDialog = false">Fechar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import {
  useZabbixTemplatesStore,
  type ZabbixTemplate,
  type ZabbixImportResult,
} from '@/stores/zabbixTemplates'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'

const templatesStore = useZabbixTemplatesStore()

const importDialog = ref(false)
const resultDialog = ref(false)
const itemsDialog = ref(false)
const selectedFile = ref<File[] | File | null>(null)
const fileError = ref('')
const importResults = ref<ZabbixImportResult[]>([])
const selectedTemplate = ref<ZabbixTemplate | null>(null)

const headers = [
  { title: 'Template', key: 'name' },
  { title: 'Versão', key: 'zabbixVersion', width: '140px' },
  { title: 'Itens SNMP', key: 'items', width: '140px' },
  { title: 'Uso', key: 'deviceCount', width: '160px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '80px' },
]

onMounted(async () => {
  await templatesStore.fetchTemplates()
})

function openImportDialog() {
  selectedFile.value = null
  fileError.value = ''
  importDialog.value = true
}

function openItemsDialog(template: ZabbixTemplate) {
  selectedTemplate.value = template
  itemsDialog.value = true
}

async function doImport() {
  const file = Array.isArray(selectedFile.value) ? selectedFile.value[0] : selectedFile.value
  if (!file) return

  const content = await file.text()
  const results = await templatesStore.importTemplate(content)
  if (results) {
    importResults.value = results
    importDialog.value = false
    resultDialog.value = true
  }
}

async function confirmDelete(template: ZabbixTemplate) {
  if (
    confirm(
      `Excluir o template "${template.name}"? Dispositivos vinculados a ele deixarão de coletar essas métricas.`
    )
  ) {
    await templatesStore.deleteTemplate(template.id)
  }
}
</script>

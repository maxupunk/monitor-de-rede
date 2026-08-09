<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 760"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-item class="pa-5 pb-3">
        <template #prepend>
          <v-avatar color="deep-purple" size="44" rounded="lg" variant="tonal">
            <v-icon size="24">mdi-dns-outline</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-h6">Servidores DNS</v-card-title>
        <v-card-subtitle>
          Usados no autocomplete dos monitores e na comparação de tempo de consulta do dashboard.
        </v-card-subtitle>
      </v-card-item>

      <v-divider></v-divider>

      <v-card-text class="pa-5">
        <v-alert
          v-if="store.error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-4"
          :text="store.error"
        ></v-alert>

        <!-- Formulário de cadastro / edição -->
        <v-form ref="formRef" validate-on="submit lazy" @submit.prevent="submit">
          <v-sheet border rounded class="pa-4 mb-5">
            <div class="text-overline text-medium-emphasis mb-3">
              {{ editingId ? 'Editando servidor' : 'Adicionar servidor' }}
            </div>
            <v-row>
              <v-col cols="12" md="4">
                <v-text-field
                  v-model="form.name"
                  label="Nome *"
                  placeholder="Ex: DNS da Matriz"
                  variant="outlined"
                  density="comfortable"
                  :rules="[(v: string) => !!(v || '').trim() || 'Informe um nome']"
                  hide-details="auto"
                ></v-text-field>
              </v-col>
              <v-col cols="12" md="3">
                <v-select
                  v-model="form.protocol"
                  :items="PROTOCOL_OPTIONS"
                  item-title="label"
                  item-value="value"
                  label="Transporte"
                  variant="outlined"
                  density="comfortable"
                  hide-details="auto"
                ></v-select>
              </v-col>
              <v-col cols="12" md="5">
                <v-text-field
                  v-model="form.address"
                  :label="form.protocol === 'doh' ? 'Endpoint DoH *' : 'Endereço IP *'"
                  :placeholder="
                    form.protocol === 'doh' ? 'https://cloudflare-dns.com/dns-query' : '1.1.1.1'
                  "
                  variant="outlined"
                  density="comfortable"
                  :rules="[addressRule]"
                  hide-details="auto"
                ></v-text-field>
              </v-col>
              <v-col cols="12" md="8">
                <v-text-field
                  v-model="form.description"
                  label="Descrição (opcional)"
                  variant="outlined"
                  density="comfortable"
                  hide-details="auto"
                ></v-text-field>
              </v-col>
              <v-col cols="12" md="4" class="d-flex align-center">
                <v-switch
                  v-model="form.isDefault"
                  color="deep-purple"
                  density="comfortable"
                  label="Comparar no dashboard"
                  hide-details
                ></v-switch>
              </v-col>
            </v-row>

            <div class="d-flex justify-end ga-2 mt-3">
              <v-btn v-if="editingId" variant="text" @click="resetForm">Cancelar edição</v-btn>
              <v-btn
                color="deep-purple"
                variant="flat"
                :loading="store.saving"
                :disabled="!canSubmit"
                type="submit"
              >
                {{ editingId ? 'Salvar alterações' : 'Adicionar' }}
              </v-btn>
            </div>
          </v-sheet>
        </v-form>

        <!-- Lista cadastrada -->
        <div class="text-overline text-medium-emphasis mb-2">
          Cadastrados ({{ store.servers.length }})
        </div>

        <v-list v-if="store.servers.length > 0" class="border rounded py-0">
          <v-list-item
            v-for="server in store.servers"
            :key="server.id"
            class="px-4 py-3 border-b"
            :class="{ 'bg-deep-purple-lighten-5': editingId === server.id }"
          >
            <template #prepend>
              <v-avatar
                :color="server.isDefault ? 'deep-purple' : 'grey'"
                size="34"
                variant="tonal"
              >
                <v-icon size="18">{{ protocolIcon(server.protocol) }}</v-icon>
              </v-avatar>
            </template>

            <v-list-item-title class="d-flex align-center ga-2 flex-wrap">
              <span class="font-weight-medium">{{ server.name }}</span>
              <v-chip size="x-small" variant="tonal" color="grey">
                {{ server.protocol.toUpperCase() }}
              </v-chip>
              <v-chip v-if="server.isDefault" size="x-small" variant="tonal" color="deep-purple">
                no dashboard
              </v-chip>
            </v-list-item-title>
            <v-list-item-subtitle>
              {{ server.address }}
              <span v-if="server.description"> · {{ server.description }}</span>
            </v-list-item-subtitle>

            <template #append>
              <v-btn icon size="small" variant="text" color="primary" @click="startEdit(server)">
                <v-icon size="20">mdi-pencil</v-icon>
                <v-tooltip activator="parent" location="top">Editar</v-tooltip>
              </v-btn>
              <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(server)">
                <v-icon size="20">mdi-delete</v-icon>
                <v-tooltip activator="parent" location="top">Excluir</v-tooltip>
              </v-btn>
            </template>
          </v-list-item>
        </v-list>
        <div v-else class="text-center text-grey py-8 border rounded">
          <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-server-off</v-icon>
          <div class="text-body-2">Nenhum servidor cadastrado ainda.</div>
        </div>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="pa-4 justify-end">
        <v-btn variant="text" @click="emit('update:modelValue', false)">Fechar</v-btn>
      </v-card-actions>
    </v-card>

    <!-- Confirmação de exclusão -->
    <v-dialog v-model="deleteDialog" max-width="420">
      <v-card class="rounded-lg pa-2">
        <v-card-item>
          <template #prepend>
            <v-avatar color="error" variant="tonal" rounded="lg">
              <v-icon>mdi-delete-alert-outline</v-icon>
            </v-avatar>
          </template>
          <v-card-title class="font-weight-bold">Excluir servidor</v-card-title>
        </v-card-item>
        <v-card-text>
          Remover <strong>{{ serverToDelete?.name }}</strong> da lista? Monitores que já usam este
          endereço continuam funcionando — apenas o atalho deixa de aparecer.
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="deleteDialog = false">Cancelar</v-btn>
          <v-btn color="error" variant="flat" :loading="store.saving" @click="executeDelete">
            Excluir
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="3000" location="top">
      {{ feedback.text }}
    </v-snackbar>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { useDnsServersStore, type DnsServer } from '@/stores/dnsServers'
import { isHostname, isIpAddress } from '@/utils/monitorTypes'

const props = defineProps<{
  modelValue: boolean
  /** Endereço digitado no formulário que abriu o diálogo, já pré-preenchido aqui */
  prefillAddress?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', server: DnsServer): void
}>()

const store = useDnsServersStore()

const PROTOCOL_OPTIONS = [
  { value: 'udp', label: 'UDP' },
  { value: 'tcp', label: 'TCP' },
  { value: 'doh', label: 'DoH (HTTPS)' },
] as const

const formRef = ref<any>(null)
const editingId = ref<number | null>(null)
const deleteDialog = ref(false)
const serverToDelete = ref<DnsServer | null>(null)

const feedback = ref<{ show: boolean; text: string; color: string }>({
  show: false,
  text: '',
  color: 'success',
})

const form = reactive<{
  name: string
  address: string
  protocol: 'udp' | 'tcp' | 'doh'
  isDefault: boolean
  description: string
}>({
  name: '',
  address: '',
  protocol: 'udp',
  isDefault: true,
  description: '',
})

const addressRule = (value: unknown) => {
  const text = String(value ?? '').trim()
  if (!text) return 'Informe o endereço'
  if (form.protocol === 'doh') {
    return /^https:\/\/.+/i.test(text) || 'O endpoint DoH precisa começar com https://'
  }
  const host = text.split(':')[0] ?? ''
  return isIpAddress(host) || isHostname(host) || 'Informe um IP ou hostname válido'
}

const canSubmit = computed(
  () => !!form.name.trim() && addressRule(form.address) === true && !store.saving
)

watch(
  () => props.modelValue,
  (isOpen) => {
    if (!isOpen) return
    store.fetchServers(true)
    resetForm()
    if (props.prefillAddress) {
      form.address = props.prefillAddress
      form.protocol = /^https:\/\//i.test(props.prefillAddress) ? 'doh' : 'udp'
    }
  }
)

function protocolIcon(protocol: string): string {
  if (protocol === 'doh') return 'mdi-lock-outline'
  if (protocol === 'tcp') return 'mdi-transit-connection-variant'
  return 'mdi-lightning-bolt-outline'
}

async function resetForm() {
  editingId.value = null
  form.name = ''
  form.address = ''
  form.protocol = 'udp'
  form.isDefault = true
  form.description = ''
  store.error = null
  await nextTick()
  formRef.value?.resetValidation()
}

async function startEdit(server: DnsServer) {
  editingId.value = server.id
  form.name = server.name
  form.address = server.address
  form.protocol = server.protocol
  form.isDefault = server.isDefault
  form.description = server.description || ''
  await nextTick()
  formRef.value?.resetValidation()
}

async function submit() {
  const { valid } = (await formRef.value?.validate()) || { valid: true }
  if (!valid) return

  const payload = {
    name: form.name.trim(),
    address: form.address.trim(),
    protocol: form.protocol,
    isDefault: form.isDefault,
    description: form.description.trim() || null,
  }

  const isEditing = !!editingId.value
  const saved = editingId.value
    ? await store.updateServer(editingId.value, payload)
    : await store.createServer(payload)

  if (saved) {
    emit('saved', saved)
    await resetForm()
    feedback.value = {
      show: true,
      text: isEditing
        ? 'Servidor DNS atualizado com sucesso!'
        : 'Servidor DNS cadastrado com sucesso!',
      color: 'success',
    }
  }
}

function confirmDelete(server: DnsServer) {
  serverToDelete.value = server
  deleteDialog.value = true
}

async function executeDelete() {
  if (!serverToDelete.value) return
  const removedId = serverToDelete.value.id
  const success = await store.deleteServer(removedId)
  if (success) {
    if (editingId.value === removedId) await resetForm()
    deleteDialog.value = false
    serverToDelete.value = null
    feedback.value = {
      show: true,
      text: 'Servidor DNS removido com sucesso!',
      color: 'info',
    }
  }
}
</script>

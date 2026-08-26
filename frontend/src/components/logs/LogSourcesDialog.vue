<template>
  <v-dialog v-model="open" max-width="900">
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center">
        <v-icon start>mdi-lan-connect</v-icon>
        Origens de syslog vistas
        <v-spacer></v-spacer>
        <v-btn icon="mdi-refresh" variant="text" size="small" @click="refresh"></v-btn>
      </v-card-title>

      <v-card-subtitle class="pb-3">
        Tudo que chegou desde o último reinício do servidor. Origem que não resolve para um
        dispositivo ou para uma rede cadastrada tem as mensagens descartadas — vincule-a aqui para
        começar a guardá-las.
      </v-card-subtitle>

      <v-divider></v-divider>

      <v-card-text style="max-height: 60vh; overflow-y: auto">
        <!--
          O mascaramento muda o significado da coluna "Endereço": ela deixa de
          identificar o equipamento. Sem esta explicação o operador vê trinta
          linhas com o mesmo IP e conclui que o sistema está quebrado.
        -->
        <v-alert
          v-if="logsStore.natMasking"
          type="warning"
          variant="tonal"
          density="comfortable"
          border="start"
          class="mb-4"
        >
          <div class="font-weight-bold mb-1">
            As mensagens chegam com o endereço reescrito pelo Docker.
          </div>
          O que aparece em <strong>Endereço</strong> é
          {{ (logsStore.nat?.gateways ?? []).join(', ') || 'o gateway da bridge' }}, não o
          equipamento — a publicação de porta troca a origem antes de o pacote chegar aqui. Por isso
          o vínculo abaixo é feito pelo <strong>nome</strong> que cada aparelho envia no syslog, e
          quem não envia nome não pode ser separado dos demais. Para receber o endereço real,
          publique o servidor com <code>network_mode: host</code> no
          <code>docker-compose.yml</code>.
        </v-alert>

        <v-table density="compact">
          <thead>
            <tr>
              <th class="text-left">Endereço</th>
              <th class="text-left">Situação</th>
              <th class="text-left">Mensagens</th>
              <th class="text-left" style="width: 260px">Vínculo</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="source in logsStore.sources" :key="source.bindKey">
              <td>
                <div class="d-flex align-center ga-1">
                  <span class="font-weight-medium">{{
                    source.masked ? (source.hostname ?? 'sem nome') : source.sourceIp
                  }}</span>
                  <v-tooltip v-if="source.masked" location="top">
                    <template #activator="{ props: tooltip }">
                      <v-icon v-bind="tooltip" size="14" color="warning">
                        mdi-alert-circle-outline
                      </v-icon>
                    </template>
                    Chegou por {{ source.sourceIp }}, que é o gateway do Docker — o endereço real do
                    equipamento não sobrevive à publicação de porta.
                  </v-tooltip>
                </div>
                <div v-if="!source.masked && source.hostname" class="text-caption text-grey">
                  {{ source.hostname }}
                </div>
                <div v-else-if="source.masked" class="text-caption text-grey">
                  via {{ source.sourceIp }}
                </div>
                <div class="text-caption text-grey log-preview">{{ source.lastMessage }}</div>
              </td>
              <td>
                <v-chip :color="kindColor(source.kind)" size="x-small" variant="tonal">
                  {{ kindLabel(source.kind) }}
                </v-chip>
                <div v-if="source.deviceName" class="text-caption text-grey mt-1">
                  {{ source.deviceName }}
                </div>
              </td>
              <td class="text-caption">
                <div>{{ source.messageCount }} recebidas</div>
                <div v-if="source.droppedCount > 0" class="text-warning">
                  {{ source.droppedCount }} descartadas
                </div>
              </td>
              <td>
                <!--
                  Mascarada e sem hostname não tem o que vincular: o IP é
                  compartilhado, e escolher um dispositivo aqui atribuiria a ele
                  o log de todo o parque. O backend recusa; a tela não oferece.
                -->
                <div v-if="source.masked && !source.hostname" class="text-caption text-grey">
                  Sem nome no syslog — não há como separar este equipamento dos outros atrás do
                  mesmo endereço.
                </div>
                <v-select
                  v-else
                  :model-value="source.deviceId"
                  :items="deviceOptions"
                  item-title="title"
                  item-value="value"
                  label="Dispositivo"
                  hide-details
                  clearable
                  density="compact"
                  variant="outlined"
                  :loading="saving === source.bindKey"
                  @update:model-value="(value) => bind(source.bindKey, value)"
                ></v-select>
              </td>
            </tr>
          </tbody>
        </v-table>

        <div v-if="logsStore.sources.length === 0" class="pa-8 text-center text-grey">
          <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-lan-disconnect</v-icon>
          <div class="text-subtitle-2 font-weight-medium">Nenhuma origem enviou syslog ainda</div>
          <div class="text-caption">
            No cadastro de um novo dispositivo, marque “Configurar envio de logs (Syslog)” para o
            NetMonitor detectar e aplicar a configuração automaticamente.
          </div>
        </div>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="open = false">Fechar</v-btn>
      </v-card-actions>
    </v-card>

    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="6000" location="bottom">
      {{ feedback.text }}
      <template #actions>
        <v-btn variant="text" @click="feedback.show = false">Fechar</v-btn>
      </template>
    </v-snackbar>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useLogsStore } from '@/stores/logs'
import { useDevicesStore } from '@/stores/devices'

const open = defineModel<boolean>({ required: true })

const logsStore = useLogsStore()
const devicesStore = useDevicesStore()
const saving = ref<string | null>(null)

/** Padrão da casa: `v-snackbar` local, sem store de toast compartilhada. */
const feedback = reactive({ show: false, text: '', color: 'success' })

const deviceOptions = computed(() =>
  devicesStore.devices.map((device) => ({ title: device.name, value: device.id }))
)

/**
 * `ambiguous` merece cor de aviso, não de erro: o log **está** sendo guardado,
 * só não vinculado. Tratá-lo como falha faria o operador procurar um problema
 * que não existe.
 */
function kindColor(kind: string): string {
  switch (kind) {
    case 'device':
      return 'success'
    case 'network':
      return 'info'
    case 'ambiguous':
      return 'warning'
    default:
      return 'error'
  }
}

function kindLabel(kind: string): string {
  switch (kind) {
    case 'device':
      return 'Vinculada'
    case 'network':
      return 'Rede conhecida'
    case 'ambiguous':
      return 'Ambígua'
    default:
      return 'Descartando'
  }
}

/**
 * `bindKey` e não o IP: atrás de NAT a chave é `host:<nome>`, e é o servidor
 * que decide qual das duas vale para cada origem.
 *
 * A falha precisa aparecer: o servidor recusa o vínculo do gateway do NAT com
 * uma explicação inteira, e sem mostrá-la o seletor voltava a vazio sem dizer
 * nada — indistinguível de "não funcionou".
 */
async function bind(bindKey: string, deviceId: number | null): Promise<void> {
  saving.value = bindKey
  feedback.show = false
  try {
    await logsStore.bindSource(bindKey, deviceId)
    feedback.color = 'success'
    feedback.text = deviceId
      ? 'Vínculo salvo — as próximas mensagens desta origem serão guardadas.'
      : 'Vínculo removido. A origem volta a ser resolvida pelo cadastro.'
    feedback.show = true
  } catch (erro) {
    feedback.color = 'error'
    feedback.text =
      erro instanceof Error && erro.message.trim()
        ? erro.message.trim()
        : 'Não foi possível salvar o vínculo.'
    feedback.show = true
  } finally {
    saving.value = null
  }
}

function refresh(): void {
  void logsStore.fetchSources()
}

watch(open, (aberto) => {
  if (aberto) {
    void logsStore.fetchSources()
    void devicesStore.fetchDevices()
  }
})
</script>

<style scoped>
.log-preview {
  font-family: 'Roboto Mono', 'Courier New', monospace;
  font-size: 0.7rem;
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>

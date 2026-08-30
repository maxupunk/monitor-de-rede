<template>
  <v-dialog v-model="open" max-width="820">
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center">
        <v-icon start>mdi-console</v-icon>
        Configurar envio de syslog
      </v-card-title>

      <v-card-subtitle class="pb-3">
        Cole os comandos no equipamento. Os registros começam a aparecer em poucos segundos — desde
        que o endereço abaixo seja alcançável a partir dele.
      </v-card-subtitle>

      <v-divider></v-divider>

      <v-card-text>
        <!--
          O endereço é escolhido, não deduzido: os comandos abaixo são colados
          dentro do roteador, e qual endereço serve depende de onde ele está.
        -->
        <div class="d-flex align-center flex-wrap ga-3 mb-4">
          <!--
            O botão fica **colado** no campo, e não empurrado para a ponta da
            linha pelo `v-spacer`: ele existe para resolver a falta de um
            endereço na própria lista logo acima, e a distância desfazia essa
            leitura.
          -->
          <div class="d-flex align-center ga-2" style="min-width: 320px">
            <v-select
              v-model="addressChoice"
              :items="addressOptions"
              item-title="title"
              item-value="value"
              label="Endereço que os equipamentos vão usar"
              density="compact"
              variant="outlined"
              hide-details
              class="flex-grow-1 min-width-0"
              :item-props="addressItemProps"
            ></v-select>
            <ServerAddressesButton @saved="recarrega" />
          </div>
          <div v-if="guide" class="text-body-2">
            <span class="text-grey">Porta:</span>
            <strong class="ml-1">{{ guide.port }}/udp e /tcp</strong>
          </div>
        </div>

        <v-alert
          v-if="addressesStore.isEmpty"
          type="warning"
          variant="tonal"
          density="compact"
          class="mb-4"
        >
          Nenhum endereço definido para este servidor ainda. Sem ele os comandos abaixo saem com um
          marcador no lugar do IP e não funcionam quando colados.
        </v-alert>

        <v-tabs v-model="sistema" density="compact" class="mb-3">
          <v-tab
            v-for="snippet in guide?.snippets ?? []"
            :key="snippet.system"
            :value="snippet.system"
          >
            {{ snippet.label }}
          </v-tab>
        </v-tabs>

        <v-window v-model="sistema">
          <v-window-item
            v-for="snippet in guide?.snippets ?? []"
            :key="snippet.system"
            :value="snippet.system"
          >
            <p class="text-caption text-grey mb-3">{{ snippet.note }}</p>
            <v-sheet class="pa-3 rounded-lg border position-relative" color="surface-variant">
              <v-btn
                :icon="copied === snippet.system ? 'mdi-check' : 'mdi-content-copy'"
                :color="copied === snippet.system ? 'success' : undefined"
                variant="text"
                size="small"
                class="copy-button"
                @click="copy(snippet.system, snippet.commands)"
              ></v-btn>
              <pre class="snippet">{{ snippet.commands }}</pre>
            </v-sheet>
          </v-window-item>
        </v-window>

        <div v-if="!guide" class="pa-8 text-center text-grey">
          <v-progress-circular indeterminate size="32"></v-progress-circular>
        </div>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="open = false">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useLogsStore } from '@/stores/logs'
import { useServerAddressesStore } from '@/stores/serverAddresses'
import ServerAddressesButton from '@/components/ServerAddressesButton.vue'

const open = defineModel<boolean>({ required: true })

const logsStore = useLogsStore()
const addressesStore = useServerAddressesStore()
const sistema = ref('')
const copied = ref<string | null>(null)
const addressChoice = ref('')

const guide = computed(() => logsStore.setupGuide)

interface AddressOption {
  value: string
  title: string
  subtitle: string
}

const addressOptions = computed<AddressOption[]>(() =>
  addressesStore.usable.map((entrada) => ({
    value: entrada.value ?? '',
    title: `${entrada.label} — ${entrada.value}`,
    subtitle: entrada.description,
  }))
)

function addressItemProps(item: AddressOption): Record<string, unknown> {
  return { subtitle: item.subtitle }
}

/**
 * Recarrega o guia com o endereço escolhido.
 *
 * Os comandos são gerados no backend com o endereço estampado; trocar a escolha
 * precisa regerá-los, senão o operador copiaria o comando do endereço anterior.
 */
async function recarrega(): Promise<void> {
  await addressesStore.fetchAll(true)
  if (!addressChoice.value || !addressesStore.usable.some((e) => e.value === addressChoice.value)) {
    const preferido = addressesStore.usable.find((e) => e.id === addressesStore.preferredId)
    addressChoice.value = (preferido ?? addressesStore.usable[0])?.value ?? ''
  }
  await logsStore.fetchSetupGuide(addressChoice.value || null)
}

watch(addressChoice, (endereco) => {
  if (endereco) void logsStore.fetchSetupGuide(endereco)
})

watch(guide, (current) => {
  const snippets = current?.snippets ?? []
  if (!snippets.some((snippet) => snippet.system === sistema.value)) {
    sistema.value = snippets[0]?.system ?? ''
  }
})

async function copy(chave: string, texto: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(texto)
    copied.value = chave
    // O aviso de "copiado" some sozinho: um estado que fica preso na tela
    // deixa dúvida se a segunda cópia funcionou.
    setTimeout(() => {
      if (copied.value === chave) copied.value = null
    }, 2000)
  } catch {
    // Navegador sem permissão de área de transferência (http em host remoto):
    // o texto continua selecionável na tela.
  }
}

watch(open, (aberto) => {
  if (aberto) void recarrega()
})
</script>

<style scoped>
/* Sem isto o campo se recusa a encolher e empurra o botão para fora da linha. */
.min-width-0 {
  min-width: 0;
}

.snippet {
  font-family: 'Roboto Mono', 'Courier New', monospace;
  font-size: 0.8125rem;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  padding-right: 40px;
}

.copy-button {
  position: absolute;
  top: 4px;
  right: 4px;
}
</style>

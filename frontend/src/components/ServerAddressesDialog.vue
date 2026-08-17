<template>
  <v-dialog
    v-model="open"
    :max-width="$vuetify.display.xs ? undefined : 780"
    :fullscreen="$vuetify.display.xs"
    scrollable
  >
    <v-card class="rounded-lg">
      <v-card-item class="pa-5 pb-3">
        <template #prepend>
          <v-avatar color="primary" size="44" rounded="lg" variant="tonal">
            <v-icon size="24">mdi-server-network</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-h6">Endereços deste servidor</v-card-title>
        <!--
          Esta frase é o recurso inteiro. Sem ela a tela é uma lista de três IPs
          sem critério; com ela, o operador entende por que existe mais de um e
          qual escolher, sem precisar de nenhuma explicação adicional.
        -->
        <v-card-subtitle class="text-wrap">
          Um servidor, várias portas de entrada. Cada equipamento alcança o NetMonitor pelo endereço
          da rede em que ele está — quem fica na mesma rede usa um, quem chega pelo túnel usa outro,
          quem vem pela internet usa um terceiro.
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

        <div v-if="store.loading" class="py-10 text-center">
          <v-progress-circular indeterminate size="32"></v-progress-circular>
        </div>

        <template v-else>
          <v-sheet
            v-for="entrada in rows"
            :key="entrada.id"
            border
            rounded
            class="pa-4 mb-3"
            :class="{ 'border-primary': entrada.id === preferred }"
          >
            <div class="d-flex align-start ga-3">
              <v-avatar :color="addressColor(entrada.kind)" size="36" rounded="lg" variant="tonal">
                <v-icon size="20">{{ addressIcon(entrada.kind) }}</v-icon>
              </v-avatar>

              <div class="flex-grow-1 min-width-0">
                <div class="d-flex align-center ga-2 flex-wrap">
                  <span class="font-weight-bold">{{ entrada.label }}</span>
                  <v-chip
                    v-if="entrada.id === preferred"
                    size="x-small"
                    color="primary"
                    variant="flat"
                  >
                    Padrão
                  </v-chip>
                </div>
                <div class="text-caption text-medium-emphasis">{{ entrada.description }}</div>

                <v-text-field
                  v-model="drafts[entrada.id]"
                  :placeholder="entrada.detected ?? 'Nenhum endereço definido'"
                  density="compact"
                  variant="outlined"
                  hide-details="auto"
                  class="mt-3"
                  :error-messages="rowErrors[entrada.id]"
                  @update:model-value="rowErrors[entrada.id] = ''"
                ></v-text-field>

                <!--
                  A procedência fica **abaixo** do campo, não dentro dele. No
                  `append-inner` ela dividia a largura com o valor digitado e não
                  quebrava linha: o motivo de um endereço não ter sido detectado
                  é uma frase inteira, e era justamente ela que ficava cortada.
                -->
                <div class="d-flex align-start ga-1 mt-2">
                  <v-icon :color="sourceTone(entrada).color" size="14" class="source-icon">
                    {{ sourceTone(entrada).icon }}
                  </v-icon>
                  <span class="text-caption source-text" :class="sourceTone(entrada).textClass">
                    {{ sourceLabel(entrada) }}
                  </span>
                </div>
              </div>

              <div class="d-flex flex-column ga-1">
                <v-tooltip location="top" text="Usar como padrão">
                  <template #activator="{ props: tip }">
                    <v-btn
                      v-bind="tip"
                      :icon="entrada.id === preferred ? 'mdi-star' : 'mdi-star-outline'"
                      :color="entrada.id === preferred ? 'primary' : undefined"
                      variant="text"
                      size="small"
                      :disabled="!drafts[entrada.id] && !entrada.detected"
                      @click="togglePreferred(entrada.id)"
                    ></v-btn>
                  </template>
                </v-tooltip>
                <!--
                  Detectado corrigido volta ao detectado; personalizado é
                  removido. São ações diferentes com o mesmo gesto, e o ícone
                  precisa dizer qual é.
                -->
                <v-tooltip
                  location="top"
                  :text="entrada.kind === 'custom' ? 'Remover' : 'Voltar ao detectado'"
                >
                  <template #activator="{ props: tip }">
                    <v-btn
                      v-bind="tip"
                      :icon="
                        entrada.kind === 'custom' ? 'mdi-delete-outline' : 'mdi-backup-restore'
                      "
                      variant="text"
                      size="small"
                      :disabled="!canReset(entrada)"
                      @click="reset(entrada)"
                    ></v-btn>
                  </template>
                </v-tooltip>
              </div>
            </div>
          </v-sheet>

          <!--
            Adicionar personalizado. Sem `color="surface-variant"`: aquele tom
            baixa o contraste do rótulo e do campo junto com o fundo, e destoava
            das linhas acima. O bloco usa a mesma moldura delas — `border` +
            `rounded` — que é também o padrão do diálogo de servidores DNS.
          -->
          <v-sheet border rounded class="pa-4 mt-4">
            <div class="text-overline mb-3">Adicionar outro endereço</div>
            <v-row dense>
              <v-col cols="12" sm="5">
                <v-text-field
                  v-model="novo.label"
                  label="Quando usar"
                  placeholder="Ex: Filial Norte"
                  density="compact"
                  variant="outlined"
                  hide-details="auto"
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="5">
                <v-text-field
                  v-model="novo.value"
                  label="Endereço"
                  placeholder="IP ou nome DNS"
                  density="compact"
                  variant="outlined"
                  hide-details="auto"
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="2" class="d-flex align-center">
                <v-btn
                  color="primary"
                  variant="flat"
                  block
                  :disabled="!novo.label.trim() || !novo.value.trim()"
                  @click="adicionar"
                >
                  Adicionar
                </v-btn>
              </v-col>
            </v-row>
          </v-sheet>
        </template>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="pa-4">
        <v-btn variant="text" size="small" @click="store.fetchAll(true)">
          <v-icon start>mdi-refresh</v-icon>
          Redetectar
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn variant="text" :disabled="store.saving" @click="open = false">Cancelar</v-btn>
        <v-btn color="primary" variant="flat" :loading="store.saving" @click="salvar">
          Salvar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { reactive, ref, watch, computed } from 'vue'
import {
  useServerAddressesStore,
  addressIcon,
  addressColor,
  type ServerAddressEntry,
} from '@/stores/serverAddresses'

const open = defineModel<boolean>({ required: true })
const emit = defineEmits<{ saved: [] }>()

const store = useServerAddressesStore()

/** Rascunho por id: o que está no campo, ainda não gravado. */
const drafts = reactive<Record<string, string>>({})
const rowErrors = reactive<Record<string, string>>({})
const preferred = ref<string | null>(null)
const novo = reactive({ label: '', value: '' })

/** Personalizados adicionados nesta sessão, ainda sem id do servidor. */
const adicionados = ref<{ id: string; label: string; value: string; kind: string }[]>([])

const rows = computed<ServerAddressEntry[]>(() => [
  ...store.entries,
  ...adicionados.value.map((item) => ({
    id: item.id,
    kind: 'custom',
    label: item.label,
    description: 'Endereço definido por você',
    value: item.value,
    detected: null,
    overridden: true,
    source: 'ainda não salvo',
  })),
])

/**
 * O texto à direita do campo. É ele que impede um palpite de ser lido como
 * certeza — "detectado neste servidor" e "corrigido por você" pesam diferente
 * na hora de confiar no valor.
 */
function sourceLabel(entrada: ServerAddressEntry): string {
  const rascunho = (drafts[entrada.id] ?? '').trim()
  if (rascunho && rascunho !== (entrada.detected ?? '')) return 'corrigido por você'
  return entrada.source
}

/**
 * Ícone e cor da procedência.
 *
 * O texto do "não detectado" é uma frase inteira, e uma frase corrida em cinza
 * vira parede. O ícone dá o tom antes da leitura — âmbar quando falta endereço,
 * neutro quando há — e é ele que faz a linha ser escaneável em vez de lida.
 */
function sourceTone(entrada: ServerAddressEntry): {
  icon: string
  color: string
  textClass: string
} {
  const rascunho = (drafts[entrada.id] ?? '').trim()
  if (rascunho && rascunho !== (entrada.detected ?? '')) {
    return { icon: 'mdi-pencil-outline', color: 'primary', textClass: 'text-primary' }
  }
  if (!entrada.value) {
    return {
      icon: 'mdi-alert-circle-outline',
      color: 'warning',
      textClass: 'text-warning',
    }
  }
  return { icon: 'mdi-check-circle-outline', color: 'success', textClass: 'text-medium-emphasis' }
}

function canReset(entrada: ServerAddressEntry): boolean {
  if (entrada.kind === 'custom') return true
  return Boolean((drafts[entrada.id] ?? '').trim())
}

function reset(entrada: ServerAddressEntry): void {
  if (entrada.kind === 'custom') {
    adicionados.value = adicionados.value.filter((item) => item.id !== entrada.id)
    delete drafts[entrada.id]
    if (preferred.value === entrada.id) preferred.value = null
    return
  }
  drafts[entrada.id] = ''
  rowErrors[entrada.id] = ''
}

function togglePreferred(id: string): void {
  preferred.value = preferred.value === id ? null : id
}

function adicionar(): void {
  const id = `novo:${Date.now()}:${adicionados.value.length}`
  adicionados.value.push({
    id,
    kind: 'custom',
    label: novo.label.trim(),
    value: novo.value.trim(),
  })
  drafts[id] = novo.value.trim()
  novo.label = ''
  novo.value = ''
}

async function salvar(): Promise<void> {
  // Só o que diverge do detectado vira correção: mandar o detectado de volta o
  // congelaria, e o IP da rede local mudaria sem a tela perceber.
  const overrides: Record<string, string> = {}
  for (const entrada of store.entries) {
    if (entrada.kind === 'custom') continue
    const rascunho = (drafts[entrada.id] ?? '').trim()
    if (rascunho && rascunho !== (entrada.detected ?? '')) overrides[entrada.id] = rascunho
  }

  const custom = [
    ...store.entries
      .filter((entrada) => entrada.kind === 'custom')
      .map((entrada) => ({
        id: entrada.id,
        label: entrada.label,
        value: (drafts[entrada.id] ?? entrada.value ?? '').trim(),
      })),
    ...adicionados.value.map((item) => ({
      id: '',
      label: item.label,
      value: (drafts[item.id] ?? item.value).trim(),
    })),
  ].filter((item) => item.value)

  // Um padrão apontando para um item que ainda não tem id do servidor não
  // sobreviveria à gravação; deixar em branco é melhor que apontar para nada.
  const preferredId = preferred.value?.startsWith('novo:') ? null : preferred.value

  const ok = await store.save({ overrides, custom, preferredId })
  if (!ok) return
  adicionados.value = []
  sincronizaRascunhos()
  emit('saved')
  open.value = false
}

function sincronizaRascunhos(): void {
  Object.keys(drafts).forEach((chave) => delete drafts[chave])
  Object.keys(rowErrors).forEach((chave) => delete rowErrors[chave])
  for (const entrada of store.entries) {
    // Só a correção vai para o campo: o detectado fica de `placeholder`, e é
    // isso que deixa "voltar ao detectado" ser simplesmente apagar o texto.
    drafts[entrada.id] = entrada.overridden ? (entrada.value ?? '') : ''
  }
  preferred.value = store.preferredId
}

watch(open, async (aberto) => {
  if (!aberto) return
  adicionados.value = []
  novo.label = ''
  novo.value = ''
  await store.fetchAll(true)
  sincronizaRascunhos()
})
</script>

<style scoped>
/*
 * Sem isto o conteúdo do flex herda `min-width: auto` e se recusa a encolher —
 * é a causa clássica de texto que estoura o container em vez de quebrar.
 */
.min-width-0 {
  min-width: 0;
}

.source-text {
  line-height: 1.35;
  overflow-wrap: anywhere;
}

/* Alinha o ícone com a primeira linha do texto, e não com o bloco inteiro. */
.source-icon {
  margin-top: 2px;
  flex: 0 0 auto;
}
</style>

<template>
  <div>
    <div class="d-flex flex-wrap align-center justify-space-between ga-3 mb-4">
      <div>
        <div class="text-subtitle-1 font-weight-bold d-flex align-center ga-2">
          <v-icon color="primary">mdi-bell-cog-outline</v-icon>
          Regras que valem aqui ({{ regras.length }})
        </div>
        <!--
          A contagem separa as duas origens porque elas se comportam de forma
          diferente: apagar uma regra deste equipamento afeta só ele; apagar
          uma global afeta o inventário inteiro.
        -->
        <div class="text-caption text-grey mt-1">
          {{ contagem }} São as mesmas regras da Central de Alertas — editar aqui ou lá altera o
          mesmo registro.
        </div>
      </div>
      <div class="d-flex flex-wrap ga-2">
        <v-btn
          color="primary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-star-outline"
          @click="catalogDialog = true"
        >
          Regras pré-configuradas
        </v-btn>
        <v-btn color="primary" size="small" prepend-icon="mdi-plus" @click="abrirFormulario()">
          Criar personalizada
        </v-btn>
      </div>
    </div>

    <v-alert
      v-if="alertsStore.error"
      type="error"
      variant="tonal"
      density="comfortable"
      class="mb-4 rounded-lg"
    >
      {{ alertsStore.error }}
    </v-alert>

    <div v-if="alertsStore.loading && regras.length === 0" class="py-8 text-center">
      <v-progress-circular indeterminate color="primary" />
    </div>

    <v-alert
      v-else-if="regras.length === 0"
      type="info"
      variant="tonal"
      density="comfortable"
      class="rounded-lg"
    >
      Nenhuma regra vinculada a este dispositivo. Use as regras pré-configuradas para começar — só
      aparecem as compatíveis com o que este equipamento mede.
    </v-alert>

    <div v-else class="table-responsive">
      <v-table hover density="comfortable" class="rounded-lg border">
        <thead>
          <tr>
            <th>Regra</th>
            <th>Alcance</th>
            <th>Condição</th>
            <th>Severidade</th>
            <th class="text-center">Ativa</th>
            <th class="text-end">Ações</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="regra in regras" :key="regra.id">
            <td class="font-weight-medium">
              {{ regra.name }}
              <div v-if="escopoDaRegra(regra)" class="text-caption text-grey">
                {{ escopoDaRegra(regra) }}
              </div>
            </td>
            <!--
              O rótulo de alcance é obrigatório: sem ele, uma regra global
              listada aqui pareceria pertencer a este equipamento, e excluí-la
              seria uma surpresa cara.
            -->
            <td>
              <v-chip
                :color="ehGlobal(regra) ? 'warning' : 'primary'"
                size="x-small"
                label
                variant="tonal"
              >
                <v-icon start size="12">
                  {{ ehGlobal(regra) ? 'mdi-earth' : 'mdi-router-network' }}
                </v-icon>
                {{ ehGlobal(regra) ? 'Todos os dispositivos' : 'Este dispositivo' }}
              </v-chip>
            </td>
            <td class="text-caption">
              {{
                describeRule(
                  regra.condition,
                  regra.durationSeconds ?? 0,
                  regra.recoveryWindowSeconds ?? 0,
                  regra.flapThreshold ?? 0,
                  regra.flapWindowSeconds ?? 900,
                  {
                    notificationCooldownSeconds: regra.notificationCooldownSeconds ?? 0,
                    inhibitWhenParentDown: regra.inhibitWhenParentDown ?? false,
                  }
                )
              }}
            </td>
            <td>
              <v-chip :color="severityColor(regra.severity)" size="x-small" label>
                {{ severityLabel(regra.severity) }}
              </v-chip>
            </td>
            <td class="text-center">
              <v-switch
                :model-value="regra.enabled !== false"
                color="primary"
                density="compact"
                hide-details
                inset
                :aria-label="`Ativar ou desativar a regra ${regra.name}`"
                @update:model-value="alternar(regra, $event)"
              />
            </td>
            <td class="text-end text-no-wrap">
              <v-btn
                icon
                size="small"
                variant="text"
                :aria-label="`Editar a regra ${regra.name}`"
                @click="abrirFormulario(regra)"
              >
                <v-icon size="18">mdi-pencil</v-icon>
                <v-tooltip activator="parent" location="top">Editar regra</v-tooltip>
              </v-btn>
              <v-btn
                icon
                size="small"
                variant="text"
                color="error"
                :aria-label="`Excluir a regra ${regra.name}`"
                @click="confirmarExclusao(regra)"
              >
                <v-icon size="18">mdi-delete-outline</v-icon>
                <v-tooltip activator="parent" location="top">Excluir</v-tooltip>
              </v-btn>
            </td>
          </tr>
        </tbody>
      </v-table>
    </div>

    <AlertRuleCatalogDialog
      v-model="catalogDialog"
      :scope="escopo"
      :scope-label="deviceName"
      @applied="onAplicado"
    />

    <!--
      O **mesmo** componente da Central de Alertas, com o dispositivo já
      preenchido: quem abriu o formulário de dentro do equipamento não deveria
      ter de escolhê-lo de novo. O escopo fica restrito a duas opções — este
      equipamento ou o parque inteiro —, e não travado.
    -->
    <AlertRuleFormDialog
      v-model="formDialog"
      :rule="regraEmEdicao"
      :fixed-device-id="deviceId"
      :fixed-device-name="deviceName"
      :available-fields="availableFields"
      @saved="onAplicado"
    />

    <!--
      Excluir daqui apaga o registro, não o vínculo. Numa regra global isso
      atinge todo o inventário, e o diálogo precisa dizer isso com todas as
      letras antes de o operador confirmar.
    -->
    <v-dialog v-model="exclusaoDialog" max-width="460">
      <v-card class="rounded-lg pa-2">
        <v-card-item>
          <template #prepend>
            <v-avatar color="error" variant="tonal" rounded="lg">
              <v-icon>mdi-delete-alert-outline</v-icon>
            </v-avatar>
          </template>
          <v-card-title class="font-weight-bold">Excluir regra</v-card-title>
        </v-card-item>
        <v-card-text>
          <p class="mb-2">
            A regra <strong>{{ regraParaExcluir?.name }}</strong> será removida permanentemente.
          </p>
          <v-alert
            v-if="regraParaExcluir && ehGlobal(regraParaExcluir)"
            type="warning"
            variant="tonal"
            density="compact"
            class="rounded-lg"
          >
            Esta regra vale para <strong>todos os dispositivos</strong>. Excluí-la aqui a remove do
            inventário inteiro, não só deste equipamento. Para parar de avaliá-la sem apagá-la, use
            o interruptor "Ativa".
          </v-alert>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="exclusaoDialog = false">Cancelar</v-btn>
          <v-btn color="error" variant="flat" :loading="excluindo" @click="excluir">
            Excluir
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
/**
 * A aba **Regras** de um dispositivo qualquer.
 *
 * Não é um segundo gerenciador de regras: usa a mesma store, o mesmo catálogo,
 * o mesmo diálogo de templates, o mesmo formulário e os mesmos registros de
 * `/alerts`. O que muda é só o recorte — `deviceId` — e o fato de o catálogo já
 * nascer com esse escopo fixado.
 *
 * A aba lista **o que é avaliado nas checagens deste equipamento**, e não "o
 * que aponta para o id dele": uma regra global também vale aqui. Filtrar
 * apenas por `deviceId` fazia uma regra criada nesta tela com escopo "todos"
 * desaparecer dela — indistinguível de a criação ter falhado.
 */
import { computed, onMounted, ref, watch } from 'vue'
import AlertRuleCatalogDialog from '@/components/AlertRuleCatalogDialog.vue'
import AlertRuleFormDialog from '@/components/AlertRuleFormDialog.vue'
import { useAlertsStore, type AlertRule, type AlertRuleScope } from '@/stores/alerts'
import { describeRule, severityColor, severityLabel } from '@/utils/alertPresentation'

const props = defineProps<{
  deviceId: number
  deviceName?: string
  /** Monitores do dispositivo, para descrever o escopo de cada regra. */
  monitorNames?: Record<number, string>
  /**
   * O vocabulário que este dispositivo publica, vindo das capacidades.
   * O formulário só oferece métricas desta lista enquanto o escopo for este
   * equipamento — uma regra sobre um campo que ele não mede nunca dispararia.
   */
  availableFields?: string[]
}>()

const emit = defineEmits<{ (e: 'changed'): void }>()

const alertsStore = useAlertsStore()
const catalogDialog = ref(false)
const formDialog = ref(false)
const regraEmEdicao = ref<AlertRule | null>(null)
const exclusaoDialog = ref(false)
const excluindo = ref(false)
const regraParaExcluir = ref<AlertRule | null>(null)

function abrirFormulario(regra?: AlertRule): void {
  regraEmEdicao.value = regra ?? null
  formDialog.value = true
}

const escopo = computed<AlertRuleScope>(() => ({ deviceId: props.deviceId }))

/** Sem site, dispositivo nem monitor: vale para todo o inventário. */
function ehGlobal(regra: AlertRule): boolean {
  return regra.deviceId == null && regra.monitorId == null && regra.siteId == null
}

/**
 * A lista já vem filtrada do backend. O filtro em memória existe porque a
 * store é compartilhada com `/alerts`: se aquela tela recarregar a lista
 * completa enquanto esta aba está aberta, a tabela passaria a mostrar o parque
 * inteiro dentro da página de um equipamento.
 *
 * As locais vêm primeiro: quem abre a aba está olhando este equipamento, e o
 * que é dele é a resposta mais provável à pergunta que o trouxe aqui.
 */
const regras = computed(() => {
  const relevantes = alertsStore.alertRules.filter(
    (regra) =>
      regra.deviceId === props.deviceId ||
      (regra.monitorId != null && props.monitorNames?.[regra.monitorId] !== undefined) ||
      ehGlobal(regra)
  )
  return [
    ...relevantes.filter((regra) => !ehGlobal(regra)),
    ...relevantes.filter((regra) => ehGlobal(regra)),
  ]
})

const contagem = computed(() => {
  const globais = regras.value.filter(ehGlobal).length
  const locais = regras.value.length - globais
  return `${locais} deste dispositivo · ${globais} ${globais === 1 ? 'global' : 'globais'}.`
})

function escopoDaRegra(regra: AlertRule): string {
  if (regra.monitorId != null) {
    return `Monitor: ${props.monitorNames?.[regra.monitorId] ?? `#${regra.monitorId}`}`
  }
  return ''
}

async function carregar(): Promise<void> {
  await alertsStore.fetchAlertRules({ deviceId: props.deviceId, includeGlobal: true })
}

async function alternar(regra: AlertRule, valor: boolean | null): Promise<void> {
  await alertsStore.updateAlertRule(regra.id, { enabled: valor === true })
  emit('changed')
}

function confirmarExclusao(regra: AlertRule): void {
  regraParaExcluir.value = regra
  exclusaoDialog.value = true
}

async function excluir(): Promise<void> {
  if (!regraParaExcluir.value) return
  excluindo.value = true
  try {
    const ok = await alertsStore.deleteAlertRule(regraParaExcluir.value.id)
    if (!ok) return
    exclusaoDialog.value = false
    regraParaExcluir.value = null
    await onAplicado()
  } finally {
    excluindo.value = false
  }
}

async function onAplicado(): Promise<void> {
  await carregar()
  emit('changed')
}

onMounted(carregar)
watch(() => props.deviceId, carregar)
</script>

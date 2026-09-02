<template>
  <div>
    <div class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2">
      <v-icon color="primary">mdi-chip</v-icon>
      Saúde do equipamento
    </div>

    <v-row class="mb-2">
      <!--
        Duas por linha, e não três: o resumo de saúde é a informação principal
        da Visão Geral, e a largura precisa acompanhar essa hierarquia — CPU e
        memória lado a lado, cada uma com espaço para o sparkline ser lido.
        `cols="12"` no celular porque dois cards espremidos num telefone
        matariam justamente o sparkline, que é a única coisa que o card tem de
        próprio.
      -->
      <v-col v-for="card in cards" :key="card.serie" cols="12" sm="6">
        <!--
          O card é o ponto de entrada do gráfico daquela série — a regra de
          layout do roadmap: nada de aba depósito, o detalhe abre de onde o
          número apareceu. `role`/`tabindex` mantêm a navegação por teclado.
        -->
        <v-card
          border
          flat
          class="pa-4 rounded-lg h-100 card-clicavel"
          role="button"
          tabindex="0"
          :aria-label="`Ver histórico de ${card.titulo}`"
          @click="abrir(card)"
          @keydown.enter="abrir(card)"
          @keydown.space.prevent="abrir(card)"
        >
          <div class="d-flex align-center justify-space-between mb-2 ga-2">
            <span class="text-subtitle-2 font-weight-bold d-flex align-center ga-2">
              <v-icon size="18" :color="card.disponivel ? card.cor : 'grey'">{{
                card.icone
              }}</v-icon>
              {{ card.titulo }}
            </span>
            <v-chip size="x-small" :color="card.disponivel ? card.cor : 'grey'" variant="tonal">
              {{ card.textoValor }}
            </v-chip>
          </div>

          <v-progress-linear
            v-if="card.percentual"
            :model-value="card.disponivel ? (card.progresso ?? 0) : 0"
            height="10"
            rounded
            :color="card.disponivel ? card.cor : 'grey-lighten-2'"
            class="mb-3"
          />

          <MonitorSparkline
            v-if="card.historico.length > 1"
            :data="card.historico"
            :color="card.corHex"
            :width="220"
            :height="32"
            class="mb-3"
          />

          <div class="d-flex align-center justify-space-between text-caption text-grey ga-2">
            <span class="text-truncate">{{ card.legenda }}</span>
            <span class="text-no-wrap d-flex align-center ga-1">
              {{ card.coletadoEm }}
              <v-icon size="14">mdi-chart-line</v-icon>
            </span>
          </div>
        </v-card>
      </v-col>
    </v-row>

    <MetricHistoryDialog
      v-model="dialogAberto"
      :metric-names="cardAberto?.nomes ?? []"
      :title="cardAberto?.titulo ?? ''"
      :icon="cardAberto?.icone"
      :color="cardAberto?.corHex"
      :unit-type="cardAberto?.tipoDeUnidade ?? 'generic'"
      :metrics="metrics"
      :format="cardAberto?.formata"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * Resumo de saúde de **um dispositivo qualquer**.
 *
 * Não é um painel do servidor: recebe a lista de métricas do dispositivo e
 * monta um card por série de saúde encontrada. O Servidor NetMonitor é o
 * primeiro a preenchê-los todos porque é quem coleta CPU, memória,
 * armazenamento, carga, memória de processo e uptime; um roteador SNMP
 * preenche os dois que o SNMP publica. Nenhum caminho de código distingue os
 * dois casos.
 *
 * A decisão de "o que está sendo coletado" vem das **séries gravadas**, e não
 * do nome do monitor. Deduzir por nome — como a versão anterior fazia,
 * procurando `cpu` no rótulo — quebra ao renomear o monitor e mente quando
 * alguém chama um monitor de ping de "CPU do roteador".
 */
import { computed, ref } from 'vue'
import MetricHistoryDialog from '@/components/devices/MetricHistoryDialog.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import type { DeviceMetric } from '@/stores/deviceDetail'
import { gaugeHexColor } from '@/utils/monitorPresentation'
import { formatBytes } from '@/utils/formatters'

const props = defineProps<{
  metrics: DeviceMetric[]
}>()

/** Teto de amostras da mini tendência. */
const SPARKLINE_LIMIT = 30

interface DefinicaoSerie {
  serie: string
  /** Nomes alternativos herdados do esquema antigo. */
  alternativos?: string[]
  titulo: string
  icone: string
  legenda: string
  /** Valor de 0 a 100, com barra de progresso. */
  percentual: boolean
  /** Faixas de alerta, para a cor do card. */
  atencao?: number
  critico?: number
  formata?: (valor: number) => string
  /** Como o gráfico rotula o eixo. */
  tipoDeUnidade?: 'bandwidth' | 'bytes' | 'latency' | 'percentage' | 'generic'
}

/**
 * As séries de saúde, na ordem em que aparecem.
 *
 * Os nomes são os de `metrics.name` no backend
 * (`services::monitoring::health::series`) — a mesma família que o SNMP já
 * gravava, e é por isso que os cards de CPU e memória funcionam para o
 * servidor sem uma linha de código nova.
 */
const DEFINICOES: DefinicaoSerie[] = [
  {
    serie: 'cpu_usage',
    tipoDeUnidade: 'percentage',
    titulo: 'Uso de CPU',
    icone: 'mdi-chip',
    legenda: 'Percentual utilizado',
    percentual: true,
    atencao: 65,
    critico: 85,
  },
  {
    serie: 'memory_used_bytes',
    tipoDeUnidade: 'bytes',
    titulo: 'Memória usada',
    icone: 'mdi-memory',
    legenda: 'Quantidade em uso',
    percentual: true,
    atencao: 75,
    critico: 90,
    formata: formatBytes,
  },
  {
    serie: 'storage_usage',
    tipoDeUnidade: 'percentage',
    titulo: 'Armazenamento usado',
    icone: 'mdi-harddisk',
    legenda: 'Percentual ocupado do volume de dados',
    percentual: true,
    atencao: 75,
    critico: 85,
  },
  {
    serie: 'load_average_1m',
    titulo: 'Carga média (1 min)',
    icone: 'mdi-speedometer',
    legenda: 'Processos aguardando execução',
    percentual: false,
    formata: (valor) => valor.toFixed(2),
  },
  {
    serie: 'process_memory_bytes',
    tipoDeUnidade: 'bytes',
    titulo: 'Memória do processo',
    icone: 'mdi-application-cog-outline',
    legenda: 'Residente, do processo do NetMonitor',
    percentual: false,
    formata: formatBytes,
  },
  {
    serie: 'uptime_seconds',
    titulo: 'Tempo ligado',
    icone: 'mdi-clock-outline',
    legenda: 'Desde o último reinício',
    percentual: false,
    formata: formataDuracao,
  },
]

/** Duração legível a partir de segundos. */
function formataDuracao(segundos: number): string {
  const dias = Math.floor(segundos / 86_400)
  const horas = Math.floor((segundos % 86_400) / 3_600)
  const minutos = Math.floor((segundos % 3_600) / 60)
  if (dias > 0) return `${dias}d ${horas}h`
  if (horas > 0) return `${horas}h ${minutos}min`
  return `${minutos}min`
}

function corDaFaixa(valor: number | null, def: DefinicaoSerie): string {
  if (valor === null) return 'grey'
  if (def.critico !== undefined && valor > def.critico) return 'error'
  if (def.atencao !== undefined && valor > def.atencao) return 'warning'
  return 'success'
}

function nomesDa(def: DefinicaoSerie): string[] {
  return [def.serie, ...(def.alternativos ?? [])]
}

/**
 * Um card por série **que existe**.
 *
 * Métrica que este sistema não consegue medir simplesmente não gera card — o
 * backend a declara indisponível com o motivo em vez de publicar zero, e um
 * card com `0 B` seria indistinguível de um servidor ocioso.
 */
const cards = computed(() =>
  DEFINICOES.flatMap((def) => {
    const nomes = nomesDa(def)
    // `metrics` chega do mais recente para o mais antigo.
    const amostras = props.metrics.filter((m) => nomes.includes(m.metricName))
    if (amostras.length === 0) return []

    const atual = amostras[0]
    const valor = Number(atual.metricValue)
    const disponivel = Number.isFinite(valor)
    const percentualMemoria =
      def.serie === 'memory_used_bytes'
        ? Number(props.metrics.find((m) => m.metricName === 'memory_usage')?.metricValue)
        : null
    const progresso =
      def.serie === 'memory_used_bytes' && Number.isFinite(percentualMemoria)
        ? percentualMemoria
        : disponivel && def.percentual
          ? valor
          : null
    const totalMemoria =
      def.serie === 'memory_used_bytes'
        ? Number(props.metrics.find((m) => m.metricName === 'memory_total_bytes')?.metricValue)
        : null
    const cor = corDaFaixa(progresso, def)
    const legenda =
      def.serie === 'memory_used_bytes'
        ? [
            Number.isFinite(totalMemoria) ? `de ${formatBytes(totalMemoria)}` : null,
            Number.isFinite(percentualMemoria)
              ? `${Number(percentualMemoria).toFixed(1)}% utilizado`
              : null,
          ]
            .filter(Boolean)
            .join(' · ') || def.legenda
        : def.legenda

    return [
      {
        serie: def.serie,
        nomes,
        formata: def.formata,
        tipoDeUnidade: def.tipoDeUnidade ?? 'generic',
        titulo: def.titulo,
        icone: def.icone,
        legenda,
        percentual: def.percentual,
        progresso,
        valor: disponivel ? valor : null,
        disponivel,
        cor,
        corHex: gaugeHexColor(
          progresso,
          def.serie === 'memory_used_bytes' ? 'memory_usage' : def.serie
        ),
        textoValor: !disponivel
          ? 'Sem dados'
          : def.formata
            ? def.formata(valor)
            : `${Math.round(valor * 10) / 10}%`,
        coletadoEm: `Coleta: ${atual.createdAt || 'N/A'}`,
        historico: amostras
          .slice(0, SPARKLINE_LIMIT)
          .reverse()
          .map((m) => ({ value: Number(m.metricValue) || 0, recordedAt: m.createdAt })),
      },
    ]
  })
)

/** O card cujo gráfico está aberto. */
type Card = (typeof cards)['value'][number]

const dialogAberto = ref(false)
const cardAberto = ref<Card | null>(null)

function abrir(card: Card): void {
  cardAberto.value = card
  dialogAberto.value = true
}
</script>

<style scoped>
.card-clicavel {
  cursor: pointer;
  transition: border-color 0.15s ease;
}

.card-clicavel:hover,
.card-clicavel:focus-visible {
  border-color: rgb(var(--v-theme-primary));
}
</style>

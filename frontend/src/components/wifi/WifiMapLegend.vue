<template>
  <section class="legend" aria-label="Legenda do mapa Wi-Fi">
    <div class="legend-item legend-quality">
      <div class="legend-title">Qualidade do enlace (cor da linha)</div>
      <div class="gradient" :style="{ background: gradient }" role="presentation"></div>
      <div class="d-flex justify-space-between text-caption font-weight-medium">
        <span>Crítico</span>
        <span>Atenção</span>
        <span>Excelente</span>
      </div>
      <div class="text-caption">Sinal, ruído (SNR) e retransmissões — vale o pior.</div>
    </div>

    <div class="legend-item">
      <div class="legend-title">Linha e aparelho</div>
      <ul class="legend-list">
        <li>
          <span class="swatch line thick" :style="{ background: palette.success }"></span>
          Mais grossa = taxa maior
        </li>
        <li>
          <span class="swatch line dotted" :style="{ borderColor: palette.info }"></span>
          Pontilhada = em repouso
        </li>
        <li>
          <span class="swatch ring" :style="{ outlineColor: palette.primary }"></span>
          Anel azul = aparelho vigiado
        </li>
        <li>
          <span class="swatch dot thick-border" :style="{ borderColor: palette.error }"></span>
          Borda grossa = precisa de atenção
        </li>
        <li>
          <span class="swatch dot faded"></span>
          Esmaecido = saiu do Wi-Fi
        </li>
        <li>
          <span class="swatch line dashed" :style="{ borderColor: palette.info }"></span>
          Tracejado andando = trocou de AP
        </li>
      </ul>
    </div>

    <div class="legend-item">
      <div class="legend-title">Distância (anéis tracejados)</div>
      <div class="text-body-2">
        Estimada pelo sinal, como faixa (“~3–6 m”) — paredes e o próprio aparelho mudam muito. Os
        anéis marcam 5, 10, 20 e 40 m; quem está além de 60 m fica na borda.
      </div>
      <div class="text-caption mt-1">
        Sem a potência do rádio, a estimativa usa o padrão da banda e o painel avisa.
      </div>
    </div>

    <div class="legend-item">
      <div class="legend-title">Ícones (pelo nome e fabricante)</div>
      <div class="d-flex flex-wrap ga-2">
        <span v-for="kind in KINDS" :key="kind" class="d-inline-flex align-center ga-1 text-body-2">
          <v-icon size="18">{{ DEVICE_KIND[kind].icon }}</v-icon>
          {{ DEVICE_KIND[kind].label }}
        </span>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useTopologyPalette } from '@/composables/topology/useTopologyPalette'
import { DEVICE_KIND, type WifiDeviceKind } from '@/utils/wifiMap/deviceKind'
import { linkGradient } from '@/utils/wifiMap/style'

const palette = useTopologyPalette()
const gradient = computed(() => linkGradient(palette.value))

const KINDS: WifiDeviceKind[] = ['phone', 'laptop', 'tablet', 'tv', 'camera', 'iot', 'generic']
</script>

<style scoped>
.legend {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 16px;
}

.legend-title {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 6px;
}

.gradient {
  height: 12px;
  border-radius: 6px;
  margin-bottom: 4px;
}

.legend-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 4px;
  font-size: 0.875rem;
}

.legend-list li {
  display: flex;
  align-items: center;
  gap: 8px;
}

.swatch {
  display: inline-block;
  flex-shrink: 0;
  width: 26px;
}

.swatch.line {
  height: 3px;
  border-radius: 2px;
}

.swatch.line.thick {
  height: 6px;
}

.swatch.line.dotted {
  height: 0;
  border-top: 3px dotted;
}

.swatch.line.dashed {
  height: 0;
  border-top: 4px dashed;
}

.swatch.dot,
.swatch.ring {
  width: 16px;
  height: 16px;
  margin: 0 5px;
  border-radius: 50%;
  border: 2px solid rgb(var(--v-theme-on-surface));
}

.swatch.ring {
  outline: 3px solid;
  outline-offset: 2px;
}

.swatch.dot.thick-border {
  border-width: 4px;
}

.swatch.dot.faded {
  opacity: 0.4;
  border-style: dashed;
}
</style>

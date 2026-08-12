<template>
  <div class="backdrop" aria-hidden="true">
    <svg class="backdrop__mesh" viewBox="0 0 1200 800" preserveAspectRatio="xMidYMid slice">
      <defs>
        <radialGradient id="glow-primary" cx="18%" cy="12%" r="62%">
          <stop offset="0%" stop-color="#1976D2" stop-opacity="0.55"></stop>
          <stop offset="100%" stop-color="#1976D2" stop-opacity="0"></stop>
        </radialGradient>
        <radialGradient id="glow-secondary" cx="86%" cy="88%" r="58%">
          <stop offset="0%" stop-color="#00ACC1" stop-opacity="0.42"></stop>
          <stop offset="100%" stop-color="#00ACC1" stop-opacity="0"></stop>
        </radialGradient>
        <pattern id="grid" width="48" height="48" patternUnits="userSpaceOnUse">
          <path d="M48 0H0V48" fill="none" stroke="#ffffff" stroke-opacity="0.035" />
        </pattern>
      </defs>

      <rect width="1200" height="800" fill="#0B0F16"></rect>
      <rect width="1200" height="800" fill="url(#grid)"></rect>
      <rect width="1200" height="800" fill="url(#glow-primary)"></rect>
      <rect width="1200" height="800" fill="url(#glow-secondary)"></rect>

      <g stroke="#7FD3E4" stroke-opacity="0.22" stroke-width="1.1" fill="none">
        <path v-for="(link, index) in links" :key="`l${index}`" :d="link"></path>
      </g>

      <g>
        <circle
          v-for="(node, index) in nodes"
          :key="`n${index}`"
          :cx="node.x"
          :cy="node.y"
          :r="node.r"
          :fill="node.core"
          class="backdrop__node"
          :style="{ animationDelay: `${node.delay}s` }"
        ></circle>
      </g>
    </svg>

    <div class="backdrop__vignette"></div>
  </div>
</template>

<script setup lang="ts">
/**
 * Fundo das telas de autenticação: um grafo de rede desenhado em SVG.
 *
 * SVG inline, e não uma imagem: `assets/hero.png` é o logotipo do scaffold do
 * Loco, com 360 px — esticado em tela cheia viraria um borrão. O desenho aqui
 * é vetorial (nítido em 4K), pesa alguns kB, dispensa requisição e usa as cores
 * do tema, então acompanha a paleta se ela mudar.
 *
 * As coordenadas são fixas de propósito: sorteá-las a cada carga faria a tela
 * de login parecer diferente a cada visita, e o fundo deixaria de ser
 * reconhecível como "a tela do NetMonitor".
 */

interface Node {
  x: number
  y: number
  r: number
  core: string
  delay: number
}

const PRIMARY = '#5AB0FF'
const SECONDARY = '#4DD9EC'

const nodes: Node[] = [
  { x: 150, y: 130, r: 5, core: PRIMARY, delay: 0 },
  { x: 330, y: 92, r: 3.5, core: SECONDARY, delay: 0.7 },
  { x: 262, y: 288, r: 7, core: PRIMARY, delay: 1.4 },
  { x: 96, y: 402, r: 3.5, core: SECONDARY, delay: 2.1 },
  { x: 428, y: 214, r: 4.5, core: SECONDARY, delay: 0.4 },
  { x: 214, y: 578, r: 5, core: PRIMARY, delay: 1.1 },
  { x: 412, y: 660, r: 3.5, core: SECONDARY, delay: 1.8 },
  { x: 596, y: 420, r: 8, core: PRIMARY, delay: 0.2 },
  { x: 742, y: 168, r: 4.5, core: SECONDARY, delay: 2.4 },
  { x: 880, y: 302, r: 5, core: PRIMARY, delay: 0.9 },
  { x: 1044, y: 138, r: 3.5, core: SECONDARY, delay: 1.6 },
  { x: 986, y: 546, r: 6, core: PRIMARY, delay: 0.5 },
  { x: 1108, y: 690, r: 3.5, core: SECONDARY, delay: 2.2 },
  { x: 760, y: 636, r: 4.5, core: SECONDARY, delay: 1.3 },
  { x: 640, y: 748, r: 3.5, core: PRIMARY, delay: 2.7 },
]

/** Arestas em curva suave — linha reta daria ar de diagrama, não de rede viva. */
const links: string[] = [
  'M150 130 Q 240 96 330 92',
  'M150 130 Q 190 210 262 288',
  'M330 92 Q 390 150 428 214',
  'M262 288 Q 340 244 428 214',
  'M262 288 Q 160 330 96 402',
  'M262 288 Q 216 440 214 578',
  'M214 578 Q 310 636 412 660',
  'M428 214 Q 528 300 596 420',
  'M596 420 Q 668 296 742 168',
  'M596 420 Q 748 372 880 302',
  'M596 420 Q 622 596 640 748',
  'M596 420 Q 690 540 760 636',
  'M742 168 Q 900 140 1044 138',
  'M880 302 Q 946 420 986 546',
  'M986 546 Q 1052 620 1108 690',
  'M760 636 Q 872 600 986 546',
  'M214 578 Q 396 500 596 420',
]
</script>

<style scoped>
.backdrop {
  position: absolute;
  inset: 0;
  overflow: hidden;
}

.backdrop__mesh {
  width: 100%;
  height: 100%;
  display: block;
}

/* Escurece as bordas para o card no centro ganhar contraste sem precisar de
   uma cor de fundo opaca, que mataria o efeito de vidro. */
.backdrop__vignette {
  position: absolute;
  inset: 0;
  background: radial-gradient(
    ellipse at center,
    rgba(11, 15, 22, 0) 38%,
    rgba(6, 9, 14, 0.86) 100%
  );
}

.backdrop__node {
  animation: pulse 5.5s ease-in-out infinite;
  transform-box: fill-box;
  transform-origin: center;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.45;
  }
  50% {
    opacity: 1;
  }
}

/* Fundo animado atrás de um formulário é decoração; para quem pediu menos
   movimento no sistema, ele fica parado. */
@media (prefers-reduced-motion: reduce) {
  .backdrop__node {
    animation: none;
    opacity: 0.8;
  }
}
</style>

<template>
  <v-app>
    <v-main class="auth-shell">
      <NetworkBackdrop />

      <v-container class="auth-shell__content pa-4 pa-sm-6" fluid>
        <v-row justify="center" class="ma-0 w-100">
          <v-col cols="12" :sm="10" :md="columns.md" :lg="columns.lg" class="pa-0">
            <v-card class="auth-card" :elevation="0" rounded="xl">
              <div class="auth-card__brand">
                <v-avatar class="auth-card__mark" size="44" rounded="lg">
                  <v-icon size="24" color="white">mdi-shield-network-outline</v-icon>
                </v-avatar>
                <div class="ms-3">
                  <div class="text-subtitle-1 font-weight-bold lh-tight">NetMonitor</div>
                  <div class="text-caption text-medium-emphasis">
                    Plataforma de Monitoramento de Redes
                  </div>
                </div>
              </div>

              <v-divider class="auth-card__rule"></v-divider>

              <div class="auth-card__body">
                <h1 class="auth-card__title text-h5 font-weight-bold mb-1">{{ title }}</h1>
                <p class="text-body-2 text-medium-emphasis mb-5">{{ subtitle }}</p>

                <slot></slot>
              </div>
            </v-card>

            <p class="auth-shell__footer text-caption text-center mt-4 mb-0">
              <v-icon size="13" class="me-1">mdi-lock-outline</v-icon>
              Acesso restrito aos operadores desta instalação
            </p>
          </v-col>
        </v-row>
      </v-container>
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import NetworkBackdrop from './NetworkBackdrop.vue'

/**
 * Moldura comum ao login e ao primeiro acesso.
 *
 * As duas telas são a mesma coisa vista em momentos diferentes da vida da
 * instalação. Mantê-las como um componente só garante que continuem parecendo
 * isso — quando eram dois arquivos independentes, cada ajuste de espaçamento
 * precisava ser feito duas vezes, e uma das duas sempre ficava para trás.
 *
 * O `wide` existe porque o cadastro tem cinco campos e o login tem dois: mesmo
 * card, larguras diferentes.
 */
const props = withDefaults(
  defineProps<{
    title: string
    subtitle: string
    wide?: boolean
  }>(),
  { wide: false }
)

const columns = computed(() => (props.wide ? { md: 7, lg: 5 } : { md: 6, lg: 4 }))
</script>

<style scoped>
.auth-shell {
  position: relative;
  min-height: 100vh;
  /* `100dvh` acompanha a barra do navegador no celular, onde `100vh` deixa o
     rodapé escondido atrás dela. */
  min-height: 100dvh;
  background: #0b0f16;
}

/* Centraliza verticalmente **sem** impedir a rolagem.
   `align-items: center` num flex mais alto que a viewport empurra o topo para
   fora do alcance da barra de rolagem: no cadastro, que tem cinco campos, o
   cabeçalho ficava inacessível numa tela de notebook. Com `margin: auto` no
   filho o excedente sobra dos dois lados e continua alcançável. */
.auth-shell__content {
  position: relative;
  z-index: 1;
  min-height: 100vh;
  min-height: 100dvh;
  display: flex;
}

/* `min-width: 0` é o que impede o vazamento lateral no celular: um item de flex
   nasce com `min-width: auto` e se recusa a encolher abaixo da largura mínima
   do próprio conteúdo — e o `<input>` tem uma largura intrínseca de ~20
   caracteres. Sem esta linha o card ficava mais largo que a tela e o lado
   direito do formulário saía do alcance. */
.auth-shell__content > * {
  margin-block: auto;
  width: 100%;
  min-width: 0;
}

/* O vidro: o fundo aparece através do card, sem apagar o texto. O
   `background` opaco no fallback importa — sem `backdrop-filter` (Firefox
   antigo), um card translúcido sobre o grafo fica ilegível. */
.auth-card {
  background: rgba(17, 22, 31, 0.92);
  border: 1px solid rgba(255, 255, 255, 0.09);
  box-shadow:
    0 24px 60px -12px rgba(0, 0, 0, 0.7),
    0 0 0 1px rgba(255, 255, 255, 0.02) inset;
}

@supports (backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px)) {
  .auth-card {
    background: rgba(17, 22, 31, 0.72);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    backdrop-filter: blur(20px) saturate(140%);
  }
}

.auth-card__brand {
  display: flex;
  align-items: center;
  padding: 16px 24px;
}

.auth-card__body {
  padding: 20px 24px 24px;
}

/* O Vuetify não zera a margem que o navegador dá ao `h1` (`0.67em`), e ela
   abria um vão logo abaixo da faixa da marca — o card parecia partido em dois.
   A entrelinha do `text-h5` também é de título de artigo, larga demais para um
   cabeçalho de uma linha. */
.auth-card__title {
  margin-top: 0;
  line-height: 1.3;
}

.auth-card__mark {
  background: linear-gradient(135deg, #1976d2 0%, #00acc1 100%);
  box-shadow: 0 6px 18px -6px rgba(25, 118, 210, 0.9);
}

.auth-card__rule {
  border-color: rgba(255, 255, 255, 0.08);
}

.auth-shell__footer {
  color: rgba(255, 255, 255, 0.42);
}

.lh-tight {
  line-height: 1.25;
}

@media (min-width: 600px) {
  .auth-card__brand {
    padding: 18px 32px;
  }

  .auth-card__body {
    padding: 22px 32px 30px;
  }
}
</style>

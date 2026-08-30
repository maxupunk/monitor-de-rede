<template>
  <div
    class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between mb-3 mb-md-6 ga-2 ga-md-4 px-1 px-md-0"
  >
    <div class="flex-grow-1">
      <h1 class="text-h5 text-md-h4 font-weight-bold text-break">{{ title }}</h1>
      <p
        v-if="subtitle"
        class="text-subtitle-2 text-md-subtitle-1 text-grey-darken-1 mt-1 text-break"
      >
        {{ subtitle }}
      </p>
    </div>
    <div class="d-flex flex-wrap align-center ga-2 w-100 w-md-auto justify-start justify-md-end">
      <v-defaults-provider :defaults="actionDefaults">
        <slot name="actions" />
      </v-defaults-provider>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string
  subtitle?: string
}>()

/**
 * No mobile as ações ocupam a largura toda e dividem o espaço em partes iguais;
 * a partir de `md` voltam à largura natural, alinhadas à direita do título.
 *
 * Vem por `v-defaults-provider` em vez de repetir as classes em cada `v-btn`
 * das dez páginas: aqui é onde o layout da área de ações é decidido, e uma
 * página nova herda o comportamento sem precisar lembrar dele.
 *
 * `flex-1-1-0` é `flex: 1 1 0` — base zero é o que garante partes iguais;
 * `flex-fill` (`1 1 auto`) dividiria proporcionalmente ao tamanho do rótulo.
 *
 * O objeto fica fora do template porque um literal ali seria recriado a cada
 * render, refazendo o provide sem necessidade. Vale notar que um `class`
 * próprio no `v-btn` da página substitui este — os defaults não são mesclados.
 */
const actionDefaults = {
  VBtn: { class: 'flex-1-1-0 flex-md-0-1' },
}
</script>

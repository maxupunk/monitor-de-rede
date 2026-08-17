<template>
  <v-btn
    v-bind="$attrs"
    icon
    color="primary"
    variant="tonal"
    :density="density"
    :disabled="disabled"
    @click="dialog = true"
  >
    <v-icon>mdi-cog-outline</v-icon>
    <v-tooltip activator="parent" location="top">Endereços deste servidor</v-tooltip>
  </v-btn>

  <ServerAddressesDialog v-model="dialog" @saved="emit('saved')" />
</template>

<script setup lang="ts">
import { ref } from 'vue'
import ServerAddressesDialog from '@/components/ServerAddressesDialog.vue'

/**
 * O atalho para o gerenciador de endereços, ao lado do campo que os oferece.
 *
 * Existe como componente porque três telas fazem a mesma coisa — a ativação
 * automática de log, o guia manual e a configuração da VPN — e porque quem
 * percebe no meio do preenchimento que falta um endereço precisa resolver **ali
 * mesmo**, sem sair da tela e sem perder o que já digitou. Antes cada uma tinha
 * o seu botão de texto, em posição e tamanho diferentes.
 *
 * O diálogo mora aqui dentro. O `v-dialog` do Vuetify se teletransporta para o
 * `body`, então estar dentro de outro diálogo não o aprisiona — o que a tela de
 * origem precisa saber é apenas que a lista mudou, e isso chega pelo `saved`.
 */
defineOptions({ inheritAttrs: false })

withDefaults(
  defineProps<{
    /** Acompanha a densidade do campo ao lado. */
    density?: 'default' | 'comfortable' | 'compact'
    disabled?: boolean
  }>(),
  { density: 'comfortable', disabled: false }
)

const emit = defineEmits<{ (e: 'saved'): void }>()

const dialog = ref(false)
</script>

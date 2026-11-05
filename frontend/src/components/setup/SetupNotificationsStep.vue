<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">Notificações</div>
      <div class="text-body-2">
        Saiba na hora quando um servidor ou container cair. Comece pelos alertas do navegador.
      </div>
    </div>

    <SetupPushCard class="mb-4" />

    <v-alert type="info" variant="tonal" density="comfortable" class="rounded-lg">
      E-mail, Telegram, webhook e os outros canais ficam em
      <strong>Configurações › Notificações</strong>. O botão para abrir essa tela aparece ao
      concluir o assistente.
    </v-alert>
  </div>
</template>

<script setup lang="ts">
import SetupPushCard from './SetupPushCard.vue'
</script>

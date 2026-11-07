<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4">
    <div class="d-flex flex-wrap align-center ga-3">
      <v-avatar color="success" variant="tonal" rounded="lg" size="44">
        <v-icon size="26">mdi-backup-restore</v-icon>
      </v-avatar>
      <div class="flex-grow-1">
        <div class="font-weight-bold">Backup e restauração</div>
        <div class="text-body-2 text-high-emphasis">
          Cópias automáticas do NetMonitor e dos seus bancos de dados, e a restauração a partir
          delas ou de um arquivo.
        </div>
      </div>
      <v-btn color="success" variant="flat" :to="{ name: 'backup' }">Abrir Backup</v-btn>
    </div>
  </v-card>
</template>

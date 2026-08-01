<template>
  <v-container fluid>
    <v-row>
      <v-col cols="12" sm="6" md="3">
        <v-card class="pa-4" elevation="2">
          <div class="text-caption text-grey">Total de Dispositivos</div>
          <div class="text-h4 font-weight-bold">0</div>
        </v-card>
      </v-col>
      <v-col cols="12" sm="6" md="3">
        <v-card class="pa-4" elevation="2">
          <div class="text-caption text-grey">Dispositivos Online</div>
          <div class="text-h4 font-weight-bold text-success">0</div>
        </v-card>
      </v-col>
      <v-col cols="12" sm="6" md="3">
        <v-card class="pa-4" elevation="2">
          <div class="text-caption text-grey">Dispositivos Offline</div>
          <div class="text-h4 font-weight-bold text-error">0</div>
        </v-card>
      </v-col>
      <v-col cols="12" sm="6" md="3">
        <v-card class="pa-4" elevation="2">
          <div class="text-caption text-grey">Alertas Ativos</div>
          <div class="text-h4 font-weight-bold text-warning">0</div>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>

<script setup lang="ts">
</script>

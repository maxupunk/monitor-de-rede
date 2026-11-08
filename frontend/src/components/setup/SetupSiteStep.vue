<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-3">
      <div>
        <div class="text-h6 font-weight-bold">Identificação do Site Local</div>
        <div class="text-caption text-high-emphasis">
          Define o local físico ou sede onde o servidor NetMonitor está operando.
        </div>
      </div>
      <v-switch
        v-model="model.enabled"
        color="primary"
        label="Criar este Site"
        density="compact"
        hide-details
      ></v-switch>
    </div>

    <v-expand-transition>
      <v-sheet v-if="model.enabled" border rounded="lg" class="pa-4 mb-4 bg-surface">
        <v-row dense>
          <v-col cols="12" sm="6">
            <v-text-field
              v-model="model.name"
              label="Nome do Site *"
              placeholder="Ex: Matriz - Servidor Central"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-domain"
              hint="Identifica o local nas telas e alertas"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>
          <v-col cols="12" sm="6">
            <v-text-field
              v-model="model.location"
              label="Localização Física"
              placeholder="Ex: São Paulo, SP - Sala de Servidores"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-map-marker-outline"
              hint="Cidade, endereço ou rack"
              persistent-hint
              class="mb-2"
            ></v-text-field>
          </v-col>
          <v-col cols="12">
            <v-text-field
              v-model="model.description"
              label="Descrição (opcional)"
              placeholder="Ex: Site principal onde o NetMonitor e o roteador de borda estão instalados"
              variant="outlined"
              density="comfortable"
              prepend-inner-icon="mdi-text"
              hide-details="auto"
            ></v-text-field>
          </v-col>
        </v-row>
      </v-sheet>
    </v-expand-transition>

    <v-card variant="tonal" color="info" class="pa-4 rounded-lg">
      <div class="d-flex align-start ga-3">
        <v-icon color="info" size="24">mdi-information-outline</v-icon>
        <div class="text-caption">
          <strong>Por que criar um site?</strong> Os sites agrupam redes e dispositivos por
          localidade física. A sub-rede que configuraremos na próxima etapa será vinculada
          automaticamente a este site.
        </div>
      </div>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import type { SiteForm } from './setupForm'

const model = defineModel<SiteForm>({ required: true })
</script>

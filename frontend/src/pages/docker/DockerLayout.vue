<template>
  <div>
    <v-card variant="outlined" rounded="xl" class="mb-4 overflow-hidden">
      <div class="d-flex align-center pr-3">
        <v-tabs color="primary" show-arrows class="flex-grow-1">
          <v-tab v-for="item in tabs" :key="item.to" :to="item.to" exact>
            <v-icon start>{{ item.icon }}</v-icon>
            {{ item.title }}
          </v-tab>
        </v-tabs>
        <v-chip
          v-if="docker.available"
          color="success"
          size="small"
          variant="tonal"
          prepend-icon="mdi-access-point"
          class="docker-live-chip"
        >
          Tempo real
        </v-chip>
      </div>
    </v-card>
    <DockerUnavailableAlert
      v-if="docker.status && !docker.available"
      :reason="docker.status.reason || 'A Docker Engine não respondeu ao backend.'"
    ></DockerUnavailableAlert>
    <router-view />
  </div>
</template>

<script setup lang="ts">
import DockerUnavailableAlert from '@/components/docker/DockerUnavailableAlert.vue'
import { useDockerStore } from '@/stores/docker'

const docker = useDockerStore()

const tabs = [
  { title: 'Visão geral', icon: 'mdi-view-dashboard-outline', to: '/docker' },
  { title: 'Containers', icon: 'mdi-cube-outline', to: '/docker/containers' },
  { title: 'Volumes', icon: 'mdi-database-outline', to: '/docker/volumes' },
  { title: 'Redes', icon: 'mdi-lan', to: '/docker/networks' },
  { title: 'Imagens', icon: 'mdi-layers-outline', to: '/docker/images' },
]
</script>

<style scoped>
.docker-live-chip {
  flex: 0 0 auto;
}

@media (max-width: 700px) {
  .docker-live-chip {
    display: none;
  }
}
</style>

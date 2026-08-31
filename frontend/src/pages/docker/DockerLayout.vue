<template>
  <div>
    <v-card variant="outlined" rounded="xl" class="mb-4 overflow-hidden">
      <v-tabs color="primary" show-arrows>
        <v-tab v-for="item in tabs" :key="item.to" :to="item.to" exact>
          <v-icon start>{{ item.icon }}</v-icon>
          {{ item.title }}
        </v-tab>
      </v-tabs>
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

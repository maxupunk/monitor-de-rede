<template>
  <div>
    <PageHeader
      title="Registry"
      subtitle="As imagens que os servidores compilam ficam aqui, na própria central — analisadas contra vulnerabilidades e guardadas para o rollback."
    >
      <template #actions>
        <v-btn color="primary" variant="tonal" prepend-icon="mdi-console" @click="helpOpen = true">
          Usar pelo Docker
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="registry.error" type="error" variant="tonal" class="mb-4">
      {{ registry.error }}
    </v-alert>

    <v-tabs v-model="tab" color="primary" class="mb-4">
      <v-tab value="images" prepend-icon="mdi-package-variant-closed">Imagens</v-tab>
      <v-tab value="tokens" prepend-icon="mdi-key-variant">Tokens de acesso</v-tab>
      <v-tab value="vulnerabilities" prepend-icon="mdi-shield-search">Vulnerabilidades</v-tab>
      <v-tab v-if="canAdmin" value="settings" prepend-icon="mdi-cog">Configurações</v-tab>
    </v-tabs>

    <v-window v-model="tab">
      <v-window-item value="images">
        <RegistryRepositories :can-admin="canAdmin" />
      </v-window-item>
      <v-window-item value="tokens">
        <RegistryTokens :host="access.host" />
      </v-window-item>
      <v-window-item value="vulnerabilities">
        <RegistryVulnerabilities :can-admin="canAdmin" />
      </v-window-item>
      <v-window-item v-if="canAdmin" value="settings">
        <RegistrySettingsCard />
      </v-window-item>
    </v-window>

    <RegistryDockerHelp v-model="helpOpen" :access="access" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import RegistryDockerHelp from '@/components/registry/RegistryDockerHelp.vue'
import RegistryRepositories from '@/components/registry/RegistryRepositories.vue'
import RegistrySettingsCard from '@/components/registry/RegistrySettingsCard.vue'
import RegistryTokens from '@/components/registry/RegistryTokens.vue'
import RegistryVulnerabilities from '@/components/registry/RegistryVulnerabilities.vue'
import { useAuthStore } from '@/stores/auth'
import { useRegistryStore } from '@/stores/registry'
import { registryAccess } from '@/utils/registryPresentation'

const registry = useRegistryStore()
const auth = useAuthStore()
const tab = ref('images')
const helpOpen = ref(false)

const canAdmin = computed(() => auth.roleIn('servers') === 'admin')
const access = registryAccess(window.location)

onMounted(() => {
  void registry.fetchOverview()
})
</script>

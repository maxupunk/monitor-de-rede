<template>
  <div>
    <v-alert v-if="tools.length === 0" type="info" variant="tonal">
      Este aplicativo não tem ferramentas avançadas.
    </v-alert>
    <v-row dense>
      <v-col v-for="tool in tools" :key="tool.id" cols="12" md="6" xl="4">
        <v-card border flat class="rounded-lg h-100 d-flex flex-column">
          <v-card-item>
            <template #prepend>
              <v-avatar color="primary" variant="tonal" rounded="lg">
                <v-icon>{{ tool.icon ?? 'mdi-tools' }}</v-icon>
              </v-avatar>
            </template>
            <v-card-title class="font-weight-bold text-wrap">{{ tool.title }}</v-card-title>
          </v-card-item>
          <v-card-text class="flex-grow-1 text-body-2">{{ tool.description }}</v-card-text>
          <v-card-actions class="px-4 pb-4">
            <v-btn
              color="primary"
              variant="flat"
              :prepend-icon="tool.icon ?? 'mdi-play'"
              :disabled="!canWrite || !hasMembers"
              @click="emit('run', tool.id)"
            >
              Abrir
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { FleetView } from '@/bindings/FleetView'
import { toolsOf } from '@/utils/fleetContext'

/**
 * A aba "Avançado": as ações que não são do dia a dia (mesh, otimizar canais…),
 * cada uma explicada. Sem `fleet.tools`, vão para cá as que nenhuma outra parte
 * da tela usa.
 */
const props = defineProps<{ view: FleetView; canWrite: boolean }>()
const emit = defineEmits<{ run: [fleetActionId: string] }>()

const hasMembers = computed(() => props.view.members.length > 0)
const tools = computed(() => toolsOf(props.view.plugin.fleet))
</script>

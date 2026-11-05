<template>
  <v-card border rounded="lg" class="bg-surface">
    <v-card-item>
      <template #prepend>
        <v-avatar color="primary" variant="tonal" rounded="lg">
          <v-icon>mdi-view-grid-plus-outline</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="font-weight-bold">Áreas do sistema</v-card-title>
      <v-card-subtitle class="text-wrap">
        Ligue só o que esta instalação usa. Desligar uma área tira do menu as telas e os cards dela
        — nada do que foi cadastrado é apagado.
      </v-card-subtitle>
    </v-card-item>

    <v-card-text>
      <v-alert v-if="!areasStore.snapshot" type="info" variant="tonal" density="compact">
        Aguardando o estado das áreas…
      </v-alert>
      <v-alert
        v-else-if="!authStore.isAdmin"
        type="info"
        variant="tonal"
        density="compact"
        class="mb-3"
      >
        Só administradores ligam ou desligam áreas.
      </v-alert>

      <v-alert
        v-if="error"
        type="error"
        variant="tonal"
        density="compact"
        closable
        class="mb-3"
        @click:close="error = null"
      >
        {{ error }}
      </v-alert>

      <AreaSwitchRow
        v-for="state in optionalAreas"
        :key="state.area"
        :state="state"
        :color="AREA_COLORS[state.area]"
        :disabled="!authStore.isAdmin || areasStore.saving !== null"
        :loading="areasStore.saving === state.area"
        @toggle="(on) => toggle(state, on)"
      />
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Area } from '@/bindings/Area'
import type { AreaState } from '@/bindings/AreaState'
import AreaSwitchRow from '@/components/areas/AreaSwitchRow.vue'
import { useConfirm } from '@/composables/useConfirm'
import { useAreasStore } from '@/stores/areas'
import { useAuthStore } from '@/stores/auth'

const AREA_COLORS: Record<Area, string> = {
  core: 'primary',
  network: 'primary',
  servers: 'info',
  vpn: 'success',
}

const areasStore = useAreasStore()
const authStore = useAuthStore()
const { confirm } = useConfirm()
const error = ref<string | null>(null)

/** O núcleo não desliga: só as áreas opcionais viram linha. */
const optionalAreas = computed(() => areasStore.states.filter((state) => state.optional))

function messageOf(reason: unknown, fallback: string): string {
  return reason instanceof Error && reason.message ? reason.message : fallback
}

/** Desligar mostra antes o que acontece; com bloqueio, não deixa confirmar. */
async function confirmTurnOff(state: AreaState): Promise<boolean> {
  const impact = await areasStore.fetchImpact(state.area)
  return confirm({
    title: `Desligar ${state.title}?`,
    message: impact.consequences.length
      ? 'Ao desligar a área:'
      : 'A área sai do menu e do Dashboard. Nada do que foi cadastrado é apagado.',
    details: impact.consequences.map((consequence) => consequence.message),
    blocker: impact.blocker?.message ?? null,
    confirmText: 'Desligar',
    confirmColor: 'warning',
    icon: state.icon,
    width: 540,
  })
}

async function toggle(state: AreaState, on: boolean) {
  error.value = null
  try {
    if (!on && !(await confirmTurnOff(state))) return
    await areasStore.setEnabled(state.area, on)
  } catch (reason: unknown) {
    error.value = messageOf(
      reason,
      on ? `Não foi possível ligar ${state.title}.` : `Não foi possível desligar ${state.title}.`
    )
  }
}
</script>

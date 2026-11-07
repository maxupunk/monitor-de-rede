<template>
  <div class="area-off d-flex justify-center">
    <v-card border rounded="lg" class="bg-surface pa-6 area-off__card">
      <div class="d-flex align-center ga-4 mb-4">
        <v-avatar :color="noAccess ? 'error' : 'warning'" variant="tonal" size="56" rounded="lg">
          <v-icon size="30">{{
            noAccess ? 'mdi-lock-outline' : (state?.icon ?? 'mdi-power-plug-off-outline')
          }}</v-icon>
        </v-avatar>
        <div>
          <div
            class="text-overline font-weight-bold"
            :class="noAccess ? 'text-error' : 'text-warning'"
          >
            {{ noAccess ? 'Sem acesso' : 'Área desligada' }}
          </div>
          <h1 class="text-h5 font-weight-bold">
            {{ noAccess ? 'Você não tem acesso a esta área' : 'Esta área está desligada' }}
          </h1>
        </div>
      </div>

      <p class="text-body-1 mb-3">
        <strong>{{ title }}</strong>
        <template v-if="state?.summary">: {{ state.summary }}</template>
      </p>
      <p v-if="contents.length > 0 && !noAccess" class="text-body-2 mb-4">
        Ela reúne {{ contents.join(', ') }}. Enquanto estiver desligada, essas telas saem do menu e
        os cards dela não aparecem no Dashboard — nada do que foi cadastrado é apagado.
      </p>
      <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-4">
        {{ error }}
      </v-alert>

      <template v-if="noAccess">
        <v-alert type="info" variant="tonal" density="compact" class="mb-4">
          Seu usuário não tem papel nesta área. Peça a um administrador para liberar o acesso em
          Administração › Usuários — assim que ele salvar, esta página abre sozinha.
        </v-alert>
        <v-btn color="primary" variant="tonal" to="/">Ir para o Dashboard</v-btn>
      </template>

      <div v-else-if="authStore.isAdmin" class="d-flex flex-wrap ga-2">
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-power"
          :loading="areasStore.saving === area"
          @click="turnOn"
        >
          Ligar área
        </v-btn>
        <v-btn color="primary" variant="tonal" to="/">Ir para o Dashboard</v-btn>
      </div>
      <template v-else>
        <v-alert type="info" variant="tonal" density="compact" class="mb-4">
          Só um administrador pode ligar a área. Peça a ele em Configurações › Geral › Áreas do
          sistema.
        </v-alert>
        <v-btn color="primary" variant="tonal" to="/">Ir para o Dashboard</v-btn>
      </template>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { Area } from '@/bindings/Area'
import { AREAS } from '@/areas'
import { useAreasStore } from '@/stores/areas'
import { useAuthStore } from '@/stores/auth'

const route = useRoute()
const router = useRouter()
const areasStore = useAreasStore()
const authStore = useAuthStore()
const error = ref<string | null>(null)

const areaModule = computed(() => AREAS.find((item) => item.id === route.params.area))
const area = computed<Area>(() => areaModule.value?.id ?? 'core')
const state = computed(() => areasStore.stateOf(area.value))
const title = computed(() => state.value?.title ?? area.value)
/** Chegou aqui por falta de papel na área (ADR 019), não por ela estar desligada. */
const noAccess = computed(() => route.query.motivo === 'acesso')

/** O que a área contém, a partir do próprio menu dela. */
const contents = computed(() =>
  (areaModule.value?.nav ?? []).flatMap((entry) =>
    typeof entry.item === 'function' ? [] : [entry.item.title]
  )
)

/** Volta ao endereço pedido; sem ele, ao Dashboard. */
function goBack() {
  const from = route.query.from
  void router.replace(typeof from === 'string' && from.startsWith('/') ? from : '/')
}

async function turnOn() {
  error.value = null
  try {
    await areasStore.setEnabled(area.value, true)
  } catch (reason: unknown) {
    error.value = reason instanceof Error ? reason.message : 'Não foi possível ligar a área.'
  }
}

// Ligada aqui ou em outra aba, ou o acesso liberado (os dois chegam pelo SSE):
// volta sozinho.
watch(
  () => areasStore.isVisible(area.value),
  (on) => {
    if (on) goBack()
  },
  { immediate: true }
)
</script>

<style scoped>
.area-off {
  padding-top: 8vh;
}
.area-off__card {
  max-width: 640px;
  width: 100%;
}
</style>

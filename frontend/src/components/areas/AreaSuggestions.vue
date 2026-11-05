<template>
  <div v-if="visible.length > 0" class="d-flex flex-column ga-2">
    <v-alert
      v-for="item in visible"
      :key="item.area + ':' + item.direction"
      :type="item.direction === 'show' ? 'info' : 'warning'"
      variant="tonal"
      density="compact"
      closable
      :icon="item.direction === 'show' ? 'mdi-eye-plus-outline' : 'mdi-eye-off-outline'"
      @click:close="dismiss(item)"
    >
      <div class="d-flex align-center flex-wrap ga-2">
        <span class="flex-grow-1">{{ item.message }}</span>
        <v-btn
          v-if="item.direction === 'show'"
          color="primary"
          variant="flat"
          size="small"
          :loading="areasStore.saving === item.area"
          @click="show(item)"
        >
          Mostrar
        </v-btn>
        <v-btn v-else color="warning" variant="flat" size="small" @click="askHide(item)">
          Ocultar
        </v-btn>
      </div>
      <div v-if="errors[item.area]" class="text-body-2 font-weight-bold mt-1">
        {{ errors[item.area] }}
      </div>
    </v-alert>

    <v-dialog v-model="confirm.open" max-width="520">
      <v-card rounded="xl">
        <v-card-title class="d-flex align-center ga-2">
          <v-icon color="warning">mdi-eye-off-outline</v-icon>
          Ocultar a área {{ confirm.title }}?
        </v-card-title>
        <v-card-text>
          <div v-if="confirm.loading" class="text-center pa-4">
            <v-progress-circular indeterminate color="primary"></v-progress-circular>
          </div>
          <template v-else-if="confirm.impact">
            <v-alert
              v-if="confirm.impact.blocker"
              type="error"
              variant="tonal"
              density="compact"
              class="mb-3"
            >
              {{ confirm.impact.blocker.message }}
            </v-alert>
            <ul v-if="confirm.impact.consequences.length > 0" class="text-body-2 pl-4">
              <li v-for="consequence in confirm.impact.consequences" :key="consequence.message">
                {{ consequence.message }}
              </li>
            </ul>
            <div v-else class="text-body-2">Os itens da área somem do menu e do painel.</div>
          </template>
          <v-alert v-if="confirm.error" type="error" variant="tonal" density="compact" class="mt-3">
            {{ confirm.error }}
          </v-alert>
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn color="primary" variant="text" @click="confirm.open = false">Cancelar</v-btn>
          <v-btn
            color="warning"
            variant="flat"
            :disabled="!confirm.impact || Boolean(confirm.impact.blocker)"
            :loading="areasStore.saving === confirm.area"
            @click="hide"
          >
            Ocultar área
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import type { Area } from '@/bindings/Area'
import type { AreaImpact } from '@/bindings/AreaImpact'
import { useAreasStore } from '@/stores/areas'
import { useAuthStore } from '@/stores/auth'
import { useOnboardingStore } from '@/stores/onboarding'
import {
  areaSuggestions,
  dismissSuggestion,
  isSuggestionDismissed,
  suggestionKey,
  type AreaSuggestion,
} from './areaSuggestions'

/**
 * Sugere mostrar uma área desligada que tem sinais de uso, ou ocultar uma
 * ligada sem nada em uso (ADR 017). Só para o administrador; cada aviso
 * aparece uma vez por usuário e direção, e nada muda sem o clique dele.
 */
const areasStore = useAreasStore()
const auth = useAuthStore()
const onboardingStore = useOnboardingStore()

/** Dispensados nesta sessão (o `localStorage` não é reativo). */
const dismissedNow = ref<ReadonlySet<string>>(new Set())
const errors = reactive<Partial<Record<Area, string>>>({})

const confirm = reactive({
  open: false,
  area: null as Area | null,
  title: '',
  loading: false,
  impact: null as AreaImpact | null,
  error: null as string | null,
  suggestion: null as AreaSuggestion | null,
})

const visible = computed<AreaSuggestion[]>(() => {
  const userId = auth.user?.id
  if (!auth.isAdmin || userId === undefined) return []
  const intentChosen = onboardingStore.status?.intentChosen === true
  return areaSuggestions(areasStore.states, intentChosen).filter(
    (item) =>
      !dismissedNow.value.has(suggestionKey(userId, item.area, item.direction)) &&
      !isSuggestionDismissed(userId, item.area, item.direction)
  )
})

function message(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback
}

function dismiss(item: AreaSuggestion) {
  const userId = auth.user?.id
  if (userId === undefined) return
  dismissSuggestion(userId, item.area, item.direction)
  dismissedNow.value = new Set([
    ...dismissedNow.value,
    suggestionKey(userId, item.area, item.direction),
  ])
}

async function show(item: AreaSuggestion) {
  delete errors[item.area]
  try {
    await areasStore.setEnabled(item.area, true)
    dismiss(item)
  } catch (err: unknown) {
    errors[item.area] = message(err, 'Não foi possível mostrar a área.')
  }
}

async function askHide(item: AreaSuggestion) {
  Object.assign(confirm, {
    open: true,
    area: item.area,
    title: item.title,
    loading: true,
    impact: null,
    error: null,
    suggestion: item,
  })
  try {
    confirm.impact = await areasStore.fetchImpact(item.area)
  } catch (err: unknown) {
    confirm.error = message(err, 'Não foi possível consultar o impacto.')
  } finally {
    confirm.loading = false
  }
}

async function hide() {
  const item = confirm.suggestion
  if (!item) return
  confirm.error = null
  try {
    await areasStore.setEnabled(item.area, false)
    dismiss(item)
    confirm.open = false
  } catch (err: unknown) {
    confirm.error = message(err, 'Não foi possível ocultar a área.')
  }
}
</script>

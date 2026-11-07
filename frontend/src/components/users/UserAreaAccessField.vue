<template>
  <div>
    <div class="d-flex align-center ga-2 mb-2">
      <v-icon color="primary">mdi-view-grid-outline</v-icon>
      <span class="text-subtitle-1 font-weight-bold">Acesso por área</span>
    </div>

    <v-alert v-if="role === 'admin'" type="info" variant="tonal" density="compact">
      O administrador acessa todas as áreas, com todas as permissões.
    </v-alert>

    <template v-else>
      <p class="text-body-2 mb-3">
        No Núcleo vale o perfil acima. Em cada área, escolha o que este usuário pode fazer — sem
        papel, a área não aparece para ele.
      </p>

      <div class="d-flex flex-wrap ga-2 mb-3">
        <v-btn
          size="small"
          color="info"
          variant="tonal"
          prepend-icon="mdi-eye-outline"
          :disabled="disabled"
          @click="setAll('viewer')"
        >
          Todas como visualizador
        </v-btn>
        <v-btn
          size="small"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-account-cog-outline"
          :disabled="disabled"
          @click="setAll('operator')"
        >
          Todas como operador
        </v-btn>
        <v-btn
          size="small"
          color="error"
          variant="tonal"
          prepend-icon="mdi-lock-outline"
          :disabled="disabled"
          @click="setAll(null)"
        >
          Nenhuma
        </v-btn>
      </div>

      <v-sheet border rounded="lg" class="bg-surface">
        <div
          v-for="(item, index) in rows"
          :key="item.area"
          class="d-flex flex-wrap align-center ga-3 pa-3"
          :class="{ 'border-t': index > 0 }"
        >
          <v-avatar color="primary" variant="tonal" rounded="lg" size="36">
            <v-icon>{{ item.icon }}</v-icon>
          </v-avatar>
          <span class="font-weight-medium flex-grow-1">{{ item.title }}</span>
          <v-chip-group
            :model-value="choiceOf(item.area)"
            mandatory
            :disabled="disabled"
            @update:model-value="(value: unknown) => setOne(item.area, value)"
          >
            <v-chip
              v-for="choice in CHOICES"
              :key="choice.value"
              :value="choice.value"
              :color="choice.color"
              :prepend-icon="choice.icon"
              :variant="choiceOf(item.area) === choice.value ? 'flat' : 'outlined'"
              size="small"
            >
              {{ choice.title }}
            </v-chip>
          </v-chip-group>
        </div>
      </v-sheet>

      <v-alert v-if="noArea" type="warning" variant="tonal" density="compact" class="mt-3">
        Sem nenhuma área, este usuário verá só o Núcleo (dashboard, dispositivos, monitores e
        alertas).
      </v-alert>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { Area } from '@/bindings/Area'
import { useAreasStore } from '@/stores/areas'
import type { AreaRoles, UserRole } from '@/utils/access'

type Choice = 'none' | 'viewer' | 'operator'

const props = defineProps<{
  modelValue: AreaRoles
  role: UserRole
  disabled?: boolean
}>()

const emit = defineEmits<{ 'update:modelValue': [value: AreaRoles] }>()

const CHOICES: { value: Choice; title: string; color: string; icon: string }[] = [
  { value: 'none', title: 'Sem acesso', color: 'error', icon: 'mdi-lock-outline' },
  { value: 'viewer', title: 'Visualizador', color: 'info', icon: 'mdi-eye-outline' },
  { value: 'operator', title: 'Operador', color: 'primary', icon: 'mdi-account-cog-outline' },
]

const OPTIONAL_AREAS: Area[] = ['network', 'servers', 'vpn']

const areasStore = useAreasStore()

/** Nome e ícone vêm do estado das áreas (o mesmo do card de Configurações). */
const rows = computed(() =>
  OPTIONAL_AREAS.map((area) => {
    const state = areasStore.stateOf(area)
    return { area, title: state?.title ?? area, icon: state?.icon ?? 'mdi-shape-outline' }
  })
)

const noArea = computed(() => OPTIONAL_AREAS.every((area) => !props.modelValue[area]))

function choiceOf(area: Area): Choice {
  const role = props.modelValue[area]
  return role === 'viewer' || role === 'operator' ? role : 'none'
}

function setOne(area: Area, value: unknown) {
  const next: AreaRoles = { ...props.modelValue }
  if (value === 'viewer' || value === 'operator') next[area] = value
  else delete next[area]
  emit('update:modelValue', next)
}

function setAll(role: 'viewer' | 'operator' | null) {
  const next: AreaRoles = {}
  if (role) for (const area of OPTIONAL_AREAS) next[area] = role
  emit('update:modelValue', next)
}
</script>

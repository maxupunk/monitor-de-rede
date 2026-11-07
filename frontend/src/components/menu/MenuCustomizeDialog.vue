<template>
  <v-dialog
    :model-value="modelValue"
    max-width="620"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card rounded="lg" class="bg-surface">
      <v-card-title class="d-flex align-center ga-3 pa-4">
        <v-avatar color="primary" variant="tonal" rounded="lg" size="40">
          <v-icon>mdi-menu-open</v-icon>
        </v-avatar>
        <div>
          <div class="text-h6 font-weight-bold">Personalizar menu</div>
          <div class="text-body-2 text-high-emphasis">
            Fixe no topo o que você mais usa e esconda o que não usa. Vale para você, em qualquer
            navegador.
          </div>
        </div>
      </v-card-title>

      <v-divider />

      <v-card-text class="pa-4">
        <div v-for="section in sections" :key="section.title" class="mb-4">
          <div class="text-overline font-weight-bold text-primary">{{ section.title }}</div>
          <v-sheet border rounded="lg" class="bg-surface">
            <div
              v-for="(leaf, index) in section.leaves"
              :key="leaf.to"
              class="d-flex align-center ga-2 px-3 py-1"
              :class="{ 'border-t': index > 0 }"
            >
              <v-icon :color="isHidden(leaf.to) ? 'error' : 'primary'">{{ leaf.icon }}</v-icon>
              <span
                class="flex-grow-1 font-weight-medium"
                :class="{ 'text-decoration-line-through': isHidden(leaf.to) }"
              >
                {{ leaf.title }}
              </span>
              <v-btn
                :icon="isPinned(leaf.to) ? 'mdi-pin' : 'mdi-pin-outline'"
                :color="isPinned(leaf.to) ? 'primary' : 'secondary'"
                :variant="isPinned(leaf.to) ? 'flat' : 'tonal'"
                size="small"
                :aria-label="isPinned(leaf.to) ? 'Desafixar' : 'Fixar no topo'"
                @click="togglePin(leaf.to)"
              />
              <v-switch
                :model-value="!isHidden(leaf.to)"
                color="success"
                density="compact"
                hide-details
                inset
                :disabled="isPinned(leaf.to)"
                :aria-label="isHidden(leaf.to) ? 'Mostrar no menu' : 'Esconder do menu'"
                @update:model-value="(on: boolean | null) => setShown(leaf.to, Boolean(on))"
              />
            </div>
          </v-sheet>
        </div>
      </v-card-text>

      <v-divider />

      <v-card-actions class="pa-4 flex-wrap ga-2">
        <v-btn
          color="warning"
          variant="tonal"
          prepend-icon="mdi-restore"
          :disabled="!changedFromDefault"
          @click="restore"
        >
          Restaurar padrão
        </v-btn>
        <v-spacer />
        <v-btn color="primary" variant="tonal" @click="emit('update:modelValue', false)">
          Cancelar
        </v-btn>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-content-save-outline"
          :loading="menuPrefs.saving"
          @click="save"
        >
          Salvar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { MenuPrefs } from '@/bindings/MenuPrefs'
import { navLeaves, type NavSection } from '@/navigation'
import { useMenuPrefsStore } from '@/stores/menuPrefs'

const props = defineProps<{
  modelValue: boolean
  /** O menu de áreas e papéis, antes do pessoal: tudo o que dá para ajustar. */
  baseSections: NavSection[]
}>()

const emit = defineEmits<{ 'update:modelValue': [value: boolean]; saved: [] }>()

const menuPrefs = useMenuPrefsStore()
const draft = ref<MenuPrefs>({ pinned: [], hidden: [] })

watch(
  () => props.modelValue,
  (open) => {
    if (open)
      draft.value = { pinned: [...menuPrefs.prefs.pinned], hidden: [...menuPrefs.prefs.hidden] }
  },
  { immediate: true }
)

const sections = computed(() => {
  const leaves = navLeaves(props.baseSections).filter(
    (leaf): leaf is typeof leaf & { to: string } => Boolean(leaf.to)
  )
  const titles = [...new Set(leaves.map((leaf) => leaf.section))]
  return titles.map((title) => ({
    title,
    leaves: leaves.filter((leaf) => leaf.section === title),
  }))
})

const changedFromDefault = computed(
  () => draft.value.pinned.length > 0 || draft.value.hidden.length > 0
)

function isPinned(path: string): boolean {
  return draft.value.pinned.includes(path)
}

function isHidden(path: string): boolean {
  return draft.value.hidden.includes(path)
}

/** Fixar também mostra: item fixado nunca fica escondido. */
function togglePin(path: string) {
  const { pinned, hidden } = draft.value
  draft.value = isPinned(path)
    ? { pinned: pinned.filter((item) => item !== path), hidden }
    : { pinned: [...pinned, path], hidden: hidden.filter((item) => item !== path) }
}

function setShown(path: string, shown: boolean) {
  const hidden = draft.value.hidden.filter((item) => item !== path)
  draft.value = { ...draft.value, hidden: shown ? hidden : [...hidden, path] }
}

function restore() {
  draft.value = { pinned: [], hidden: [] }
}

async function save() {
  await menuPrefs.save(draft.value)
  emit('saved')
  emit('update:modelValue', false)
}
</script>

<template>
  <div>
    <div class="d-flex align-start ga-2">
      <v-select
        :model-value="modelValue"
        :items="items"
        item-title="name"
        item-value="id"
        :label="label"
        variant="outlined"
        :clearable="clearable"
        :no-data-text="'Nenhum destino cadastrado ainda'"
        :hint="hint"
        persistent-hint
        class="flex-grow-1"
        @update:model-value="emit('update:modelValue', $event)"
      >
        <template #item="{ props: itemProps, item }">
          <v-list-item
            v-bind="itemProps"
            :prepend-icon="item.icon"
            :subtitle="item.target ?? undefined"
          ></v-list-item>
        </template>
      </v-select>
      <v-btn
        color="primary"
        variant="tonal"
        prepend-icon="mdi-plus"
        class="mt-2"
        @click="creating = true"
      >
        Novo
      </v-btn>
    </div>

    <StorageFormDialog v-model="creating" :storage-id="null" @saved="onCreated" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { StorageDestinationDetail } from '@/bindings/StorageDestinationDetail'
import StorageFormDialog from '@/components/storages/StorageFormDialog.vue'
import { useStoragesStore } from '@/stores/storages'
import { providerInfo } from '@/utils/storagePresentation'

/**
 * "Para onde vão as cópias": escolhe um destino ou cadastra um novo sem sair
 * do formulário — o destino recém-criado já volta selecionado.
 */
withDefaults(
  defineProps<{
    modelValue: number | null
    label?: string
    hint?: string
    clearable?: boolean
  }>(),
  {
    label: 'Destino das cópias',
    hint: 'Uma pasta no servidor, um NAS por SFTP ou um bucket na nuvem.',
    clearable: false,
  }
)

const emit = defineEmits<{ (e: 'update:modelValue', value: number | null): void }>()

const storagesStore = useStoragesStore()
const creating = ref(false)

const items = computed(() =>
  storagesStore.storages.map((storage) => ({
    id: storage.id,
    name: storage.name,
    target: storage.target,
    icon: providerInfo(storage.provider).icon,
  }))
)

onMounted(() => {
  if (!storagesStore.loaded) void storagesStore.fetchStorages()
})

function onCreated(saved: StorageDestinationDetail) {
  emit('update:modelValue', saved.id)
}
</script>

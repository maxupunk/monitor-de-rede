<template>
  <v-alert type="error" variant="tonal" density="compact" class="mb-2">
    <slot>
      Esta ação <strong>altera a configuração do equipamento</strong>. Confira os parâmetros e tenha
      um backup da configuração antes de continuar.
    </slot>
  </v-alert>
  <v-checkbox
    :model-value="modelValue"
    color="error"
    density="compact"
    hide-details
    :label="label"
    @update:model-value="(checked) => emit('update:modelValue', Boolean(checked))"
  ></v-checkbox>
</template>

<script setup lang="ts">
/** O aviso e o "estou ciente" de toda ação que altera equipamento. */
withDefaults(defineProps<{ modelValue: boolean; label?: string }>(), {
  label: 'Entendo que esta ação altera o equipamento',
})

const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
</script>

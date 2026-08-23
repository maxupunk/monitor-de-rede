<template>
  <v-dialog
    v-model="state.isOpen"
    :max-width="state.options.width || 480"
    :persistent="state.options.persistent"
    @update:model-value="onModelUpdate"
  >
    <v-card class="rounded-xl pa-2">
      <v-card-item class="pb-2">
        <template #prepend>
          <v-avatar :color="state.options.confirmColor || 'primary'" variant="tonal" rounded="lg">
            <v-icon size="24">{{ state.options.icon || 'mdi-help-circle-outline' }}</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold text-h6">
          {{ state.options.title || 'Confirmação' }}
        </v-card-title>
      </v-card-item>

      <v-card-text class="pt-2">
        <div v-if="state.options.message" class="text-body-1 text-medium-emphasis mb-3">
          {{ state.options.message }}
        </div>

        <v-form v-if="state.isPrompt" ref="formRef" @submit.prevent="submitPrompt">
          <v-text-field
            v-model="state.promptValue"
            :label="state.options.inputLabel || 'Valor'"
            :placeholder="state.options.placeholder"
            :type="state.options.inputType || 'text'"
            :rules="state.options.rules"
            autofocus
            variant="outlined"
            density="comfortable"
            hide-details="auto"
            class="mt-2"
          ></v-text-field>
        </v-form>
      </v-card-text>

      <v-card-actions class="justify-end px-4 pb-3 ga-2">
        <v-btn variant="text" @click="handleCancel">
          {{ state.options.cancelText || 'Cancelar' }}
        </v-btn>
        <v-btn
          :color="state.options.confirmColor || 'primary'"
          variant="flat"
          @click="state.isPrompt ? submitPrompt() : handleConfirm()"
        >
          {{ state.options.confirmText || 'Confirmar' }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useConfirm } from '@/composables/useConfirm'

const { state, handleConfirm, handleCancel } = useConfirm()
const formRef = ref<any>(null)

function onModelUpdate(value: boolean) {
  if (!value) {
    handleCancel()
  }
}

async function submitPrompt() {
  if (formRef.value) {
    const { valid } = await formRef.value.validate()
    if (!valid) return
  }
  handleConfirm(state.promptValue)
}
</script>

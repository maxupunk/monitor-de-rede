<template>
  <div class="d-inline-flex align-center" role="group" aria-label="Avaliar esta resposta">
    <v-btn
      icon
      size="x-small"
      variant="text"
      :color="rating?.value === 'up' ? 'success' : 'primary'"
      :aria-pressed="rating?.value === 'up'"
      :title="rating?.value === 'up' ? 'Desfazer a avaliação' : 'Boa resposta'"
      @click="aiStore.rate(messageId, 'up')"
    >
      <v-icon size="14">{{
        rating?.value === 'up' ? 'mdi-thumb-up' : 'mdi-thumb-up-outline'
      }}</v-icon>
    </v-btn>

    <v-menu v-model="open" location="top end" :close-on-content-click="false" max-width="320">
      <template #activator="{ props: menuProps }">
        <v-btn
          v-bind="menuProps"
          icon
          size="x-small"
          variant="text"
          :color="rating?.value === 'down' ? 'error' : 'primary'"
          :aria-pressed="rating?.value === 'down'"
          :title="
            rating?.value === 'down'
              ? `Avaliada como ruim: ${rating.reason ?? 'sem motivo'}`
              : 'Resposta ruim'
          "
        >
          <v-icon size="14">
            {{ rating?.value === 'down' ? 'mdi-thumb-down' : 'mdi-thumb-down-outline' }}
          </v-icon>
        </v-btn>
      </template>
      <v-card class="pa-3" elevation="8">
        <div class="text-body-medium font-weight-bold mb-2">O que deu errado?</div>
        <div class="d-flex flex-wrap ga-1 mb-2">
          <v-chip
            v-for="reason in AI_RATING_REASONS"
            :key="reason"
            size="small"
            color="error"
            :variant="rating?.reason === reason ? 'flat' : 'tonal'"
            @click="choose(reason)"
          >
            {{ reason }}
          </v-chip>
        </div>
        <v-text-field
          v-model="other"
          density="compact"
          variant="outlined"
          label="Outro motivo (opcional)"
          hide-details
          maxlength="200"
          @keydown.enter.prevent="choose(other)"
        />
        <div class="d-flex justify-end ga-2 mt-2">
          <v-btn
            v-if="rating?.value === 'down'"
            size="small"
            variant="text"
            color="primary"
            @click="undo"
          >
            Desfazer
          </v-btn>
          <v-btn size="small" variant="flat" color="error" @click="choose(other)">
            Avaliar como ruim
          </v-btn>
        </div>
      </v-card>
    </v-menu>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useAiStore } from '@/stores/ai'
import { AI_RATING_REASONS } from '@/utils/aiRating'

/**
 * 👍/👎 de uma resposta. O 👎 pede o motivo (ou aceita sem): é ele que diz,
 * na exportação, o que corrigir.
 */
const props = defineProps<{ messageId: string }>()

const aiStore = useAiStore()
const open = ref(false)
const other = ref('')

const rating = computed(
  () => aiStore.messages.find((message) => message.id === props.messageId)?.rating ?? null
)

function choose(reason: string) {
  const text = reason.trim() || null
  // Já está 👎: o mesmo motivo (ou nenhum) só fecha — desfazer é pelo botão.
  if (rating.value?.value === 'down' && (text === null || rating.value.reason === text)) {
    open.value = false
    return
  }
  aiStore.rate(props.messageId, 'down', text)
  other.value = ''
  open.value = false
}

function undo() {
  aiStore.rate(props.messageId, 'down')
  open.value = false
}
</script>

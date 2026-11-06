<template>
  <div>
    <PageHeader
      title="Configurações do Sistema"
      subtitle="Preferências, conectividade, notificações, inteligência artificial e sistema"
    />

    <v-card elevation="2" class="rounded-lg mb-4">
      <v-tabs v-model="tab" color="primary" show-arrows>
        <v-tab v-for="item in tabs" :key="item.value" :value="item.value" :prepend-icon="item.icon">
          {{ item.title }}
        </v-tab>
      </v-tabs>
    </v-card>

    <v-window v-model="tab" :touch="false">
      <v-window-item v-for="item in tabs" :key="item.value" :value="item.value">
        <v-row dense>
          <v-col
            v-for="(card, index) in item.cards"
            :key="index"
            cols="12"
            :md="card.cols?.md"
            :lg="card.cols?.lg"
          >
            <component :is="card.component" @saved="notify" />
          </v-col>
        </v-row>
      </v-window-item>
    </v-window>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4000">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { AREAS } from '@/areas'
import { buildSettingsTabs } from '@/areas/settings'
import { useAreasStore } from '@/stores/areas'
import PageHeader from '@/components/PageHeader.vue'

const areasStore = useAreasStore()

/** As abas vêm das áreas (ADR 017): área desligada leva as dela junto. */
const tabs = computed(() => buildSettingsTabs(AREAS, areasStore.isEnabled))

const route = useRoute()
const router = useRouter()
/** A aba escolhida fica na URL (`?tab=ia`) para links diretos. */
const tab = computed<string>({
  get: () =>
    tabs.value.find((item) => item.value === route.query.tab)?.value ?? tabs.value[0]?.value ?? '',
  set: (value) => void router.replace({ query: { ...route.query, tab: value } }),
})

const feedback = reactive({ visible: false, message: '', color: 'success' })

/** Todo cartão avisa pelo mesmo evento `saved`; a página só mostra o aviso. */
function notify(message: string, color = 'success'): void {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}
</script>

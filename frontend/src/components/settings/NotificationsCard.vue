<template>
  <v-card elevation="2" class="rounded-lg pa-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="warning">mdi-bell-ring-outline</v-icon>
      Notificações PWA & Web Push
    </v-card-title>
    <v-card-text class="mt-2">
      <p class="text-caption text-grey-darken-1 mb-3">
        Configure as notificações Web Push para receber alertas de queda e falhas críticas
        diretamente no seu dispositivo,
        <strong>mesmo com o navegador ou o aplicativo fechados</strong>.
      </p>

      <v-row dense class="mb-3">
        <v-col cols="12" sm="6">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Permissão do Navegador</div>
              <div class="text-caption text-grey">
                {{
                  permissionState === 'granted'
                    ? 'Permissão concedida'
                    : permissionState === 'denied'
                      ? 'Bloqueado no navegador'
                      : permissionState === 'unsupported'
                        ? 'Não suportado'
                        : 'Ainda não solicitado'
                }}
              </div>
            </div>
            <v-chip
              :color="
                permissionState === 'granted'
                  ? 'success'
                  : permissionState === 'denied'
                    ? 'error'
                    : 'warning'
              "
              size="small"
              variant="tonal"
              class="font-weight-bold"
            >
              {{ permissionState.toUpperCase() }}
            </v-chip>
          </div>
        </v-col>

        <v-col cols="12" sm="6">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Web Push em 2º Plano</div>
              <div class="text-caption text-grey">
                {{
                  !isWebPushSupported
                    ? 'Não suportado no navegador'
                    : isSubscribed
                      ? 'Dispositivo inscrito e ativo'
                      : 'Não inscrito neste dispositivo'
                }}
              </div>
            </div>
            <v-chip
              :color="!isWebPushSupported ? 'grey' : isSubscribed ? 'success' : 'warning'"
              size="small"
              variant="tonal"
              class="font-weight-bold"
            >
              {{ !isWebPushSupported ? 'INDISPONÍVEL' : isSubscribed ? 'ATIVO' : 'INATIVO' }}
            </v-chip>
          </div>
        </v-col>
      </v-row>

      <v-switch
        v-if="isWebPushSupported"
        :model-value="isSubscribed"
        label="Receber Alertas em Segundo Plano (Web Push - App Fechado)"
        color="primary"
        :loading="isSubscribing"
        :disabled="isSubscribing || permissionState === 'denied'"
        @update:model-value="(val) => handleToggleWebPush(Boolean(val))"
      />

      <v-switch
        :model-value="notificationsEnabled"
        label="Notificações Visuais na Interface Aberta"
        color="primary"
        :disabled="permissionState !== 'granted'"
        @update:model-value="(val) => setNotificationsEnabled(Boolean(val))"
      />

      <v-alert
        v-if="feedback.text"
        :type="feedback.type"
        variant="tonal"
        density="compact"
        class="mt-2 text-caption"
        closable
        @click:close="feedback.text = ''"
      >
        {{ feedback.text }}
      </v-alert>
    </v-card-text>

    <v-card-actions class="justify-space-between flex-wrap ga-2 pt-0">
      <v-btn
        v-if="permissionState !== 'granted'"
        color="primary"
        variant="flat"
        size="small"
        prepend-icon="mdi-bell-check"
        :disabled="permissionState === 'unsupported'"
        @click="requestPermission"
      >
        Solicitar Permissão
      </v-btn>
      <div v-else class="d-flex ga-2 flex-wrap">
        <v-btn
          color="primary"
          variant="flat"
          size="small"
          prepend-icon="mdi-send-check"
          :loading="testingPush"
          :disabled="!isSubscribed || testingPush"
          @click="handleSendTestPush"
        >
          Testar Web Push (Segundo Plano)
        </v-btn>
        <v-btn
          color="secondary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-bell-ring"
          @click="emit('test-notification')"
        >
          Teste Local (Aba Aberta)
        </v-btn>
      </div>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useNotifications } from '@/composables/useNotifications'

const emit = defineEmits<{
  'test-notification': []
}>()

const {
  permissionState,
  notificationsEnabled,
  isWebPushSupported,
  isSubscribed,
  isSubscribing,
  requestPermission,
  toggleWebPush,
  sendTestPush,
  setNotificationsEnabled,
} = useNotifications()

const testingPush = ref(false)
const feedback = reactive<{ text: string; type: 'success' | 'info' | 'warning' | 'error' }>({
  text: '',
  type: 'info',
})

async function handleToggleWebPush(enable: boolean) {
  feedback.text = ''
  const ok = await toggleWebPush(enable)
  if (ok) {
    feedback.type = 'success'
    feedback.text = enable
      ? 'Dispositivo inscrito com sucesso para notificações Web Push!'
      : 'Inscrição Web Push desativada neste dispositivo.'
  } else {
    feedback.type = 'error'
    feedback.text = 'Não foi possível alterar a inscrição de Web Push no navegador.'
  }
}

async function handleSendTestPush() {
  testingPush.value = true
  feedback.text = ''
  try {
    const res = await sendTestPush()
    if (res.success) {
      feedback.type = 'success'
      feedback.text = res.message || 'Notificação Web Push enviada com sucesso!'
    } else {
      feedback.type = 'warning'
      feedback.text = res.message || 'Nenhum dispositivo recebeu a notificação de teste.'
    }
  } catch (err) {
    feedback.type = 'error'
    feedback.text = err instanceof Error ? err.message : 'Erro ao disparar teste de Web Push.'
  } finally {
    testingPush.value = false
  }
}
</script>

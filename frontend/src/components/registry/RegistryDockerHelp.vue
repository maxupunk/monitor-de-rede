<template>
  <v-dialog
    :model-value="modelValue"
    max-width="680"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-item class="px-6 pt-5">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-console</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Usar o registry pelo Docker</v-card-title>
        <v-card-subtitle class="text-high-emphasis"
          >Pela VPN do NetMonitor ou pela rede interna</v-card-subtitle
        >
      </v-card-item>
      <v-card-text class="px-6">
        <v-alert
          v-if="!access.internal"
          type="warning"
          variant="tonal"
          density="compact"
          class="mb-4"
          data-test="public-origin"
        >
          Você abriu a central por um endereço público ({{ access.host }}). Por ele o registry
          recusa token pessoal: conecte-se pela VPN do NetMonitor (ou pela rede interna) e use o
          endereço de lá.
        </v-alert>
        <ol class="pl-4">
          <li v-if="!access.secure" class="mb-3">
            <div class="font-weight-bold">Libere o endereço no Docker da sua máquina</div>
            <div class="text-body-2 mb-1">
              Em <code>/etc/docker/daemon.json</code> (Linux) ou nas configurações do Docker
              Desktop, depois reinicie o Docker. O endereço é HTTP; pela VPN o tráfego já vai
              cifrado.
            </div>
            <CopyableCommand label="daemon.json" :command="daemonJson(access.host)" />
          </li>
          <li class="mb-3">
            <div class="font-weight-bold">Entre com um token de acesso</div>
            <div class="text-body-2 mb-1">
              Crie o token na aba Tokens de acesso e use-o como senha.
            </div>
            <CopyableCommand label="Terminal" :command="commands.login" />
          </li>
          <li>
            <div class="font-weight-bold">Envie e baixe</div>
            <CopyableCommand label="Terminal" :command="`${commands.push}\n${commands.pull}`" />
          </li>
        </ol>
      </v-card-text>
      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" @click="emit('update:modelValue', false)"
          >Fechar</v-btn
        >
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import CopyableCommand from '@/components/CopyableCommand.vue'
import { daemonJson, dockerCommands, type RegistryAccess } from '@/utils/registryPresentation'

const props = defineProps<{ modelValue: boolean; access: RegistryAccess }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()

const commands = computed(() => dockerCommands(props.access.host, 'equipe/minha-imagem', '1.0'))
</script>

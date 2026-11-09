<template>
  <div>
    <v-row dense>
      <v-col cols="12" sm="6">
        <v-text-field
          v-model="model.name"
          label="Nome do alvo *"
          placeholder="Ex.: Produção, Homologação, Cliente B"
          variant="outlined"
          :rules="[requiredRule('Dê um nome ao alvo')]"
        ></v-text-field>
      </v-col>
      <v-col cols="12" sm="6">
        <v-select
          v-model="model.probeId"
          :items="agentItems"
          item-title="title"
          item-value="value"
          item-props="props"
          label="Servidor (agente) *"
          variant="outlined"
          :disabled="lockAgent"
          :rules="[(value: number | null) => value !== null || 'Escolha o servidor']"
        ></v-select>
      </v-col>
      <v-col cols="12" sm="6">
        <v-text-field
          v-model="model.project"
          label="Projeto compose *"
          hint="Nome do projeto no servidor (o mesmo de docker compose -p)"
          persistent-hint
          variant="outlined"
          :disabled="lockProject"
          :rules="[requiredRule('Informe o projeto')]"
        ></v-text-field>
      </v-col>
      <v-col cols="12" sm="6">
        <v-text-field
          v-model="model.reference"
          label="Acompanha *"
          hint="Branch (main) ou tags (v*)"
          persistent-hint
          variant="outlined"
          prepend-inner-icon="mdi-source-branch"
          :rules="[requiredRule('Informe a branch ou o padrão de tag')]"
        ></v-text-field>
      </v-col>
    </v-row>

    <div class="text-subtitle-2 font-weight-bold mt-4 mb-1">Quando publicar</div>
    <v-switch
      v-model="model.requiresApproval"
      color="primary"
      inset
      hide-details
      label="Pedir aprovação antes de publicar (recomendado para produção)"
    ></v-switch>
    <div class="text-body-2 text-high-emphasis mb-2">
      {{
        model.requiresApproval
          ? 'Cada versão nova espera um administrador da área Servidores revisar os commits e publicar — pela tela ou pela notificação.'
          : 'Cada versão nova é publicada assim que o GitHub termina de compilar (bom para homologação).'
      }}
    </div>
    <v-row dense>
      <v-col cols="6" sm="3">
        <v-text-field
          v-model="model.windowStart"
          label="Janela: início"
          type="time"
          variant="outlined"
          clearable
        ></v-text-field>
      </v-col>
      <v-col cols="6" sm="3">
        <v-text-field
          v-model="model.windowEnd"
          label="Janela: fim"
          type="time"
          variant="outlined"
          clearable
        ></v-text-field>
      </v-col>
      <v-col cols="12" sm="6" class="d-flex align-center">
        <div class="text-body-2 text-high-emphasis">
          {{
            hasWindow
              ? `Fora de ${model.windowStart}–${model.windowEnd}, o deploy aprovado espera a janela abrir (o "Publicar agora" ignora a janela).`
              : 'Sem janela: o deploy roda assim que aprovado.'
          }}
        </div>
      </v-col>
    </v-row>

    <div class="text-subtitle-2 font-weight-bold mt-2 mb-1">Segurança</div>
    <v-select
      v-model="model.backupConnectionId"
      :items="backupItems"
      label="Backup do banco antes de atualizar"
      hint="As migrations da versão nova rodam na subida: com backup, dá para voltar o banco também"
      persistent-hint
      variant="outlined"
      clearable
      class="mb-2"
    ></v-select>
    <v-switch
      v-model="model.autoRollback"
      color="primary"
      inset
      hide-details
      label="Voltar a versão anterior se os healthchecks falharem"
    ></v-switch>

    <v-expansion-panels variant="accordion" class="mt-2">
      <v-expansion-panel elevation="0" class="border">
        <v-expansion-panel-title class="font-weight-bold">Avançado</v-expansion-panel-title>
        <v-expansion-panel-text>
          <v-row dense>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model.number="model.waitTimeoutSecs"
                type="number"
                label="Espera dos healthchecks (segundos)"
                variant="outlined"
                :min="10"
                :max="1800"
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model.number="model.keepImages"
                type="number"
                label="Versões antigas guardadas para o rollback"
                variant="outlined"
                :min="0"
                :max="20"
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6">
              <v-text-field
                v-model.number="model.position"
                type="number"
                label="Ordem (um de cada vez)"
                hint="Menor primeiro: homologação antes de produção"
                persistent-hint
                variant="outlined"
              ></v-text-field>
            </v-col>
            <v-col cols="12" sm="6" class="d-flex align-center">
              <v-switch
                v-model="model.enabled"
                color="primary"
                inset
                hide-details
                label="Alvo ativo"
              ></v-switch>
            </v-col>
          </v-row>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useDatabasesStore } from '@/stores/databases'
import { useDeploysStore } from '@/stores/deploys'
import type { TargetForm } from '@/utils/deployForms'
import { requiredRule } from '@/utils/formRules'

const model = defineModel<TargetForm>({ required: true })
defineProps<{ lockAgent?: boolean; lockProject?: boolean }>()

const deploys = useDeploysStore()
const databases = useDatabasesStore()
const auth = useAuthStore()

const hasWindow = computed(() => Boolean(model.value.windowStart && model.value.windowEnd))

const agentItems = computed(() =>
  deploys.agents.map((agent) => ({
    title: agent.name,
    value: agent.id,
    props: {
      subtitle: !agent.connected
        ? 'Desconectado agora'
        : agent.allowsDeploy
          ? 'Conectado · libera deploy'
          : 'Conectado · sem a permissão deploy',
      prependIcon: agent.connected ? 'mdi-lan-connect' : 'mdi-lan-disconnect',
    },
  }))
)

const backupItems = computed(() =>
  databases.connections.map((connection) => ({
    title: connection.storageDestinationName
      ? connection.name
      : `${connection.name} (sem armazenamento)`,
    value: connection.id,
    props: { disabled: !connection.storageDestinationName },
  }))
)

onMounted(() => {
  if (!deploys.agents.length) void deploys.fetchAgents()
  // As conexões de banco são só do administrador global (levam senha).
  if (auth.isAdmin && !databases.connections.length) void databases.fetchConnections()
})
</script>

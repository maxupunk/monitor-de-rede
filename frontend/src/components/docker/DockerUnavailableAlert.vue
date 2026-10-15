<template>
  <v-alert type="warning" variant="tonal" icon="mdi-docker" class="mb-4">
    <div class="font-weight-bold text-subtitle-1">Docker Engine indisponível</div>
    <div class="mt-1">{{ reason }}</div>
    <div class="mt-2 text-body-2">
      Quem acessa a Engine é o agente NetMonitor {{ placeLabel }} — a correção é feita lá, não no
      navegador.
    </div>

    <v-expansion-panels :model-value="0" variant="accordion" class="mt-4">
      <v-expansion-panel>
        <v-expansion-panel-title>
          <span class="font-weight-medium">Como corrigir o agente</span>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <ol class="docker-help-steps pl-5">
            <li>
              {{ folderLabel }}, abra o arquivo <code>.env</code> que fica ao lado de
              <code>docker-compose.yml</code> e confirme:
              <pre class="docker-help-code"><code>DOCKER_ENABLED=true</code></pre>
            </li>
            <li>
              Confirme que o serviço <code>netmonitor-agent</code> monta o socket da Engine:
              <pre class="docker-help-code"><code>services:
  netmonitor-agent:
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock</code></pre>
            </li>
            <li>
              Salve os arquivos e recrie o agente a partir dessa mesma pasta:
              <pre
                class="docker-help-code"
              ><code>docker compose up -d --force-recreate netmonitor-agent</code></pre>
            </li>
          </ol>

          <div class="font-weight-medium mt-4">Diagnóstico rápido</div>
          <div class="text-body-2 mt-1">
            O primeiro comando deve listar o socket; o segundo mostra mensagens de permissão ou
            conexão do agente.
          </div>
          <pre
            class="docker-help-code"
          ><code>docker compose exec netmonitor-agent ls -l /var/run/docker.sock
docker compose logs --tail=100 netmonitor-agent</code></pre>

          <v-alert v-if="local" type="info" variant="tonal" density="compact" class="mt-3">
            A API do NetMonitor não monta o socket: por segurança, só o agente local o acessa, sob a
            política de <code>LOCAL_AGENT_ALLOW</code>.
          </v-alert>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </v-alert>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  reason: string
  /** O host é o desta central (agente local) ou um servidor remoto. */
  local: boolean
}>()

const placeLabel = computed(() =>
  props.local ? 'que roda ao lado desta central' : 'instalado no servidor remoto'
)
const folderLabel = computed(() =>
  props.local
    ? 'Na pasta de instalação do NetMonitor'
    : 'No servidor remoto, na pasta onde o agente foi instalado'
)
</script>

<style scoped>
.docker-help-steps li + li {
  margin-top: 1rem;
}

.docker-help-code {
  overflow-x: auto;
  margin-top: 0.5rem;
  padding: 0.75rem;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  border-radius: 8px;
  background: rgba(var(--v-theme-on-surface), 0.04);
  font-size: 0.78rem;
  line-height: 1.5;
  white-space: pre;
}
</style>

<template>
  <v-alert type="warning" variant="tonal" icon="mdi-docker" class="mb-4">
    <div class="font-weight-bold text-subtitle-1">Docker Engine indisponível</div>
    <div class="mt-1">{{ reason }}</div>
    <div class="mt-2 text-body-2">
      A configuração é feita no servidor onde o NetMonitor está instalado — não no navegador.
    </div>

    <v-expansion-panels :model-value="0" variant="accordion" class="mt-4">
      <v-expansion-panel>
        <v-expansion-panel-title>
          <span class="font-weight-medium">Como corrigir em uma instalação Docker Compose</span>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <ol class="docker-help-steps pl-5">
            <li>
              Na pasta de instalação, abra o arquivo <code>.env</code> que fica ao lado de
              <code>docker-compose.yml</code> e adicione:
              <pre class="docker-help-code"><code>DOCKER_ENABLED=true</code></pre>
            </li>
            <li>
              Confirme que o serviço <code>netmonitor</code> possui a variável e a montagem do
              socket:
              <pre class="docker-help-code"><code>services:
  netmonitor:
    environment:
      DOCKER_ENABLED: ${DOCKER_ENABLED:-true}
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock</code></pre>
            </li>
            <li>
              Salve os arquivos e recrie o container a partir dessa mesma pasta:
              <pre
                class="docker-help-code"
              ><code>docker compose up -d --force-recreate netmonitor</code></pre>
            </li>
          </ol>

          <div class="font-weight-medium mt-4">Diagnóstico rápido</div>
          <div class="text-body-2 mt-1">
            O primeiro comando deve listar o socket; o segundo mostra mensagens de permissão ou
            conexão do backend.
          </div>
          <pre
            class="docker-help-code"
          ><code>docker compose exec netmonitor ls -l /var/run/docker.sock
docker compose logs --tail=100 netmonitor</code></pre>

          <v-alert type="info" variant="tonal" density="compact" class="mt-3">
            Se o backend roda diretamente no host, defina <code>DOCKER_ENABLED=true</code> no
            ambiente do processo e confirme que Docker Desktop ou Docker Engine está em execução.
            Nesse modo não existe montagem de socket no <code>docker-compose.yml</code>.
          </v-alert>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </v-alert>
</template>

<script setup lang="ts">
defineProps<{
  reason: string
}>()
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

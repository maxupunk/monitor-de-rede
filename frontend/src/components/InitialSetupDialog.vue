<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 860"
    :fullscreen="$vuetify.display.xs"
    persistent
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-xl initial-setup-card d-flex flex-column">
      <!-- Cabeçalho do Diálogo -->
      <v-card-item class="pa-5 pb-4 border-b bg-surface">
        <template #prepend>
          <v-avatar color="primary" size="48" rounded="lg" class="elevation-2">
            <v-icon size="28" color="white">mdi-rocket-launch-outline</v-icon>
          </v-avatar>
        </template>
        <div class="d-flex align-center ga-2 flex-wrap">
          <v-card-title class="font-weight-bold text-h6 pa-0">
            Assistente de Configuração Inicial
          </v-card-title>
          <v-chip size="x-small" color="primary" variant="tonal" class="font-weight-bold">
            Primeiro Acesso
          </v-chip>
        </div>
        <v-card-subtitle class="mt-1 text-wrap text-body-2">
          Configure os parâmetros essenciais do NetMonitor para iniciar o monitoramento do seu
          ambiente.
        </v-card-subtitle>
        <template #append>
          <v-btn
            v-if="currentStep < 8 && !applying"
            variant="text"
            size="small"
            color="grey"
            prepend-icon="mdi-close"
            class="text-none"
            @click="handleSkip"
          >
            Pular
          </v-btn>
        </template>
      </v-card-item>

      <!-- Indicador Visual de Etapas -->
      <div v-if="currentStep < 8" class="px-5 pt-3 pb-2 bg-grey-lighten-5 border-b">
        <div class="d-flex align-center justify-space-between mb-2">
          <span class="text-caption font-weight-bold text-primary">
            Etapa {{ currentStep }} de 7: {{ stepTitles[currentStep - 1] }}
          </span>
          <span class="text-caption text-medium-emphasis">
            {{ Math.round(((currentStep - 1) / 6) * 100) }}% concluído
          </span>
        </div>
        <v-progress-linear
          :model-value="((currentStep - 1) / 6) * 100"
          color="primary"
          height="6"
          rounded
        ></v-progress-linear>

        <!-- Barra de ícones das etapas -->
        <div class="d-none d-sm-flex justify-space-between align-center mt-3 px-1">
          <div
            v-for="(title, idx) in stepTitles"
            :key="idx"
            class="d-flex flex-column align-center cursor-pointer step-pill"
            :class="{
              'text-primary font-weight-bold': currentStep === idx + 1,
              'text-success': currentStep > idx + 1,
              'text-grey': currentStep < idx + 1,
            }"
            @click="goToStep(idx + 1)"
          >
            <v-avatar
              :color="
                currentStep === idx + 1
                  ? 'primary'
                  : currentStep > idx + 1
                    ? 'success'
                    : 'grey-lighten-2'
              "
              size="26"
              class="mb-1"
              :variant="currentStep === idx + 1 ? 'flat' : currentStep > idx + 1 ? 'tonal' : 'flat'"
            >
              <v-icon
                size="14"
                :color="
                  currentStep === idx + 1 ? 'white' : currentStep > idx + 1 ? 'success' : 'grey'
                "
              >
                {{ currentStep > idx + 1 ? 'mdi-check' : stepIcons[idx] }}
              </v-icon>
            </v-avatar>
            <span class="text-caption font-weight-medium" style="font-size: 0.68rem !important">
              {{ title }}
            </span>
          </div>
        </div>
      </div>

      <!-- Conteúdo das Etapas -->
      <v-card-text class="pa-5 flex-grow-1 overflow-y-auto">
        <!-- ======================================================== -->
        <!-- ETAPA 1: Boas-vindas & Visão Geral                       -->
        <!-- ======================================================== -->
        <div v-if="currentStep === 1" class="py-2">
          <v-sheet class="pa-6 rounded-xl border bg-surface text-center mb-5">
            <v-avatar color="primary" size="64" class="mb-3" variant="tonal">
              <v-icon size="36" color="primary">mdi-shield-check</v-icon>
            </v-avatar>
            <h2 class="text-h5 font-weight-bold mb-2">Bem-vindo ao NetMonitor!</h2>
            <p class="text-body-1 text-medium-emphasis max-w-600 mx-auto">
              Seu servidor foi instalado com sucesso. Este assistente irá guiá-lo na configuração
              dos locais, sub-redes, servidores DNS, endereços de acesso e preferências do sistema.
            </p>
          </v-sheet>

          <div class="text-subtitle-2 font-weight-bold mb-3 d-flex align-center">
            <v-icon start color="primary" size="18">mdi-layers-triple-outline</v-icon>
            O que você poderá configurar em instantes:
          </div>

          <v-row dense>
            <v-col cols="12" sm="6">
              <v-card variant="outlined" class="pa-3 rounded-lg h-100">
                <div class="d-flex align-center ga-3">
                  <v-avatar color="info" size="36" variant="tonal" rounded="lg">
                    <v-icon size="20">mdi-domain</v-icon>
                  </v-avatar>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">1. Local do Servidor (Site)</div>
                    <div class="text-caption text-medium-emphasis">
                      Identifica a matriz ou datacenter onde o servidor reside.
                    </div>
                  </div>
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6">
              <v-card variant="outlined" class="pa-3 rounded-lg h-100">
                <div class="d-flex align-center ga-3">
                  <v-avatar color="primary" size="36" variant="tonal" rounded="lg">
                    <v-icon size="20">mdi-server-network</v-icon>
                  </v-avatar>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">2. Endereços deste Servidor</div>
                    <div class="text-caption text-medium-emphasis">
                      Detecção automática de IPs locais, VPN e internet.
                    </div>
                  </div>
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6">
              <v-card variant="outlined" class="pa-3 rounded-lg h-100">
                <div class="d-flex align-center ga-3">
                  <v-avatar color="teal" size="36" variant="tonal" rounded="lg">
                    <v-icon size="20">mdi-lan</v-icon>
                  </v-avatar>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">3. Sub-rede & Descoberta</div>
                    <div class="text-caption text-medium-emphasis">
                      Cadastre a rede local para encontrar dispositivos automaticamente.
                    </div>
                  </div>
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6">
              <v-card variant="outlined" class="pa-3 rounded-lg h-100">
                <div class="d-flex align-center ga-3">
                  <v-avatar color="deep-purple" size="36" variant="tonal" rounded="lg">
                    <v-icon size="20">mdi-dns-outline</v-icon>
                  </v-avatar>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">4. Servidores DNS</div>
                    <div class="text-caption text-medium-emphasis">
                      Benchmark contínuo de latência (Google, Cloudflare, etc.).
                    </div>
                  </div>
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6">
              <v-card variant="outlined" class="pa-3 rounded-lg h-100">
                <div class="d-flex align-center ga-3">
                  <v-avatar color="deep-orange" size="36" variant="tonal" rounded="lg">
                    <v-icon size="20">mdi-shield-lock-outline</v-icon>
                  </v-avatar>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">5. WireGuard VPN (Opcional)</div>
                    <div class="text-caption text-medium-emphasis">
                      Conecte filiais e roteadores remotos com segurança.
                    </div>
                  </div>
                </div>
              </v-card>
            </v-col>

            <v-col cols="12" sm="6">
              <v-card variant="outlined" class="pa-3 rounded-lg h-100">
                <div class="d-flex align-center ga-3">
                  <v-avatar color="warning" size="36" variant="tonal" rounded="lg">
                    <v-icon size="20">mdi-bell-ring-outline</v-icon>
                  </v-avatar>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">6. Notificações & Padrões</div>
                    <div class="text-caption text-medium-emphasis">
                      Alertas PWA em tempo real e intervalos de coleta.
                    </div>
                  </div>
                </div>
              </v-card>
            </v-col>
          </v-row>

          <v-alert type="info" variant="tonal" density="comfortable" class="mt-4 rounded-lg">
            <div class="text-caption">
              <strong>Observação:</strong> Todos os itens possuem dados de exemplo pré-carregados.
              Você pode avançar rapidamente clicando em "Avançar" ou pular a qualquer momento.
            </div>
          </v-alert>
        </div>

        <!-- ======================================================== -->
        <!-- ETAPA 2: Local & Site do Servidor                        -->
        <!-- ======================================================== -->
        <div v-else-if="currentStep === 2" class="py-2">
          <div class="d-flex align-center justify-space-between mb-3">
            <div>
              <div class="text-h6 font-weight-bold">Identificação do Site Local</div>
              <div class="text-caption text-medium-emphasis">
                Define o local físico ou sede onde o servidor NetMonitor está operando.
              </div>
            </div>
            <v-switch
              v-model="form.site.enabled"
              color="primary"
              label="Criar este Site"
              density="compact"
              hide-details
            ></v-switch>
          </div>

          <v-expand-transition>
            <v-sheet v-if="form.site.enabled" border rounded="lg" class="pa-4 mb-4 bg-surface">
              <v-row dense>
                <v-col cols="12" sm="6">
                  <v-text-field
                    v-model="form.site.name"
                    label="Nome do Site *"
                    placeholder="Ex: Matriz - Servidor Central"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-domain"
                    hint="Identifica o local nas telas e alertas"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>
                <v-col cols="12" sm="6">
                  <v-text-field
                    v-model="form.site.location"
                    label="Localização Física"
                    placeholder="Ex: São Paulo, SP - Sala de Servidores"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-map-marker-outline"
                    hint="Cidade, endereço ou rack"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>
                <v-col cols="12">
                  <v-text-field
                    v-model="form.site.description"
                    label="Descrição (opcional)"
                    placeholder="Ex: Site principal onde o NetMonitor e o roteador de borda estão instalados"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-text"
                    hide-details="auto"
                  ></v-text-field>
                </v-col>
              </v-row>
            </v-sheet>
          </v-expand-transition>

          <v-card variant="tonal" color="info" class="pa-4 rounded-lg">
            <div class="d-flex align-start ga-3">
              <v-icon color="info" size="24">mdi-information-outline</v-icon>
              <div class="text-caption">
                <strong>Por que criar um site?</strong> Os sites agrupam redes e dispositivos por
                localidade física. A sub-rede que configuraremos na próxima etapa será vinculada
                automaticamente a este site.
              </div>
            </div>
          </v-card>
        </div>

        <!-- ======================================================== -->
        <!-- ETAPA 3: Endereços de Acesso deste Servidor              -->
        <!-- ======================================================== -->
        <div v-else-if="currentStep === 3" class="py-2">
          <div class="mb-4">
            <div class="text-h6 font-weight-bold">Endereços por onde este Servidor é Alcançado</div>
            <div class="text-caption text-medium-emphasis">
              Equipamentos na mesma rede usam a LAN; quem vem pela internet usa o IP público. O
              NetMonitor usa essa lista para sugerir configurações de Syslog e VPN.
            </div>
          </div>

          <v-row dense class="mb-3">
            <!-- Endereço LAN -->
            <v-col cols="12">
              <v-sheet border rounded="lg" class="pa-4 bg-surface">
                <div class="d-flex align-start ga-3">
                  <v-avatar color="primary" size="38" rounded="lg" variant="tonal">
                    <v-icon size="20">mdi-lan</v-icon>
                  </v-avatar>
                  <div class="flex-grow-1 min-width-0">
                    <div class="d-flex align-center ga-2">
                      <span class="font-weight-bold">Rede Local (LAN)</span>
                      <v-chip size="x-small" color="primary" variant="flat">
                        Recomendado Padrão
                      </v-chip>
                    </div>
                    <div class="text-caption text-medium-emphasis mb-2">
                      Usado por switches, roteadores e servidores na mesma rede física.
                    </div>
                    <v-text-field
                      v-model="form.addresses.lan"
                      placeholder="Ex: 192.168.1.50"
                      variant="outlined"
                      density="compact"
                      prepend-inner-icon="mdi-ip-network"
                      hint="IP da interface local do servidor"
                      persistent-hint
                      hide-details="auto"
                    ></v-text-field>
                  </div>
                </div>
              </v-sheet>
            </v-col>

            <!-- Endereço Internet / Público -->
            <v-col cols="12">
              <v-sheet border rounded="lg" class="pa-4 bg-surface">
                <div class="d-flex align-start ga-3">
                  <v-avatar color="teal" size="38" rounded="lg" variant="tonal">
                    <v-icon size="20">mdi-web</v-icon>
                  </v-avatar>
                  <div class="flex-grow-1 min-width-0">
                    <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                      <div class="d-flex align-center ga-2">
                        <span class="font-weight-bold">Internet / IP Público (DDNS)</span>
                      </div>
                      <v-btn
                        size="small"
                        color="teal"
                        variant="tonal"
                        prepend-icon="mdi-crosshairs-gps"
                        :loading="detectingPublicIp"
                        @click="handleDetectPublicIp"
                      >
                        Detectar IP Externo
                      </v-btn>
                    </div>
                    <div class="text-caption text-medium-emphasis mb-2">
                      Usado por filiais e dispositivos remotos que acessam o servidor via internet.
                    </div>
                    <v-text-field
                      v-model="form.addresses.public"
                      placeholder="Ex: 203.0.113.15 ou meudominio.ddns.net"
                      variant="outlined"
                      density="compact"
                      prepend-inner-icon="mdi-earth"
                      hint="IP público fixo ou hostname de DNS dinâmico"
                      persistent-hint
                      hide-details="auto"
                    ></v-text-field>
                  </div>
                </div>
              </v-sheet>
            </v-col>

            <!-- Endereço VPN -->
            <v-col cols="12">
              <v-sheet border rounded="lg" class="pa-4 bg-surface">
                <div class="d-flex align-start ga-3">
                  <v-avatar color="deep-purple" size="38" rounded="lg" variant="tonal">
                    <v-icon size="20">mdi-shield-lock-outline</v-icon>
                  </v-avatar>
                  <div class="flex-grow-1 min-width-0">
                    <span class="font-weight-bold">Túnel VPN (WireGuard)</span>
                    <div class="text-caption text-medium-emphasis mb-2">
                      Endereço interno do servidor dentro do túnel criptografado.
                    </div>
                    <v-text-field
                      v-model="form.addresses.vpn"
                      placeholder="Ex: 10.8.0.1"
                      variant="outlined"
                      density="compact"
                      prepend-inner-icon="mdi-shield-outline"
                      hint="IP do servidor na sub-rede WireGuard"
                      persistent-hint
                      hide-details="auto"
                    ></v-text-field>
                  </div>
                </div>
              </v-sheet>
            </v-col>
          </v-row>
        </div>

        <!-- ======================================================== -->
        <!-- ETAPA 4: Sub-rede & Descoberta Automática                -->
        <!-- ======================================================== -->
        <div v-else-if="currentStep === 4" class="py-2">
          <div class="d-flex align-center justify-space-between mb-3">
            <div>
              <div class="text-h6 font-weight-bold">Primeira Sub-rede (Network CIDR)</div>
              <div class="text-caption text-medium-emphasis">
                Cadastre a faixa de IP para que o NetMonitor descubra seus dispositivos.
              </div>
            </div>
            <v-switch
              v-model="form.network.enabled"
              color="primary"
              label="Cadastrar Sub-rede"
              density="compact"
              hide-details
            ></v-switch>
          </div>

          <v-expand-transition>
            <v-sheet v-if="form.network.enabled" border rounded="lg" class="pa-4 mb-4 bg-surface">
              <v-row dense>
                <v-col cols="12" sm="6">
                  <v-text-field
                    v-model="form.network.name"
                    label="Nome da Rede *"
                    placeholder="Ex: Rede Local Principal (LAN)"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-lan-connect"
                    hint="Identificador da sub-rede"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>

                <v-col cols="12" sm="6">
                  <v-text-field
                    v-model="form.network.cidr"
                    label="Faixa CIDR *"
                    placeholder="Ex: 192.168.1.0/24"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-network"
                    hint="Notação CIDR da rede (ex: 192.168.1.0/24)"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>

                <v-col cols="12" sm="6">
                  <v-text-field
                    v-model="form.network.gateway"
                    label="Gateway Padrão (Roteador)"
                    placeholder="Ex: 192.168.1.1"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-router-wireless"
                    hint="IP do roteador ou switch gateway"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>

                <v-col cols="12" sm="6">
                  <v-text-field
                    v-model.number="form.network.vlanId"
                    label="ID da VLAN (opcional)"
                    type="number"
                    placeholder="Ex: 10"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-tag-outline"
                    hint="Deixe em branco se não usar VLAN tagged"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>

                <v-col cols="12">
                  <v-divider class="my-2"></v-divider>
                  <div class="d-flex flex-column ga-2 mt-2">
                    <v-switch
                      v-model="form.network.scanEnabled"
                      color="primary"
                      label="Ativar varredura periódica de novos dispositivos nesta rede"
                      density="compact"
                      hide-details
                    ></v-switch>
                    <v-switch
                      v-model="form.network.scanNow"
                      color="secondary"
                      label="Disparar varredura imediata ao concluir o assistente (Recomendado)"
                      density="compact"
                      hide-details
                    ></v-switch>
                  </div>
                </v-col>
              </v-row>
            </v-sheet>
          </v-expand-transition>

          <v-alert type="info" variant="tonal" density="comfortable" class="rounded-lg">
            <div class="text-caption">
              A varredura testa cada IP da faixa enviando Ping e consultas SNMP para identificar
              fabricantes, nomes de host e portas ativas. Os equipamentos descobertos aparecerão em
              <strong>Descoberta</strong>.
            </div>
          </v-alert>
        </div>

        <!-- ======================================================== -->
        <!-- ETAPA 5: Servidores DNS para Monitoramento               -->
        <!-- ======================================================== -->
        <div v-else-if="currentStep === 5" class="py-2">
          <div class="mb-3">
            <div class="text-h6 font-weight-bold">
              Servidores DNS para Monitoramento & Benchmark
            </div>
            <div class="text-caption text-medium-emphasis">
              Selecione quais resolvedores DNS você deseja monitorar continuamente para comparar
              latência e qualidade de navegação.
            </div>
          </div>

          <div class="text-overline text-medium-emphasis mb-2">Provedores Públicos Populares</div>
          <v-row dense class="mb-4">
            <v-col v-for="dns in popularDnsPresets" :key="dns.address" cols="12" sm="6" md="4">
              <v-card
                border
                rounded="lg"
                class="pa-3 cursor-pointer h-100 d-flex flex-column justify-space-between"
                :class="{
                  'border-primary bg-primary-lighten-5': isDnsSelected(dns.address),
                }"
                @click="toggleDnsPreset(dns)"
              >
                <div class="d-flex align-start ga-2">
                  <v-checkbox-btn
                    :model-value="isDnsSelected(dns.address)"
                    color="primary"
                    density="compact"
                    class="mt-n1"
                  ></v-checkbox-btn>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">{{ dns.name }}</div>
                    <div class="text-body-2 font-weight-medium">{{ dns.address }}</div>
                    <div class="text-caption text-medium-emphasis">{{ dns.description }}</div>
                  </div>
                </div>
                <div class="d-flex justify-end mt-2">
                  <v-chip size="x-small" variant="tonal" color="grey">
                    {{ dns.protocol.toUpperCase() }}
                  </v-chip>
                </div>
              </v-card>
            </v-col>
          </v-row>

          <div class="text-overline text-medium-emphasis mb-2">
            Adicionar DNS Personalizado (Opcional)
          </div>
          <v-sheet border rounded="lg" class="pa-4 bg-surface">
            <v-row dense>
              <v-col cols="12" sm="5">
                <v-text-field
                  v-model="customDns.name"
                  label="Nome do Servidor"
                  placeholder="Ex: DNS Interno Matriz"
                  variant="outlined"
                  density="compact"
                  hide-details="auto"
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="5">
                <v-text-field
                  v-model="customDns.address"
                  label="Endereço IP ou URL DoH"
                  placeholder="Ex: 192.168.1.1 ou 10.0.0.2"
                  variant="outlined"
                  density="compact"
                  hide-details="auto"
                ></v-text-field>
              </v-col>
              <v-col cols="12" sm="2" class="d-flex align-center">
                <v-btn
                  color="primary"
                  variant="tonal"
                  block
                  prepend-icon="mdi-plus"
                  :disabled="!customDns.name || !customDns.address"
                  @click="addCustomDns"
                >
                  Adicionar
                </v-btn>
              </v-col>
            </v-row>
          </v-sheet>
        </div>

        <!-- ======================================================== -->
        <!-- ETAPA 6: Servidor WireGuard VPN (Opcional)               -->
        <!-- ======================================================== -->
        <div v-else-if="currentStep === 6" class="py-2">
          <div class="d-flex align-center justify-space-between mb-3">
            <div>
              <div class="text-h6 font-weight-bold">Servidor WireGuard VPN (Opcional)</div>
              <div class="text-caption text-medium-emphasis">
                Permite que roteadores (MikroTik, OpenWrt, Linux) criem túneis seguros para este
                servidor.
              </div>
            </div>
            <v-switch
              v-model="form.vpn.enabled"
              color="primary"
              label="Configurar WireGuard"
              density="compact"
              hide-details
            ></v-switch>
          </div>

          <v-expand-transition>
            <v-sheet v-if="form.vpn.enabled" border rounded="lg" class="pa-4 mb-4 bg-surface">
              <v-row dense>
                <v-col cols="12" md="6">
                  <v-text-field
                    v-model="form.vpn.publicEndpoint"
                    label="Endpoint Público (IP ou DDNS) *"
                    placeholder="Ex: 203.0.113.15 ou vpn.empresa.com"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-web"
                    hint="Endereço onde os roteadores remotos se conectarão"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>

                <v-col cols="12" sm="6" md="3">
                  <v-text-field
                    v-model.number="form.vpn.listenPort"
                    label="Porta UDP *"
                    type="number"
                    placeholder="51820"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-numeric"
                    hint="Porta padrão: 51820"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>

                <v-col cols="12" sm="6" md="3">
                  <v-text-field
                    v-model="form.vpn.cidr"
                    label="Sub-rede da VPN *"
                    placeholder="10.8.0.0/24"
                    variant="outlined"
                    density="comfortable"
                    prepend-inner-icon="mdi-shield-outline"
                    hint="Faixa interna privada"
                    persistent-hint
                    class="mb-2"
                  ></v-text-field>
                </v-col>

                <v-col cols="12" class="mt-2">
                  <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                    <v-btn
                      color="primary"
                      variant="tonal"
                      size="small"
                      prepend-icon="mdi-radar"
                      :loading="vpnTesting"
                      @click="handleTestVpnPreflight"
                    >
                      Testar Acessibilidade Externa (Pré-voo UDP)
                    </v-btn>
                  </div>

                  <v-alert
                    v-if="vpnPreflightResult"
                    :type="vpnPreflightResult.level"
                    variant="tonal"
                    density="compact"
                    class="mt-3"
                  >
                    <div class="font-weight-bold">{{ vpnPreflightResult.message }}</div>
                    <div class="text-caption">{{ vpnPreflightResult.recommendation }}</div>
                  </v-alert>
                </v-col>
              </v-row>
            </v-sheet>
          </v-expand-transition>

          <v-card variant="tonal" color="grey" class="pa-4 rounded-lg">
            <div class="d-flex align-start ga-3">
              <v-icon color="grey-darken-2" size="24">mdi-help-circle-outline</v-icon>
              <div class="text-caption text-grey-darken-3">
                Não precisa de VPN agora? Deixe desmarcado e ative quando desejar através do menu
                <strong>VPN WireGuard -> Servidor VPN</strong>.
              </div>
            </div>
          </v-card>
        </div>

        <!-- ======================================================== -->
        <!-- ETAPA 7: Notificações PWA & Preferências Gerais          -->
        <!-- ======================================================== -->
        <div v-else-if="currentStep === 7" class="py-2">
          <div class="mb-4">
            <div class="text-h6 font-weight-bold">Notificações & Parâmetros de Monitoramento</div>
            <div class="text-caption text-medium-emphasis">
              Configure os alertas do navegador e as preferências padrão de coleta do sistema.
            </div>
          </div>

          <!-- Card de Notificações PWA -->
          <v-sheet border rounded="lg" class="pa-4 mb-4 bg-surface">
            <div class="d-flex align-start justify-space-between flex-wrap ga-3 mb-3">
              <div class="d-flex align-center ga-3">
                <v-avatar color="warning" size="40" rounded="lg" variant="tonal">
                  <v-icon size="22">mdi-bell-ring-outline</v-icon>
                </v-avatar>
                <div>
                  <div class="font-weight-bold text-subtitle-1">Notificações PWA do Navegador</div>
                  <div class="text-caption text-medium-emphasis">
                    Receba alertas imediatos na área de trabalho quando dispositivos caírem.
                  </div>
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
                {{
                  permissionState === 'granted'
                    ? 'PERMITIDO'
                    : permissionState === 'denied'
                      ? 'BLOQUEADO'
                      : 'NÃO SOLICITADO'
                }}
              </v-chip>
            </div>

            <div class="d-flex align-center ga-2 flex-wrap">
              <v-btn
                v-if="permissionState !== 'granted'"
                color="primary"
                variant="flat"
                size="small"
                prepend-icon="mdi-bell-check"
                @click="requestPermission"
              >
                Ativar Notificações no Navegador
              </v-btn>
              <v-btn
                v-else
                color="success"
                variant="tonal"
                size="small"
                prepend-icon="mdi-bell-ring"
                @click="testNotification"
              >
                Enviar Notificação de Teste
              </v-btn>
            </div>
          </v-sheet>

          <!-- Parâmetros de Coleta -->
          <v-sheet border rounded="lg" class="pa-4 bg-surface">
            <div class="font-weight-bold text-subtitle-2 mb-3">Parâmetros Padrão de Coleta</div>
            <v-row dense>
              <v-col cols="12" sm="6">
                <v-text-field
                  v-model.number="form.preferences.defaultPingIntervalSeconds"
                  label="Intervalo de Ping Padrão"
                  type="number"
                  suffix="segundos"
                  variant="outlined"
                  density="comfortable"
                  hint="Frequência de teste para novos monitores (mínimo 10s)"
                  persistent-hint
                  class="mb-2"
                ></v-text-field>
              </v-col>

              <v-col cols="12" sm="6">
                <v-text-field
                  v-model="form.preferences.defaultSnmpCommunity"
                  label="Comunidade SNMP Padrão"
                  variant="outlined"
                  density="comfortable"
                  hint="Usada na descoberta automática (ex: public)"
                  persistent-hint
                  class="mb-2"
                ></v-text-field>
              </v-col>

              <v-col cols="12">
                <v-switch
                  v-model="form.preferences.autoDiscoveryEnabled"
                  color="primary"
                  label="Varredura automática global periódica das redes ativada"
                  density="compact"
                  hide-details
                ></v-switch>
              </v-col>
            </v-row>
          </v-sheet>
        </div>

        <!-- ======================================================== -->
        <!-- ETAPA 8: Revisão, Aplicação & Conclusão                 -->
        <!-- ======================================================== -->
        <div v-else-if="currentStep === 8" class="py-2">
          <template v-if="!appliedSuccess">
            <div class="text-center mb-4">
              <v-avatar color="primary" size="56" variant="tonal" class="mb-2">
                <v-icon size="32" color="primary">mdi-check-decagram-outline</v-icon>
              </v-avatar>
              <div class="text-h6 font-weight-bold">Revisão das Configurações</div>
              <div class="text-caption text-medium-emphasis">
                Confira o resumo das ações que serão aplicadas no servidor.
              </div>
            </div>

            <v-sheet border rounded="lg" class="pa-4 mb-4 bg-surface">
              <v-list density="compact" class="pa-0 bg-transparent">
                <!-- Site -->
                <v-list-item class="px-0">
                  <template #prepend>
                    <v-avatar color="info" size="32" variant="tonal" class="mr-3">
                      <v-icon size="18">mdi-domain</v-icon>
                    </v-avatar>
                  </template>
                  <v-list-item-title class="font-weight-bold">Local / Site</v-list-item-title>
                  <v-list-item-subtitle>
                    {{
                      form.site.enabled
                        ? `${form.site.name} (${form.site.location || 'Sem local'})`
                        : 'Não configurado'
                    }}
                  </v-list-item-subtitle>
                </v-list-item>
                <v-divider class="my-2"></v-divider>

                <!-- Endereços -->
                <v-list-item class="px-0">
                  <template #prepend>
                    <v-avatar color="primary" size="32" variant="tonal" class="mr-3">
                      <v-icon size="18">mdi-server-network</v-icon>
                    </v-avatar>
                  </template>
                  <v-list-item-title class="font-weight-bold">
                    Endereços do Servidor
                  </v-list-item-title>
                  <v-list-item-subtitle>
                    LAN: {{ form.addresses.lan || '—' }} | Internet:
                    {{ form.addresses.public || '—' }} | VPN: {{ form.addresses.vpn || '—' }}
                  </v-list-item-subtitle>
                </v-list-item>
                <v-divider class="my-2"></v-divider>

                <!-- Sub-rede -->
                <v-list-item class="px-0">
                  <template #prepend>
                    <v-avatar color="teal" size="32" variant="tonal" class="mr-3">
                      <v-icon size="18">mdi-lan</v-icon>
                    </v-avatar>
                  </template>
                  <v-list-item-title class="font-weight-bold">Sub-rede</v-list-item-title>
                  <v-list-item-subtitle>
                    {{
                      form.network.enabled
                        ? `${form.network.name} (${form.network.cidr}) — ${form.network.scanNow ? 'Varredura imediata ativa' : 'Sem varredura imediata'}`
                        : 'Não configurada'
                    }}
                  </v-list-item-subtitle>
                </v-list-item>
                <v-divider class="my-2"></v-divider>

                <!-- DNS -->
                <v-list-item class="px-0">
                  <template #prepend>
                    <v-avatar color="deep-purple" size="32" variant="tonal" class="mr-3">
                      <v-icon size="18">mdi-dns-outline</v-icon>
                    </v-avatar>
                  </template>
                  <v-list-item-title class="font-weight-bold">Servidores DNS</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ form.dns.length }} servidor(es) selecionado(s):
                    {{ form.dns.map((d) => d.name).join(', ') || 'Nenhum' }}
                  </v-list-item-subtitle>
                </v-list-item>
                <v-divider class="my-2"></v-divider>

                <!-- WireGuard -->
                <v-list-item class="px-0">
                  <template #prepend>
                    <v-avatar color="deep-orange" size="32" variant="tonal" class="mr-3">
                      <v-icon size="18">mdi-shield-lock-outline</v-icon>
                    </v-avatar>
                  </template>
                  <v-list-item-title class="font-weight-bold">WireGuard VPN</v-list-item-title>
                  <v-list-item-subtitle>
                    {{
                      form.vpn.enabled
                        ? `Porta ${form.vpn.listenPort} | CIDR ${form.vpn.cidr} | Endpoint ${form.vpn.publicEndpoint || 'Auto'}`
                        : 'Desativado'
                    }}
                  </v-list-item-subtitle>
                </v-list-item>
                <v-divider class="my-2"></v-divider>

                <!-- Geral -->
                <v-list-item class="px-0">
                  <template #prepend>
                    <v-avatar color="warning" size="32" variant="tonal" class="mr-3">
                      <v-icon size="18">mdi-cog-outline</v-icon>
                    </v-avatar>
                  </template>
                  <v-list-item-title class="font-weight-bold">
                    Preferências Globais
                  </v-list-item-title>
                  <v-list-item-subtitle>
                    Ping: {{ form.preferences.defaultPingIntervalSeconds }}s | SNMP:
                    {{ form.preferences.defaultSnmpCommunity }} | Descoberta:
                    {{ form.preferences.autoDiscoveryEnabled ? 'Ligada' : 'Desligada' }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-list>
            </v-sheet>

            <v-alert
              v-if="applyError"
              type="error"
              variant="tonal"
              density="compact"
              class="mb-3"
              closable
              @click:close="applyError = null"
            >
              {{ applyError }}
            </v-alert>

            <div v-if="applying" class="pa-4 text-center">
              <v-progress-circular indeterminate color="primary" class="mb-2"></v-progress-circular>
              <div class="text-caption font-weight-bold">{{ applyStatusMessage }}</div>
            </div>
          </template>

          <!-- Tela de Sucesso -->
          <template v-else>
            <div class="text-center py-6">
              <v-avatar color="success" size="72" class="mb-4 elevation-2">
                <v-icon size="42" color="white">mdi-check-bold</v-icon>
              </v-avatar>
              <h3 class="text-h5 font-weight-bold mb-2">Tudo pronto!</h3>
              <p class="text-body-1 text-medium-emphasis max-w-600 mx-auto mb-6">
                As configurações básicas foram salvas com sucesso no servidor. O NetMonitor já está
                ativo e operando.
              </p>

              <div class="d-flex justify-center flex-wrap ga-3">
                <v-btn
                  color="primary"
                  size="large"
                  variant="flat"
                  prepend-icon="mdi-view-dashboard"
                  class="font-weight-bold"
                  @click="finishAndNavigate('/')"
                >
                  Ir para o Dashboard
                </v-btn>
                <v-btn
                  v-if="form.network.enabled && form.network.scanNow"
                  color="secondary"
                  size="large"
                  variant="tonal"
                  prepend-icon="mdi-radar"
                  class="font-weight-bold"
                  @click="finishAndNavigate('/discovery')"
                >
                  Acompanhar Descoberta
                </v-btn>
                <v-btn
                  color="info"
                  size="large"
                  variant="outlined"
                  prepend-icon="mdi-plus"
                  class="font-weight-bold"
                  @click="finishAndNavigate('/devices')"
                >
                  Cadastrar Dispositivo
                </v-btn>
              </div>
            </div>
          </template>
        </div>
      </v-card-text>

      <!-- Ações do Rodapé -->
      <v-divider></v-divider>
      <v-card-actions class="pa-4 bg-surface justify-space-between align-center">
        <div>
          <v-btn
            v-if="currentStep > 1 && currentStep < 8 && !applying"
            variant="text"
            prepend-icon="mdi-arrow-left"
            @click="currentStep--"
          >
            Voltar
          </v-btn>
        </div>

        <div class="d-flex align-center ga-2">
          <v-btn
            v-if="currentStep < 7"
            color="primary"
            variant="flat"
            append-icon="mdi-arrow-right"
            class="font-weight-bold px-5"
            @click="currentStep++"
          >
            Avançar
          </v-btn>
          <v-btn
            v-else-if="currentStep === 7"
            color="primary"
            variant="flat"
            append-icon="mdi-check-decagram-outline"
            class="font-weight-bold px-5"
            @click="currentStep = 8"
          >
            Revisar Configurações
          </v-btn>
          <v-btn
            v-else-if="currentStep === 8 && !appliedSuccess"
            color="success"
            variant="flat"
            size="large"
            prepend-icon="mdi-content-save-check"
            class="font-weight-bold px-6"
            :loading="applying"
            @click="handleApplyAll"
          >
            Salvar e Iniciar
          </v-btn>
        </div>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useOnboardingStore } from '@/stores/onboarding'
import { useSitesStore } from '@/stores/sites'
import { useNetworksStore } from '@/stores/networks'
import { useDnsServersStore, type DnsServerPayload } from '@/stores/dnsServers'
import { useServerAddressesStore } from '@/stores/serverAddresses'
import { useVpnStore, type VpnPreflightResult } from '@/stores/vpn'
import { usePreferencesStore, defaultPreferences } from '@/stores/preferences'
import { useNotifications } from '@/composables/useNotifications'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'completed'): void
  (e: 'skipped'): void
}>()

const router = useRouter()
const onboardingStore = useOnboardingStore()
const sitesStore = useSitesStore()
const networksStore = useNetworksStore()
const dnsServersStore = useDnsServersStore()
const serverAddressesStore = useServerAddressesStore()
const vpnStore = useVpnStore()
const prefsStore = usePreferencesStore()

const { permissionState, requestPermission, sendNotification } = useNotifications()

const currentStep = ref(1)
const stepTitles = ['Início', 'Local / Site', 'Endereços', 'Sub-rede', 'DNS', 'VPN', 'Geral']
const stepIcons = [
  'mdi-hand-wave-outline',
  'mdi-domain',
  'mdi-server-network',
  'mdi-lan',
  'mdi-dns-outline',
  'mdi-shield-lock-outline',
  'mdi-bell-ring-outline',
]

// Estado dos formulários do assistente
const form = reactive({
  site: {
    enabled: true,
    name: 'Matriz - Servidor Central',
    location: 'São Paulo, SP - Sala de Servidores',
    description: 'Site principal onde o servidor NetMonitor está hospedado',
  },
  addresses: {
    lan: '',
    public: '',
    vpn: '10.8.0.1',
  },
  network: {
    enabled: true,
    name: 'Rede Local Principal (LAN)',
    cidr: '192.168.1.0/24',
    gateway: '192.168.1.1',
    vlanId: undefined as number | undefined,
    scanEnabled: true,
    scanNow: true,
  },
  dns: [] as DnsServerPayload[],
  vpn: {
    enabled: false,
    publicEndpoint: '',
    listenPort: 51820,
    cidr: '10.8.0.0/24',
  },
  preferences: defaultPreferences(),
})

// Presets de servidores DNS populares
const popularDnsPresets: DnsServerPayload[] = [
  {
    name: 'Cloudflare DNS',
    address: '1.1.1.1',
    protocol: 'udp',
    isDefault: true,
    description: 'Ultra rápido e privado (1.1.1.1)',
  },
  {
    name: 'Google Public DNS',
    address: '8.8.8.8',
    protocol: 'udp',
    isDefault: true,
    description: 'Alta confiabilidade global (8.8.8.8)',
  },
  {
    name: 'Quad9 Security',
    address: '9.9.9.9',
    protocol: 'udp',
    isDefault: false,
    description: 'Bloqueio integrado de malware (9.9.9.9)',
  },
  {
    name: 'OpenDNS Umbrella',
    address: '208.67.222.222',
    protocol: 'udp',
    isDefault: false,
    description: 'Cisco OpenDNS (208.67.222.222)',
  },
  {
    name: 'Cloudflare DoH',
    address: 'https://cloudflare-dns.com/dns-query',
    protocol: 'doh',
    isDefault: false,
    description: 'DNS sobre HTTPS criptografado',
  },
]

const customDns = reactive({
  name: '',
  address: '',
})

const detectingPublicIp = ref(false)
const vpnTesting = ref(false)
const vpnPreflightResult = ref<VpnPreflightResult | null>(null)
const applying = ref(false)
const applyStatusMessage = ref('')
const applyError = ref<string | null>(null)
const appliedSuccess = ref(false)

function isDnsSelected(address: string): boolean {
  return form.dns.some((d) => d.address === address)
}

function toggleDnsPreset(preset: DnsServerPayload) {
  const index = form.dns.findIndex((d) => d.address === preset.address)
  if (index >= 0) {
    form.dns.splice(index, 1)
  } else {
    form.dns.push({ ...preset })
  }
}

function addCustomDns() {
  if (!customDns.name.trim() || !customDns.address.trim()) return
  const isDoh = customDns.address.startsWith('https://')
  form.dns.push({
    name: customDns.name.trim(),
    address: customDns.address.trim(),
    protocol: isDoh ? 'doh' : 'udp',
    isDefault: true,
    description: 'DNS personalizado adicionado no assistente',
  })
  customDns.name = ''
  customDns.address = ''
}

function goToStep(stepNumber: number) {
  if (stepNumber <= currentStep.value || stepNumber === currentStep.value + 1) {
    currentStep.value = stepNumber
  }
}

/** Preenche valores sugeridos inteligentes a partir dos dados detectados */
async function initializeDefaults() {
  // Preseleciona Cloudflare e Google por padrão
  if (form.dns.length === 0) {
    form.dns = [popularDnsPresets[0], popularDnsPresets[1]]
  }

  // Carrega status de onboarding e endereços do servidor
  const st = await onboardingStore.fetchStatus()
  await serverAddressesStore.fetchAll()

  if (st?.detectedLanIp) {
    form.addresses.lan = st.detectedLanIp
    // Sugere faixa CIDR baseada no IP LAN
    const parts = st.detectedLanIp.split('.')
    if (parts.length === 4) {
      form.network.cidr = `${parts[0]}.${parts[1]}.${parts[2]}.0/24`
      form.network.gateway = `${parts[0]}.${parts[1]}.${parts[2]}.1`
    }
  } else {
    const lanEntry = serverAddressesStore.entries.find((e) => e.kind === 'lan')
    if (lanEntry?.value || lanEntry?.detected) {
      const ip = (lanEntry.value || lanEntry.detected)!
      form.addresses.lan = ip
      const parts = ip.split('.')
      if (parts.length === 4) {
        form.network.cidr = `${parts[0]}.${parts[1]}.${parts[2]}.0/24`
        form.network.gateway = `${parts[0]}.${parts[1]}.${parts[2]}.1`
      }
    }
  }

  if (st?.detectedPublicIp) {
    form.addresses.public = st.detectedPublicIp
    form.vpn.publicEndpoint = st.detectedPublicIp
  }

  const vpnEntry = serverAddressesStore.entries.find((e) => e.kind === 'vpn')
  if (vpnEntry?.value || vpnEntry?.detected) {
    form.addresses.vpn = (vpnEntry.value || vpnEntry.detected)!
  }

  // Carrega preferências atuais
  await prefsStore.fetchAll()
  form.preferences = { ...prefsStore.preferences }
}

async function handleDetectPublicIp() {
  detectingPublicIp.value = true
  try {
    const ip = await vpnStore.detectEndpoint()
    if (ip) {
      form.addresses.public = ip
      form.vpn.publicEndpoint = ip
    }
  } finally {
    detectingPublicIp.value = false
  }
}

async function handleTestVpnPreflight() {
  vpnTesting.value = true
  try {
    vpnPreflightResult.value = await vpnStore.runPreflight()
  } finally {
    vpnTesting.value = false
  }
}

function testNotification() {
  sendNotification('NetMonitor - Notificação de Teste', {
    body: 'As notificações PWA em tempo real estão configuradas e operacionais!',
  })
}

function handleSkip() {
  onboardingStore.dismissWizard(true)
  emit('update:modelValue', false)
  emit('skipped')
}

/** Executa a criação e salvamento em lote de todos os parâmetros configurados */
async function handleApplyAll() {
  applying.value = true
  applyError.value = null

  try {
    let createdSiteId: number | null = null

    // 1. Criar Site se habilitado
    if (form.site.enabled && form.site.name.trim()) {
      applyStatusMessage.value = 'Criando Site de identificação...'
      await sitesStore.fetchSites()
      const existing = sitesStore.sites.find(
        (s) => s.name.trim().toLowerCase() === form.site.name.trim().toLowerCase()
      )
      if (existing) {
        createdSiteId = existing.id
      } else {
        const ok = await sitesStore.createSite({
          name: form.site.name.trim(),
          location: form.site.location.trim() || undefined,
          description: form.site.description.trim() || undefined,
        })
        if (ok) {
          await sitesStore.fetchSites()
          const created = sitesStore.sites.find((s) => s.name === form.site.name.trim())
          if (created) createdSiteId = created.id
        }
      }
    }

    // 2. Salvar Endereços do Servidor
    applyStatusMessage.value = 'Configurando endereços deste servidor...'
    const overrides: Record<string, string> = {}
    if (form.addresses.lan.trim()) overrides.lan = form.addresses.lan.trim()
    if (form.addresses.public.trim()) overrides.public = form.addresses.public.trim()
    if (form.addresses.vpn.trim()) overrides.vpn = form.addresses.vpn.trim()

    await serverAddressesStore.save({
      overrides,
      custom: [],
      preferredId: 'lan',
    })

    // 3. Criar Sub-rede se habilitada
    if (form.network.enabled && form.network.name.trim() && form.network.cidr.trim()) {
      applyStatusMessage.value = 'Cadastrando sub-rede local...'
      await networksStore.fetchNetworks()
      const existingNet = networksStore.networks.find(
        (n) => n.cidr.trim() === form.network.cidr.trim()
      )

      let targetNetId = existingNet?.id ?? null
      if (!existingNet) {
        const createdNet = await networksStore.createNetwork({
          name: form.network.name.trim(),
          cidr: form.network.cidr.trim(),
          gateway: form.network.gateway.trim() || undefined,
          vlanId: form.network.vlanId || undefined,
          siteId: createdSiteId,
          scanEnabled: form.network.scanEnabled,
        })
        if (createdNet) {
          await networksStore.fetchNetworks()
          const newNet = networksStore.networks.find(
            (n) => n.cidr.trim() === form.network.cidr.trim()
          )
          if (newNet) targetNetId = newNet.id
        }
      }

      // Disparar varredura imediata se solicitado
      if (form.network.scanNow && targetNetId) {
        applyStatusMessage.value = 'Iniciando varredura da rede...'
        void networksStore.scanNetwork(targetNetId)
      }
    }

    // 4. Cadastrar Servidores DNS
    if (form.dns.length > 0) {
      applyStatusMessage.value = 'Cadastrando servidores DNS...'
      await dnsServersStore.fetchServers(true)
      for (const dns of form.dns) {
        const exists = dnsServersStore.findByAddress(dns.address, dns.protocol)
        if (!exists) {
          await dnsServersStore.createServer({ ...dns })
        }
      }
    }

    // 5. Configurar WireGuard VPN se habilitado
    if (form.vpn.enabled) {
      applyStatusMessage.value = 'Salvando parâmetros do servidor WireGuard...'
      await vpnStore.saveServer({
        publicEndpoint: form.vpn.publicEndpoint.trim() || null,
        listenPort: form.vpn.listenPort || 51820,
        cidr: form.vpn.cidr.trim() || '10.8.0.0/24',
      })
    }

    // 6. Salvar Preferências Gerais
    applyStatusMessage.value = 'Gravando preferências globais...'
    await prefsStore.save({ ...form.preferences })

    // 7. Marcar Onboarding como Concluído no Servidor
    applyStatusMessage.value = 'Finalizando assistente...'
    await onboardingStore.completeOnboarding()

    appliedSuccess.value = true
    emit('completed')
  } catch (err: unknown) {
    applyError.value =
      err instanceof Error ? err.message : 'Ocorreu um erro ao aplicar as configurações.'
  } finally {
    applying.value = false
  }
}

function finishAndNavigate(routePath: string) {
  emit('update:modelValue', false)
  void router.push(routePath)
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      currentStep.value = 1
      appliedSuccess.value = false
      applyError.value = null
      void initializeDefaults()
    }
  }
)

onMounted(() => {
  if (props.modelValue) {
    void initializeDefaults()
  }
})
</script>

<style scoped>
.initial-setup-card {
  max-height: 90vh;
}
.max-w-600 {
  max-width: 600px;
}
.step-pill {
  transition: all 0.2s ease;
  user-select: none;
}
.cursor-pointer {
  cursor: pointer;
}
.step-pill:hover {
  opacity: 0.85;
}
</style>

<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-2 mb-md-4" to="/devices">
      Voltar para Dispositivos
    </v-btn>

    <!-- Header do Dispositivo -->
    <v-card elevation="2" class="rounded-lg pa-3 pa-md-4 mb-3 mb-md-6">
      <div
        class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between ga-3 ga-md-4"
      >
        <div class="d-flex align-center ga-3">
          <v-avatar color="primary" size="44" size-md="48" class="mr-2">
            <v-icon color="white">mdi-router-network</v-icon>
          </v-avatar>
          <div>
            <div class="d-flex align-center ga-2 flex-wrap">
              <h1 class="text-h6 text-md-h5 font-weight-bold">
                {{ detailStore.device?.name || `Dispositivo #${deviceId}` }}
              </h1>
              <v-chip
                :color="getStatusColor(detailStore.device?.status)"
                size="small"
                variant="tonal"
                class="px-3"
              >
                <v-icon start size="14">mdi-circle</v-icon>
                {{ (detailStore.device?.status || 'UNKNOWN').toUpperCase() }}
              </v-chip>
            </div>
            <div class="text-caption text-md-subtitle-2 text-grey mt-1 text-break">
              IP: {{ detailStore.device?.ipAddress || 'Não informado' }} · Tipo:
              {{ detailStore.device?.type }} · Fabricante:
              {{ detailStore.device?.vendor || 'Desconhecido' }}
            </div>
          </div>
        </div>

        <div
          class="d-flex flex-wrap align-center justify-start justify-md-end ga-2 w-100 w-md-auto"
        >
          <v-btn
            v-if="can.createMonitor"
            color="primary"
            prepend-icon="mdi-plus"
            size="small"
            class="flex-grow-1 flex-sm-grow-0"
            @click="openMonitorDialog()"
          >
            Novo monitor
          </v-btn>

          <v-btn-group
            v-if="can.anyHeaderAction"
            color="primary"
            density="comfortable"
            variant="outlined"
            divided
            class="device-action-buttons"
          >
            <v-btn
              v-if="can.snmpScan"
              prepend-icon="mdi-radar"
              :loading="detailStore.scanningSnmp"
              aria-label="Configurar monitoramento"
              @click="openScanModal"
            >
              <span class="hidden-md-and-down">Configurar</span>
              <v-tooltip activator="parent" location="bottom" max-width="300">
                Varre o equipamento via SNMP e abre a tela onde você escolhe <b>o que</b> monitorar
                (interfaces, CPU e memória). Descobre portas novas.
              </v-tooltip>
            </v-btn>

            <v-btn
              v-if="can.snmpCollect"
              prepend-icon="mdi-refresh"
              :loading="detailStore.pollingSnmp"
              aria-label="Coletar agora"
              @click="detailStore.triggerSnmpPoll(deviceId)"
            >
              <span class="hidden-md-and-down">Coletar</span>
              <v-tooltip activator="parent" location="bottom" max-width="300">
                Executa agora uma leitura das métricas do que <b>já está</b> monitorado, sem alterar
                a configuração. É o mesmo que o agendador faz a cada ciclo.
              </v-tooltip>
            </v-btn>

            <v-btn
              v-if="can.scanPorts"
              prepend-icon="mdi-lan-connect"
              aria-label="Escanear portas"
              @click="portScanOpen = true"
            >
              <span class="hidden-md-and-down">Portas</span>
              <v-tooltip activator="parent" location="bottom">Escanear portas</v-tooltip>
            </v-btn>

            <v-btn
              v-if="can.editIdentity"
              prepend-icon="mdi-pencil"
              aria-label="Editar dispositivo"
              @click="editDeviceDialog = true"
            >
              <span class="hidden-md-and-down">Editar</span>
              <v-tooltip activator="parent" location="bottom">Editar dispositivo</v-tooltip>
            </v-btn>
          </v-btn-group>
        </div>
      </div>
    </v-card>

    <!-- Abas Interativas -->
    <v-card elevation="2" class="rounded-lg">
      <v-tabs
        v-model="activeTab"
        color="primary"
        align-tabs="title"
        show-arrows
        density="comfortable"
      >
        <v-tab value="overview" prepend-icon="mdi-information-outline">Visão Geral</v-tab>
        <v-tab value="monitors" prepend-icon="mdi-heart-pulse">
          Monitores ({{ detailStore.monitors.length }})
        </v-tab>
        <v-tab value="rules" prepend-icon="mdi-bell-cog-outline">Regras</v-tab>
        <v-tab v-if="can.interfaces" value="interfaces" prepend-icon="mdi-expansion-card">
          Interfaces SNMP ({{ detailStore.interfaces.length }})
        </v-tab>
        <v-tab v-if="can.events" value="events" prepend-icon="mdi-history">
          Histórico de Eventos
        </v-tab>
        <v-tab v-if="can.logs" value="logs" prepend-icon="mdi-text-box-search-outline">Logs</v-tab>
        <v-tab v-if="can.vpn" value="vpn" prepend-icon="mdi-shield-lock-outline">VPN</v-tab>
      </v-tabs>

      <v-divider></v-divider>

      <v-card-text class="pa-2 pa-sm-4 pa-md-6">
        <v-window v-model="activeTab" :touch="false">
          <!-- Aba Visão Geral -->
          <v-window-item value="overview">
            <DeviceOverviewTab
              :device-id="deviceId"
              :can-health="can.health"
              :can-snmp-configured="can.snmpConfigured"
              :can-snmp-connected="can.snmpConnected"
              @open-scan-modal="openScanModal"
              @open-interface-chart="openInterfaceChart"
            />
          </v-window-item>

          <!-- Aba Monitores -->
          <v-window-item value="monitors">
            <DeviceMonitorsTab
              @open-monitor-dialog="openMonitorDialog"
              @reload-monitors="reloadMonitors"
            />
          </v-window-item>

          <!-- Aba Regras -->
          <v-window-item value="rules">
            <DeviceRulesTab
              :device-id="deviceId"
              :device-name="detailStore.device?.name"
              :monitor-names="monitorNames"
              :available-fields="detailStore.capabilities?.alertFields"
            />
          </v-window-item>

          <!-- Aba Interfaces SNMP -->
          <v-window-item value="interfaces">
            <DeviceInterfacesTab @open-interface-chart="openInterfaceChart" />
          </v-window-item>

          <!-- Aba Eventos -->
          <v-window-item value="events">
            <DeviceEventsTab :device-id="deviceId" />
          </v-window-item>

          <!-- Aba Logs: syslog recebido deste dispositivo -->
          <v-window-item value="logs">
            <DeviceLogsTab
              v-if="detailStore.device"
              :device-id="deviceId"
              :device="detailStore.device"
            />
          </v-window-item>

          <!-- Aba VPN -->
          <v-window-item value="vpn">
            <DeviceVpnTab
              @open-vpn-config="openVpnConfig"
              @rotate-vpn-keys="rotateVpnKeys"
              @revoke-vpn-access="revokeVpnAccess"
              @show-vpn-firewall-hints="showVpnFirewallHints"
            />
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <!-- Modal de Descoberta SNMP -->
    <v-dialog v-model="scanModalOpen" :fullscreen="$vuetify.display.xs" max-width="900" scrollable>
      <v-card class="rounded-lg">
        <v-card-title
          class="font-weight-bold d-flex align-center justify-space-between bg-primary text-white pa-4"
        >
          <div class="d-flex align-center ga-2" style="gap: 8px">
            <v-icon>mdi-radar</v-icon>
            <span>Escaneamento & Descoberta SNMP</span>
          </div>
          <div class="d-flex align-center ga-1">
            <v-btn
              variant="text"
              color="white"
              size="small"
              prepend-icon="mdi-tune-vertical"
              @click="profilesDialogOpen = true"
            >
              Perfis SNMP
            </v-btn>
            <v-btn icon variant="text" color="white" @click="scanModalOpen = false">
              <v-icon>mdi-close</v-icon>
            </v-btn>
          </div>
        </v-card-title>

        <v-card-text class="pa-6">
          <div v-if="detailStore.scanningSnmp" class="text-center py-8">
            <v-progress-circular
              indeterminate
              color="primary"
              size="48"
              class="mb-4"
            ></v-progress-circular>
            <div class="text-subtitle-1">
              Escaneando dispositivo via SNMP em {{ detailStore.device?.ipAddress }}...
            </div>
            <div class="text-caption text-grey">Consultando interfaces e uso de CPU/memória...</div>
          </div>

          <div v-else-if="detailStore.scanResult">
            <v-alert
              v-if="Object.keys(detailStore.scanResult.collectorErrors || {}).length"
              type="warning"
              variant="tonal"
              density="compact"
              class="mb-4"
            >
              Coleta parcial:
              {{
                Object.entries(detailStore.scanResult.collectorErrors)
                  .map(([collector, error]) => `${collector}: ${error}`)
                  .join(' · ')
              }}
            </v-alert>
            <v-alert
              v-if="!detailStore.scanResult.snmpResponded"
              type="warning"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-alert-circle-outline"
              title="Nenhuma resposta SNMP"
              text="O dispositivo não respondeu a nenhum OID consultado, mesmo os padrão (sysDescr/sysName). Confira: (1) SNMP está habilitado no próprio equipamento — não só aqui no cadastro; (2) a community configurada aqui bate com a community de leitura configurada no equipamento; (3) a versão SNMP (v1/v2c/v3) está correta; (4) a porta 161/UDP chega ao equipamento a partir deste servidor (sem firewall/NAT no meio)."
            ></v-alert>

            <v-alert
              v-else
              type="info"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-router"
              title="Dispositivo Conectado"
              :subtitle="detailStore.scanResult.systemInfo.sysDescr || 'Dispositivo SNMP'"
            ></v-alert>

            <!-- Perfil Detectado -->
            <v-alert
              v-if="detailStore.scanResult.matchedProfile"
              type="success"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-solar-power-variant-outline"
              :title="`Perfil Detectado: ${detailStore.scanResult.matchedProfile.name}`"
            >
              <div class="d-flex align-center justify-space-between flex-wrap ga-2 mt-1">
                <div>
                  <strong>Fabricante:</strong> {{ detailStore.scanResult.matchedProfile.vendor }} ·
                  <strong>Categoria:</strong> {{ detailStore.scanResult.matchedProfile.category }}
                </div>
                <v-btn
                  size="small"
                  variant="outlined"
                  color="success"
                  prepend-icon="mdi-format-list-bulleted"
                  @click="profilesDialogOpen = true"
                >
                  Gerenciar Perfis
                </v-btn>
              </div>
            </v-alert>

            <v-alert
              v-else-if="detailStore.scanResult.snmpResponded"
              type="info"
              variant="tonal"
              density="compact"
              class="mb-4"
              prepend-icon="mdi-information-outline"
            >
              <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                <span>
                  Dispositivo sem perfil específico mapeado.
                  <template v-if="detailStore.scanResult.interfaces.length === 0">
                    Se for um controlador solar (MPPT), nobreak ou sensor IoT, você pode cadastrar
                    um perfil com os OIDs.
                  </template>
                </span>
                <v-btn
                  size="x-small"
                  variant="outlined"
                  color="primary"
                  prepend-icon="mdi-plus"
                  @click="profilesDialogOpen = true"
                >
                  Cadastrar Perfil
                </v-btn>
              </div>
            </v-alert>

            <!-- Telemetria & Sensores Mapeados -->
            <v-card
              v-if="detailStore.scanResult.sensors && detailStore.scanResult.sensors.length > 0"
              variant="outlined"
              class="mb-6 rounded-lg pa-4"
            >
              <div class="d-flex align-center justify-space-between mb-3">
                <div
                  class="text-subtitle-1 font-weight-bold d-flex align-center ga-2"
                  style="gap: 8px"
                >
                  <v-icon color="primary">mdi-lightning-bolt</v-icon>
                  Sensores & Telemetria do Equipamento ({{ detailStore.scanResult.sensors.length }})
                </div>
                <div class="d-flex align-center ga-2" style="gap: 8px">
                  <v-btn size="small" variant="text" color="primary" @click="selectAllSensors">
                    Selecionar Todos
                  </v-btn>
                  <v-btn size="small" variant="text" color="grey" @click="unselectAllSensors">
                    Desmarcar Todos
                  </v-btn>
                </div>
              </div>

              <v-row>
                <v-col
                  v-for="sensor in detailStore.scanResult.sensors"
                  :key="sensor.key"
                  cols="12"
                  sm="6"
                  md="4"
                >
                  <v-card
                    variant="tonal"
                    color="surface-variant"
                    class="pa-3 rounded-lg fill-height d-flex flex-column justify-space-between border"
                  >
                    <div class="d-flex align-center justify-space-between">
                      <div
                        class="text-caption font-weight-bold text-truncate"
                        :title="sensor.label"
                      >
                        {{ sensor.label }}
                      </div>
                      <v-switch
                        :model-value="selectedSensors.includes(sensor.key)"
                        color="primary"
                        density="compact"
                        hide-details
                        @update:model-value="toggleSensor(sensor.key)"
                      ></v-switch>
                    </div>
                    <div class="mt-2 d-flex align-center justify-space-between">
                      <div class="text-h6 font-weight-bold text-primary text-no-wrap">
                        {{ formatSensorDisplay(sensor) }}
                      </div>
                      <v-tooltip v-if="sensor.oid" location="top">
                        <template #activator="{ props: tooltipProps }">
                          <v-icon v-bind="tooltipProps" size="16" color="grey-darken-1">
                            mdi-information-outline
                          </v-icon>
                        </template>
                        <span>OID: {{ sensor.oid }}</span>
                      </v-tooltip>
                    </div>
                  </v-card>
                </v-col>
              </v-row>
            </v-card>

            <v-card
              v-if="hasCpuData || hasMemoryData"
              variant="outlined"
              class="mb-6 rounded-lg pa-4"
            >
              <div
                class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-chip</v-icon>
                Monitoramento de Recursos da CPU & Memória
              </div>
              <v-row>
                <v-col v-if="hasCpuData" cols="12" md="6">
                  <v-switch
                    v-model="selectedCpuMonitor"
                    color="primary"
                    label="Monitorar Uso de CPU (%)"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-grey ml-8">
                    {{
                      detailStore.scanResult.cpuInfo.coresCount
                        ? `${detailStore.scanResult.cpuInfo.coresCount} núcleos detectados`
                        : 'Medição via MIB'
                    }}
                    <span v-if="detailStore.scanResult.cpuInfo.usagePercent != null">
                      - Uso Atual: {{ detailStore.scanResult.cpuInfo.usagePercent.toFixed(1) }}%
                    </span>
                  </div>
                </v-col>
                <v-col v-if="hasMemoryData" cols="12" md="6">
                  <v-switch
                    v-model="selectedMemoryMonitor"
                    color="primary"
                    label="Monitorar memória RAM"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-grey ml-8">
                    <span v-if="detailStore.scanResult.memoryInfo.usedKb != null">
                      Uso: {{ formatBinaryBytes(detailStore.scanResult.memoryInfo.usedKb * 1024) }}
                      <template v-if="detailStore.scanResult.memoryInfo.totalKb">
                        / {{ formatBinaryBytes(detailStore.scanResult.memoryInfo.totalKb * 1024) }}
                      </template>
                    </span>
                    <span v-if="detailStore.scanResult.memoryInfo.usedPercent != null" class="ml-1">
                      ({{ detailStore.scanResult.memoryInfo.usedPercent.toFixed(1) }}%)
                    </span>
                  </div>
                </v-col>
              </v-row>
            </v-card>

            <div class="d-flex align-center justify-space-between mb-3">
              <div
                class="text-subtitle-1 font-weight-bold d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-ethernet-cable</v-icon>
                Interfaces de Rede Descobertas ({{ detailStore.scanResult.interfaces.length }})
              </div>
              <div class="d-flex align-center ga-2" style="gap: 8px">
                <v-btn size="small" variant="text" color="primary" @click="selectAllInterfaces">
                  Selecionar Todas
                </v-btn>
                <v-btn size="small" variant="text" color="grey" @click="unselectAllInterfaces">
                  Desmarcar Todas
                </v-btn>
              </div>
            </div>

            <div class="table-responsive">
              <v-table border hover class="rounded-lg">
                <thead>
                  <tr>
                    <th style="width: 50px">Monitorar</th>
                    <th>Index</th>
                    <th>Nome Interface</th>
                    <th>MAC Address</th>
                    <th>Velocidade</th>
                    <th>Status Operacional</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="iface in detailStore.scanResult.interfaces" :key="iface.ifIndex">
                    <td>
                      <v-checkbox
                        :model-value="selectedIfIndexes.includes(iface.ifIndex)"
                        color="primary"
                        hide-details
                        @update:model-value="toggleInterface(iface.ifIndex)"
                      ></v-checkbox>
                    </td>
                    <td>{{ iface.ifIndex }}</td>
                    <td class="font-weight-bold">{{ iface.ifName }}</td>
                    <td>{{ iface.macAddress || 'N/A' }}</td>
                    <td>
                      <v-chip size="x-small" variant="tonal" color="info">
                        {{ formatLinkSpeed(iface.ifSpeed) }}
                      </v-chip>
                    </td>
                    <td>
                      <v-chip
                        :color="iface.ifOperStatus === 'up' ? 'success' : 'error'"
                        size="x-small"
                      >
                        {{ iface.ifOperStatus ? iface.ifOperStatus.toUpperCase() : 'DOWN' }}
                      </v-chip>
                    </td>
                  </tr>
                  <tr v-if="detailStore.scanResult.interfaces.length === 0">
                    <td colspan="6" class="text-center text-grey py-6">
                      <div class="d-flex flex-column align-center ga-1">
                        <v-icon size="32" color="grey">mdi-ethernet-off</v-icon>
                        <div class="font-weight-medium">
                          Nenhuma interface de rede convencional detectada via SNMP.
                        </div>
                        <div class="text-caption">
                          Equipamentos como controladores solares MPPT, nobreaks e sensores IoT não
                          utilizam portas de rede para medição. O monitoramento é realizado através
                          da telemetria de sensores acima.
                        </div>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>
          </div>

          <div v-else-if="detailStore.error" class="py-4">
            <v-alert
              type="error"
              variant="tonal"
              class="mb-4 rounded-lg"
              prepend-icon="mdi-alert-circle"
              title="Falha na Comunicação SNMP"
            >
              <div class="mb-3 text-body-2 font-weight-medium">{{ detailStore.error }}</div>
              <v-divider class="my-2"></v-divider>
              <div class="text-caption">
                <strong>Diagnóstico e possíveis causas:</strong>
                <ul class="pl-4 mt-2">
                  <li>
                    O serviço SNMP pode estar desabilitado no equipamento ou restrito por regras de
                    Firewall/ACL.
                  </li>
                  <li>
                    A community SNMP configurada no cadastro deste dispositivo pode não coincidir
                    com a permitida no equipamento (padrão comum: <code>public</code>).
                  </li>
                  <li>
                    A porta 161 UDP pode estar inacessível ou filtrada na rota até o endereço
                    <code>{{ detailStore.device?.ipAddress }}</code
                    >.
                  </li>
                  <li>A versão SNMP (v1, v2c ou v3) difere da configurada no dispositivo.</li>
                </ul>
              </div>
            </v-alert>
            <div class="text-center">
              <v-btn
                color="primary"
                variant="outlined"
                prepend-icon="mdi-refresh"
                @click="openScanModal"
              >
                Tentar Novamente
              </v-btn>
            </div>
          </div>
          <div v-else class="text-center py-8 text-grey">
            Nenhuma informação disponível para exibição.
          </div>
        </v-card-text>

        <v-divider></v-divider>

        <v-card-actions class="pa-4 justify-end">
          <v-btn variant="text" @click="scanModalOpen = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            prepend-icon="mdi-check"
            :loading="savingMonitors"
            :disabled="!detailStore.scanResult || detailStore.scanningSnmp"
            @click="saveMonitors"
          >
            Salvar Configurações de Monitoramento
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog
      v-model="snmpRemovalConfirmation"
      :fullscreen="$vuetify.display.xs"
      max-width="560"
      persistent
    >
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold d-flex align-center ga-2">
          <v-icon color="warning">mdi-alert-circle-outline</v-icon>
          Remover do Monitoramento & Histórico?
        </v-card-title>
        <v-card-text>
          <p class="mb-3">
            Os seguintes itens foram desmarcados ou não constam mais na varredura SNMP do
            equipamento:
          </p>
          <v-list density="compact" class="bg-grey-lighten-4 rounded-lg mb-3 pa-2">
            <v-list-item
              v-for="(item, idx) in removedItems"
              :key="idx"
              density="compact"
              prepend-icon="mdi-minus-circle-outline"
              :title="item"
            ></v-list-item>
          </v-list>
          <p class="text-body-2 text-grey-darken-1 mb-0">
            Deseja apagar o histórico de métricas, coletas e eventos dos itens removidos, ou deseja
            manter o histórico existente?
          </p>
        </v-card-text>
        <v-card-actions class="justify-end flex-wrap ga-2 pa-4">
          <v-btn variant="text" @click="snmpRemovalConfirmation = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            variant="tonal"
            :loading="savingMonitors"
            @click="confirmSaveMonitors(false)"
          >
            Manter Histórico
          </v-btn>
          <v-btn
            color="error"
            variant="flat"
            :loading="savingMonitors"
            @click="confirmSaveMonitors(true)"
          >
            Apagar Histórico
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Modal de Gráfico de Tráfego de Interface -->
    <TrafficChartDialog
      v-model="chartDialogOpen"
      :interface-id="selectedInterface?.id ?? null"
      :interface-name="selectedInterface ? interfaceLabel(selectedInterface) : ''"
      :initial-metric="selectedChartMetricType"
      :metrics="detailStore.metrics"
      can-manage-monitoring
      :is-monitored="selectedInterface?.isMonitored === true"
      :busy="detailStore.updatingInterfaceId === selectedInterface?.id"
      @toggle-monitoring="toggleInterfaceMonitoring"
    />

    <!-- Modais da aba VPN -->
    <VpnScriptViewer v-model="vpnViewerOpen" :artifact="vpnStore.lastArtifact" :qr-svg="null" />
    <VpnFirewallHintsDialog v-model="vpnFirewallOpen" :content="vpnFirewallContent" />

    <!-- Modal de Scanner de Portas TCP/UDP -->
    <PortScanDialog
      v-model="portScanOpen"
      :host="detailStore.device?.ipAddress"
      :device-name="detailStore.device?.name"
    />

    <!-- Monitor deste equipamento: o vínculo já vem definido e travado -->
    <MonitorFormDialog
      v-model="monitorDialog"
      :monitor="editingMonitor"
      :default-device-id="deviceId"
      lock-device
      @saved="onMonitorSaved"
    />

    <!-- Modal de Edição do Equipamento -->
    <DeviceDialog
      v-model="editDeviceDialog"
      :device-to-edit="detailStore.device"
      @saved="onDeviceSaved"
    />

    <!-- Modal de Perfis SNMP -->
    <SnmpProfilesDialog v-model="profilesDialogOpen" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  useDeviceDetailStore,
  type DeviceInterface,
  type DeviceMonitor,
  type DiscoveredSensorItem,
} from '@/stores/deviceDetail'
import TrafficChartDialog from '@/components/TrafficChartDialog.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import VpnFirewallHintsDialog from '@/components/VpnFirewallHintsDialog.vue'
import PortScanDialog from '@/components/PortScanDialog.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import DeviceDialog from '@/components/DeviceDialog.vue'
import SnmpProfilesDialog from '@/components/devices/SnmpProfilesDialog.vue'
import DeviceRulesTab from '@/components/devices/DeviceRulesTab.vue'
import DeviceOverviewTab from '@/components/devices/tabs/DeviceOverviewTab.vue'
import DeviceMonitorsTab from '@/components/devices/tabs/DeviceMonitorsTab.vue'
import DeviceInterfacesTab from '@/components/devices/tabs/DeviceInterfacesTab.vue'
import DeviceEventsTab from '@/components/devices/tabs/DeviceEventsTab.vue'
import DeviceLogsTab from '@/components/devices/tabs/DeviceLogsTab.vue'
import DeviceVpnTab from '@/components/devices/tabs/DeviceVpnTab.vue'
import { getStatusColor } from '@/utils/monitorPresentation'
import { formatBinaryBytes, formatLinkSpeed } from '@/utils/formatters'
import { useVpnStore } from '@/stores/vpn'
import { useLogsStore } from '@/stores/logs'
import { confirm } from '@/composables/useConfirm'

const route = useRoute()
const router = useRouter()
const detailStore = useDeviceDetailStore()
const vpnStore = useVpnStore()
const logsStore = useLogsStore()
const activeTab = ref('overview')
const profilesDialogOpen = ref(false)
const selectedSensors = ref<string[]>([])

const can = computed(() => {
  const caps = detailStore.capabilities
  const snmpConnected = caps?.snmpConnected ?? false
  const isSystem = caps?.isSystem ?? false
  const snmpScan = caps?.canSnmpScan ?? !isSystem
  const snmpCollect = caps?.canSnmpCollect ?? false
  const scanPorts = caps?.canScanPorts ?? false
  const editIdentity = caps?.canEditIdentity ?? !isSystem
  return {
    isSystem,
    snmpConfigured: caps?.snmpConfigured ?? false,
    snmpConnected,
    interfaces: caps?.interfaces ?? false,
    events: caps?.events ?? false,
    logs: caps?.logs ?? false,
    vpn: caps?.vpn ?? Boolean(detailStore.device?.vpnPeer),
    health: caps?.health ?? false,
    snmpScan,
    snmpCollect,
    scanPorts,
    editIdentity,
    createMonitor: caps?.canCreateMonitor ?? !isSystem,
    anyHeaderAction: snmpScan || snmpCollect || scanPorts || editIdentity,
  }
})

const abasAplicaveis = computed(() => {
  const abas = ['overview', 'monitors', 'rules']
  if (can.value.interfaces) abas.push('interfaces')
  if (can.value.events) abas.push('events')
  if (can.value.logs) abas.push('logs')
  if (can.value.vpn) abas.push('vpn')
  return abas
})

const monitorNames = computed<Record<number, string>>(() =>
  Object.fromEntries(detailStore.monitors.map((monitor) => [monitor.id, monitor.name]))
)
const scanModalOpen = ref(false)
const savingMonitors = ref(false)
const portScanOpen = ref(false)
const editDeviceDialog = ref(false)

async function onDeviceSaved() {
  if (deviceId.value) {
    await detailStore.loadDeviceDetails(deviceId.value)
  }
}

const chartDialogOpen = ref(false)
const selectedInterface = ref<DeviceInterface | null>(null)
const selectedChartMetricType = ref<'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'>(
  'inBps'
)

const vpnViewerOpen = ref(false)
const vpnFirewallOpen = ref(false)
const vpnFirewallContent = ref('')

function interfaceLabel(intf: DeviceInterface): string {
  return intf.ifName || intf.name || `if-${intf.id}`
}

function openInterfaceChart(
  intf: DeviceInterface,
  metricType: 'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined' = 'combined'
) {
  selectedInterface.value = intf
  selectedChartMetricType.value = metricType
  chartDialogOpen.value = true
}

async function toggleInterfaceMonitoring(enabled: boolean) {
  const target = selectedInterface.value
  if (!target) return

  const success = await detailStore.setInterfaceMonitoring(deviceId.value, target.id, enabled)
  if (success) {
    selectedInterface.value =
      detailStore.interfaces.find((intf) => intf.id === target.id) ?? selectedInterface.value
  }
}

const selectedCpuMonitor = ref(false)
const selectedMemoryMonitor = ref(false)

const hasCpuData = computed(() => {
  const cpu = detailStore.scanResult?.cpuInfo
  return Boolean(cpu && cpu.usagePercent != null)
})
const hasMemoryData = computed(() => {
  const mem = detailStore.scanResult?.memoryInfo
  return Boolean(mem && (mem.usedPercent != null || mem.totalKb != null))
})
const selectedIfIndexes = ref<number[]>([])

const deviceId = computed(() => Number(route.params.id))

onMounted(() => {
  if (deviceId.value) {
    detailStore.loadDeviceDetails(deviceId.value)
    vpnStore.fetchServer()
  }
})

watch(
  [abasAplicaveis, () => route.query.tab],
  ([abas, pedida]) => {
    const alvo = typeof pedida === 'string' ? pedida : activeTab.value
    activeTab.value = abas.includes(alvo) ? alvo : 'overview'
  },
  { immediate: true }
)

watch(activeTab, (aba, anterior) => {
  if (aba === 'logs') {
    void logsStore.fetchSources()
  } else if (anterior === 'logs') {
    logsStore.stopTail()
  }
})

const monitorDialog = ref(false)
const editingMonitor = ref<DeviceMonitor | null>(null)

function openMonitorDialog(monitor?: DeviceMonitor) {
  editingMonitor.value = monitor ?? null
  monitorDialog.value = true
}

async function onMonitorSaved() {
  if (deviceId.value) await detailStore.loadDeviceDetails(deviceId.value)
}

async function reloadMonitors() {
  if (deviceId.value) await detailStore.reloadMonitors(deviceId.value)
}

async function openVpnConfig() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return
  const artifact = await vpnStore.fetchConfig(vpnPeer.id)
  if (artifact) vpnViewerOpen.value = true
}

async function rotateVpnKeys() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return

  const ok = await confirm({
    title: 'Gerar novas chaves VPN',
    message: `Gerar novas chaves para "${detailStore.device?.name}"? A configuração atual deixará de funcionar.`,
    confirmText: 'Gerar novas chaves',
    confirmColor: 'warning',
    icon: 'mdi-key-change',
  })
  if (!ok) return

  const artifact = await vpnStore.rotateKeys(vpnPeer.id)
  if (artifact) {
    vpnViewerOpen.value = true
    await detailStore.loadDeviceDetails(deviceId.value)
  }
}

async function revokeVpnAccess() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return

  const ok = await confirm({
    title: 'Revogar acesso VPN',
    message: `Revogar o acesso VPN de "${detailStore.device?.name}"? O túnel cai imediatamente, o IP é liberado e este dispositivo será removido.`,
    confirmText: 'Revogar acesso',
    confirmColor: 'error',
    icon: 'mdi-shield-remove-outline',
  })
  if (!ok) return

  const success = await vpnStore.revokePeer(vpnPeer.id)
  if (success) {
    router.push({ name: 'vpn-devices' })
  }
}

async function showVpnFirewallHints() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return
  const content = await vpnStore.fetchFirewallHints(vpnPeer.id)
  if (!content) return

  vpnFirewallContent.value = content
  vpnFirewallOpen.value = true
}

async function openScanModal() {
  scanModalOpen.value = true
  const res = await detailStore.scanDeviceSnmp(deviceId.value)
  if (res) {
    const cpuSupported = Boolean(res.cpuInfo && res.cpuInfo.usagePercent != null)
    const memSupported = Boolean(
      res.memoryInfo && (res.memoryInfo.usedPercent != null || res.memoryInfo.totalKb != null)
    )
    selectedCpuMonitor.value = cpuSupported && res.hasCpuMonitor
    selectedMemoryMonitor.value = memSupported && res.hasMemoryMonitor
    selectedIfIndexes.value = res.interfaces.filter((i) => i.isMonitored).map((i) => i.ifIndex)
    const monitoredSensors = (res.sensors || []).filter((s) => s.isMonitored).map((s) => s.key)
    selectedSensors.value =
      monitoredSensors.length > 0 ? monitoredSensors : (res.sensors || []).map((s) => s.key)
  }
}

function toggleInterface(ifIndex: number) {
  const idx = selectedIfIndexes.value.indexOf(ifIndex)
  if (idx > -1) {
    selectedIfIndexes.value.splice(idx, 1)
  } else {
    selectedIfIndexes.value.push(ifIndex)
  }
}

function selectAllInterfaces() {
  if (detailStore.scanResult) {
    selectedIfIndexes.value = detailStore.scanResult.interfaces.map((i) => i.ifIndex)
  }
}

function unselectAllInterfaces() {
  selectedIfIndexes.value = []
}

function toggleSensor(sensorKey: string) {
  const idx = selectedSensors.value.indexOf(sensorKey)
  if (idx > -1) {
    selectedSensors.value.splice(idx, 1)
  } else {
    selectedSensors.value.push(sensorKey)
  }
}

function selectAllSensors() {
  if (detailStore.scanResult?.sensors) {
    selectedSensors.value = detailStore.scanResult.sensors.map((s) => s.key)
  }
}

function unselectAllSensors() {
  selectedSensors.value = []
}

function formatSensorDisplay(sensor: DiscoveredSensorItem): string {
  // 1. Se o sensor possui estados declarados no perfil SNMP:
  const rawNum = sensor.rawValue ?? sensor.value
  if (sensor.states && rawNum != null && Number.isFinite(rawNum)) {
    const key = String(Math.round(rawNum))
    const s = sensor.states[key]
    if (s) {
      return typeof s === 'string' ? s : s.label
    }
  }

  // 2. Se já veio formatado pelo backend com texto descritivo:
  if (sensor.formattedValue && isNaN(Number(sensor.formattedValue))) {
    return sensor.formattedValue
  }

  // 3. Se for numérico, exibe o valor formatado com a unidade física do perfil:
  if (sensor.value != null && Number.isFinite(sensor.value)) {
    if (sensor.unit === 'kWh') {
      return `${sensor.value.toFixed(2)} kWh`
    }
    const valStr = sensor.value % 1 === 0 ? sensor.value.toFixed(0) : sensor.value.toFixed(1)
    return sensor.unit ? `${valStr} ${sensor.unit}` : valStr
  }

  return sensor.formattedValue || 'Sem leitura'
}

const snmpRemovalConfirmation = ref(false)

const removedItems = computed(() => {
  const items: string[] = []
  if (!detailStore.scanResult) return items

  const unselectedInterfaces = detailStore.interfaces.filter((i) => {
    const idx = i.snmpIndex ?? i.ifIndex
    return i.isMonitored && idx != null && !selectedIfIndexes.value.includes(idx)
  })
  for (const intf of unselectedInterfaces) {
    items.push(`Interface "${interfaceLabel(intf)}" (desmarcada do monitoramento)`)
  }

  const scanIndexes = new Set(detailStore.scanResult.interfaces.map((i) => i.ifIndex))
  const missingInterfaces = detailStore.interfaces.filter((i) => {
    const idx = i.snmpIndex ?? i.ifIndex
    return idx != null && !scanIndexes.has(idx)
  })
  for (const intf of missingInterfaces) {
    if (!unselectedInterfaces.includes(intf)) {
      items.push(`Interface "${interfaceLabel(intf)}" (não encontrada no dispositivo via SNMP)`)
    }
  }

  if (detailStore.scanResult.hasCpuMonitor && !selectedCpuMonitor.value) {
    items.push('Monitor de Uso de CPU (desmarcado)')
  }
  if (detailStore.scanResult.hasMemoryMonitor && !selectedMemoryMonitor.value) {
    items.push('Monitor de Uso de Memória (desmarcado)')
  }

  if (detailStore.scanResult.sensors) {
    for (const sensor of detailStore.scanResult.sensors) {
      if (sensor.isMonitored && !selectedSensors.value.includes(sensor.key)) {
        items.push(`Sensor "${sensor.label}" (desmarcado do monitoramento)`)
      }
    }
  }

  return items
})

async function saveMonitors() {
  if (removedItems.value.length > 0) {
    snmpRemovalConfirmation.value = true
    return
  }
  await doApplyMonitors(false)
}

async function confirmSaveMonitors(clearHistory: boolean) {
  snmpRemovalConfirmation.value = false
  await doApplyMonitors(clearHistory)
}

async function doApplyMonitors(clearRemovedHistory: boolean) {
  savingMonitors.value = true
  try {
    const success = await detailStore.applySnmpMonitors(deviceId.value, {
      enableCpuMonitor: hasCpuData.value ? selectedCpuMonitor.value : false,
      enableMemoryMonitor: hasMemoryData.value ? selectedMemoryMonitor.value : false,
      monitoredIfIndexes: selectedIfIndexes.value,
      monitoredSensors: selectedSensors.value,
      clearRemovedHistory,
    })
    if (success) {
      scanModalOpen.value = false
    }
  } finally {
    savingMonitors.value = false
  }
}
</script>

<style scoped>
@media (max-width: 599.98px) {
  .device-action-buttons {
    width: 100%;
  }

  .device-action-buttons :deep(.v-btn) {
    flex: 1 1 0;
    min-width: 0;
  }
}
</style>

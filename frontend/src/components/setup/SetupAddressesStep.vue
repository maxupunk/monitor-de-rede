<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">Endereços por onde este Servidor é Alcançado</div>
      <div class="text-caption text-high-emphasis">
        Equipamentos na mesma rede usam a LAN; quem vem pela internet usa o IP público. O NetMonitor
        usa essa lista para sugerir configurações de Syslog e VPN.
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
                <v-chip size="x-small" color="primary" variant="flat"> Recomendado Padrão </v-chip>
              </div>
              <div class="text-caption text-high-emphasis mb-2">
                Usado por switches, roteadores e servidores na mesma rede física.
              </div>
              <v-text-field
                v-model="model.lan"
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
                  :loading="detecting"
                  @click="emit('detect')"
                >
                  Detectar IP Externo
                </v-btn>
              </div>
              <div class="text-caption text-high-emphasis mb-2">
                Usado por filiais e dispositivos remotos que acessam o servidor via internet.
              </div>
              <v-text-field
                v-model="model.public"
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
      <v-col v-if="showVpn" cols="12">
        <v-sheet border rounded="lg" class="pa-4 bg-surface">
          <div class="d-flex align-start ga-3">
            <v-avatar color="deep-purple" size="38" rounded="lg" variant="tonal">
              <v-icon size="20">mdi-shield-lock-outline</v-icon>
            </v-avatar>
            <div class="flex-grow-1 min-width-0">
              <span class="font-weight-bold">Túnel VPN (WireGuard)</span>
              <div class="text-caption text-high-emphasis mb-2">
                Endereço interno do servidor dentro do túnel criptografado.
              </div>
              <v-text-field
                v-model="model.vpn"
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

      <!-- Domínio -->
      <v-col cols="12">
        <v-sheet border rounded="lg" class="pa-4 bg-surface">
          <div class="d-flex align-start ga-3">
            <v-avatar color="indigo" size="38" rounded="lg" variant="tonal">
              <v-icon size="20">mdi-earth</v-icon>
            </v-avatar>
            <div class="flex-grow-1 min-width-0">
              <span class="font-weight-bold">Domínio (opcional)</span>
              <div class="text-caption text-high-emphasis mb-2">
                Nome pelo qual o servidor é acessado. Quando preenchido, vira o endereço padrão dos
                comandos de instalação dos servidores remotos.
              </div>
              <v-text-field
                v-model="model.domain"
                placeholder="Ex: monitor.empresa.com.br"
                variant="outlined"
                density="compact"
                prepend-inner-icon="mdi-web"
                hide-details="auto"
              ></v-text-field>
              <v-switch
                v-model="model.domainHttps"
                :disabled="!model.domain.trim()"
                color="primary"
                density="compact"
                inset
                hide-details
                label="Atrás de um proxy HTTPS"
                class="mt-1"
              ></v-switch>
            </div>
          </div>
        </v-sheet>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import type { AddressesForm } from './setupForm'

defineProps<{
  /** Detectando o IP público agora. */
  detecting: boolean
  /** O endereço no túnel só interessa com a área VPN ligada. */
  showVpn: boolean
}>()

const emit = defineEmits<{ detect: [] }>()

const model = defineModel<AddressesForm>({ required: true })
</script>

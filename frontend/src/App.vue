<template>
  <router-view />
  <ConfirmDialog />
  <DeviceAccessHost />
</template>

<script setup lang="ts">
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import DeviceAccessHost from '@/components/devices/access/DeviceAccessHost.vue'
import { usePwaThemeColor } from '@/composables/usePwaThemeColor'
import { useThemePreference } from '@/composables/useThemePreference'

// Inicializa a sincronização automática e contínua do status bar / app bar no PWA
usePwaThemeColor()
// Tema escolhido no menu do usuário; em "Do sistema", acompanha o sistema operacional.
useThemePreference().apply()
</script>

<template>
  <router-view />
  <ConfirmDialog />
</template>

<script setup lang="ts">
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import { usePwaThemeColor } from '@/composables/usePwaThemeColor'

// Inicializa a sincronização automática e contínua do status bar / app bar no PWA
usePwaThemeColor()
</script>

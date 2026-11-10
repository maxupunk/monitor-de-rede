<template>
  <StatusMetaChip :meta="runStatusMeta(status)" :size="size" />
</template>

<script setup lang="ts">
import StatusMetaChip from '@/components/deploys/StatusMetaChip.vue'
import type { DeployRunStatus } from '@/bindings/DeployRunStatus'
import { runStatusMeta } from '@/utils/deployPresentation'

withDefaults(defineProps<{ status: DeployRunStatus; size?: string }>(), { size: 'small' })
</script>

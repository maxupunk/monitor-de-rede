<template>
  <TopologyActionList :actions="actions" />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  resolveActions,
  type TopologyHost,
  type TopologyTarget,
} from '@/utils/topology/inspectorRegistry'
import TopologyActionList from '../TopologyActionList.vue'

const props = defineProps<{ target: TopologyTarget; host: TopologyHost }>()
const actions = computed(() => resolveActions(props.target, props.host))
</script>

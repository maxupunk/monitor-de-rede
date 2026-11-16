<template>
  <div>
    <div class="d-flex align-center flex-wrap ga-2 mb-1">
      <span class="text-subtitle-2 font-weight-medium">O que este servidor vai liberar</span>
      <v-spacer></v-spacer>
      <v-btn size="small" variant="text" color="primary" @click="setAll(true)">Tudo</v-btn>
      <v-btn size="small" variant="text" color="primary" @click="setAll(false)">Só leitura</v-btn>
    </div>
    <p class="text-caption text-high-emphasis mb-2">
      Vai no comando como <code>AGENT_ALLOW</code> e fica gravado no servidor: depois, só lá dá para
      mudar.
    </p>
    <v-row dense>
      <v-col v-for="permission in PERMISSION_ORDER" :key="permission" cols="12" sm="6">
        <v-checkbox
          :model-value="modelValue.allow.includes(permission)"
          :disabled="permission === 'read'"
          :color="permissionColor(permission)"
          density="compact"
          hide-details
          @update:model-value="toggle(permission, Boolean($event))"
        >
          <template #label>
            <div>
              <div class="text-body-2">{{ PERMISSIONS[permission].label }}</div>
              <div class="text-caption text-high-emphasis">
                {{ PERMISSIONS[permission].description }}
              </div>
            </div>
          </template>
        </v-checkbox>
      </v-col>
    </v-row>
    <v-text-field
      v-if="modelValue.allow.includes('database')"
      v-model="targets"
      label="Bancos que a ponte alcança (AGENT_DATABASE_TARGETS)"
      hint="IP:porta ou faixa:porta, separados por vírgula. O padrão cobre os containers do Docker (PostgreSQL 5432, MySQL/MariaDB 3306)."
      persistent-hint
      variant="outlined"
      density="compact"
      class="mt-3"
      @blur="commitTargets"
      @keydown.enter.prevent="commitTargets"
    ></v-text-field>
  </div>
</template>

<script setup lang="ts">
/**
 * As permissões que o comando de instalação libera no host. A tela marca
 * tudo; o operador desmarca o que não quer. Leitura é sempre liberada.
 */
import { ref, watch } from 'vue'
import type { InstallPolicyView } from '@/bindings/InstallPolicyView'
import type { Permission } from '@/bindings/Permission'
import { PERMISSION_ORDER, PERMISSIONS, permissionColor } from '@/utils/agentPermissions'

const props = defineProps<{ modelValue: InstallPolicyView }>()
const emit = defineEmits<{ (event: 'update:modelValue', value: InstallPolicyView): void }>()

const targets = ref(props.modelValue.databaseTargets)
watch(
  () => props.modelValue.databaseTargets,
  (value) => {
    targets.value = value
  }
)

function update(allow: Permission[], databaseTargets = props.modelValue.databaseTargets) {
  const unique = PERMISSION_ORDER.filter((item) => item === 'read' || allow.includes(item))
  emit('update:modelValue', { allow: unique, databaseTargets })
}

function toggle(permission: Permission, checked: boolean) {
  const rest = props.modelValue.allow.filter((item) => item !== permission)
  update(checked ? [...rest, permission] : rest)
}

function setAll(all: boolean) {
  update(all ? [...PERMISSION_ORDER] : ['read'])
}

function commitTargets() {
  if (targets.value.trim() !== props.modelValue.databaseTargets) {
    update(props.modelValue.allow, targets.value.trim())
  }
}
</script>

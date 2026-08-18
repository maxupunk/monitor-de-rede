<template>
  <div>
    <PageHeader
      title="Usuários e acessos"
      subtitle="Gerencie quem pode consultar ou alterar o monitoramento."
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-account-plus-outline" @click="openCreate">
          Novo usuário
        </v-btn>
      </template>
    </PageHeader>

    <v-row class="mb-2">
      <v-col v-for="role in ROLE_OPTIONS" :key="role.value" cols="12" md="4">
        <v-card variant="tonal" color="primary" rounded="lg" height="100%">
          <v-card-text class="d-flex align-start ga-3">
            <v-avatar color="primary" variant="tonal" size="42">
              <v-icon>{{ role.icon }}</v-icon>
            </v-avatar>
            <div>
              <div class="font-weight-bold">{{ role.title }}</div>
              <div class="text-body-2 text-medium-emphasis">{{ role.description }}</div>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <v-alert
      v-if="usersStore.error && !dialog"
      type="error"
      variant="tonal"
      closable
      class="mb-4"
      @click:close="usersStore.clearError"
    >
      {{ usersStore.error }}
    </v-alert>

    <v-card elevation="2" rounded="lg">
      <v-card-title class="pa-4 d-flex flex-column flex-md-row ga-3 align-md-center">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, e-mail ou perfil"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="w-100"
          style="max-width: 460px"
        ></v-text-field>
        <v-spacer></v-spacer>
        <v-chip variant="tonal" color="primary" prepend-icon="mdi-account-multiple-outline">
          {{ usersStore.users.length }} usuário{{ usersStore.users.length === 1 ? '' : 's' }}
        </v-chip>
      </v-card-title>

      <ResponsiveDataTable
        :headers="headers"
        :items="usersStore.users"
        :search="search"
        :loading="usersStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum usuário cadastrado"
        :clickable="false"
      >
        <template #item.fullName="{ item }">
          <div class="py-2">
            <div class="font-weight-bold d-flex align-center ga-2">
              {{ item.fullName }}
              <v-chip v-if="item.id === authStore.user?.id" size="x-small" color="primary">
                Você
              </v-chip>
            </div>
            <div class="text-caption text-medium-emphasis">{{ item.email }}</div>
          </div>
        </template>

        <template #item.role="{ item }">
          <v-chip size="small" variant="tonal" :color="roleColor(item.role)">
            {{ roleLabel(item.role) }}
          </v-chip>
        </template>

        <template #item.active="{ item }">
          <v-chip
            size="small"
            variant="tonal"
            :color="item.active ? 'success' : 'grey-darken-1'"
            :prepend-icon="item.active ? 'mdi-check-circle-outline' : 'mdi-cancel'"
          >
            {{ item.active ? 'Ativo' : 'Desativado' }}
          </v-chip>
        </template>

        <template #item.createdAt="{ item }">
          {{ formatDate(item.createdAt) }}
        </template>

        <template #item.actions="{ item }">
          <div class="d-flex ga-1 justify-end">
            <v-tooltip text="Editar usuário" location="top">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  icon="mdi-pencil-outline"
                  size="small"
                  variant="text"
                  color="primary"
                  :aria-label="`Editar ${item.fullName}`"
                  @click="openEdit(item)"
                ></v-btn>
              </template>
            </v-tooltip>
            <v-tooltip
              :text="
                item.id === authStore.user?.id
                  ? 'Sua própria conta não pode ser excluída'
                  : 'Excluir usuário'
              "
              location="top"
            >
              <template #activator="{ props }">
                <span v-bind="props">
                  <v-btn
                    icon="mdi-delete-outline"
                    size="small"
                    variant="text"
                    color="error"
                    :disabled="item.id === authStore.user?.id"
                    :aria-label="`Excluir ${item.fullName}`"
                    @click="askDelete(item)"
                  ></v-btn>
                </span>
              </template>
            </v-tooltip>
          </div>
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-3">
            <div class="min-width-0">
              <div class="font-weight-bold text-break">{{ item.fullName }}</div>
              <div class="text-body-2 text-medium-emphasis text-break">{{ item.email }}</div>
              <div class="d-flex flex-wrap ga-2 mt-2">
                <v-chip size="x-small" variant="tonal" :color="roleColor(item.role)">
                  {{ roleLabel(item.role) }}
                </v-chip>
                <v-chip size="x-small" variant="tonal" :color="item.active ? 'success' : 'grey'">
                  {{ item.active ? 'Ativo' : 'Desativado' }}
                </v-chip>
              </div>
            </div>
            <div class="d-flex ga-1">
              <v-btn
                icon="mdi-pencil-outline"
                size="small"
                variant="text"
                color="primary"
                @click="openEdit(item)"
              ></v-btn>
              <v-btn
                icon="mdi-delete-outline"
                size="small"
                variant="text"
                color="error"
                :disabled="item.id === authStore.user?.id"
                @click="askDelete(item)"
              ></v-btn>
            </div>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <UserDialog
      v-model="dialog"
      :user="selectedUser"
      :current-user-id="authStore.user?.id"
      @saved="onSaved"
    ></UserDialog>

    <v-dialog v-model="deleteDialog" max-width="460">
      <v-card rounded="xl">
        <v-card-title class="pa-5 pb-2">Excluir usuário?</v-card-title>
        <v-card-text>
          A conta de <strong>{{ userToDelete?.fullName }}</strong> será removida permanentemente e
          perderá o acesso ao sistema.
        </v-card-text>
        <v-card-actions class="pa-4">
          <v-spacer></v-spacer>
          <v-btn variant="text" :disabled="usersStore.saving" @click="deleteDialog = false">
            Cancelar
          </v-btn>
          <v-btn color="error" variant="flat" :loading="usersStore.saving" @click="confirmDelete">
            Excluir
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-snackbar v-model="successVisible" color="success" timeout="3000">
      {{ successMessage }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import UserDialog from '@/components/UserDialog.vue'
import { useAuthStore } from '@/stores/auth'
import { useUsersStore, type ManagedUser } from '@/stores/users'
import { ROLE_OPTIONS, roleLabel, type UserRole } from '@/utils/access'

const usersStore = useUsersStore()
const authStore = useAuthStore()
const search = ref('')
const dialog = ref(false)
const selectedUser = ref<ManagedUser | null>(null)
const deleteDialog = ref(false)
const userToDelete = ref<ManagedUser | null>(null)
const successVisible = ref(false)
const successMessage = ref('')

const headers = [
  { title: 'Usuário', key: 'fullName' },
  { title: 'Perfil', key: 'role', width: '160px' },
  { title: 'Status', key: 'active', width: '140px' },
  { title: 'Criado em', key: 'createdAt', width: '150px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '110px', align: 'end' as const },
]

onMounted(async () => {
  await Promise.all([usersStore.fetchUsers(), authStore.fetchMe()])
})

function roleColor(role: UserRole): string {
  if (role === 'admin') return 'deep-purple'
  if (role === 'operator') return 'primary'
  return 'blue-grey'
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short' }).format(new Date(value))
}

function openCreate() {
  selectedUser.value = null
  dialog.value = true
}

function openEdit(user: ManagedUser) {
  selectedUser.value = user
  dialog.value = true
}

function onSaved() {
  successMessage.value = selectedUser.value
    ? 'Usuário atualizado com sucesso.'
    : 'Usuário criado com sucesso.'
  successVisible.value = true
}

function askDelete(user: ManagedUser) {
  userToDelete.value = user
  usersStore.clearError()
  deleteDialog.value = true
}

async function confirmDelete() {
  if (!userToDelete.value) return
  const deleted = await usersStore.deleteUser(userToDelete.value.id)
  if (!deleted) {
    deleteDialog.value = false
    return
  }
  deleteDialog.value = false
  successMessage.value = 'Usuário excluído com sucesso.'
  successVisible.value = true
  userToDelete.value = null
}
</script>

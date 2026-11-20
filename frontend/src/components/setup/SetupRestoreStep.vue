<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">Restaurar um backup</div>
      <div class="text-body-2">
        Escolha o arquivo. Antes de gravar qualquer coisa, você vê o que ele contém.
      </div>
    </div>

    <v-card border flat class="pa-3 mb-4">
      <div class="font-weight-bold text-body-2 mb-1">O que acontece depois</div>
      <ul class="text-body-2 pl-4 mb-0">
        <li>
          <strong>Backup completo</strong> (<code>.json.gz</code>): os bancos voltam inteiros, com
          os usuários do backup. Se a conta que você acabou de criar não estiver nele, a tela de
          login abre e você entra com uma conta do backup.
        </li>
        <li>
          <strong>Configuração</strong> (<code>.json</code>): dispositivos, monitores, alertas, VPN
          e preferências voltam; a sua conta continua valendo e a página recarrega.
        </li>
        <li>
          Segredos cifrados (VPN, credenciais de equipamentos) só funcionam se esta instalação usar
          a mesma <code>ENCRYPTION_KEY</code> da de origem.
        </li>
      </ul>
    </v-card>

    <BackupFileRestore @restored="emit('restored', $event)" />
  </div>
</template>

<script setup lang="ts">
import type { BackupRestoreResponse } from '@/bindings/BackupRestoreResponse'
import BackupFileRestore from '@/components/backup/BackupFileRestore.vue'

/**
 * A restauração no primeiro acesso. É a mesma da tela de Backup — mesmo
 * componente, mesma rota (`POST /api/backup/restore`, só administrador) —, e
 * por isso só aparece depois do cadastro da conta: não existe caminho sem
 * login para gravar um backup.
 */
const emit = defineEmits<{ restored: [result: BackupRestoreResponse] }>()
</script>

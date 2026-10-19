<template>
  <v-btn
    v-if="auth.canManagePlugins"
    size="small"
    color="primary"
    variant="flat"
    prepend-icon="mdi-package-down"
    :loading="installing"
    v-bind="$attrs"
    @click.stop="install"
  >
    Instalar
  </v-btn>
  <span v-else class="admin-only text-body-small" v-bind="$attrs">
    <v-icon size="16" color="info">mdi-account-key-outline</v-icon>
    Peça a um administrador para instalar o {{ packages }} em {{ fix.deviceName }}.
  </span>
</template>

<script setup lang="ts">
/**
 * "Instalar" do passo `install_package`: pergunta antes (o que acontece no
 * equipamento), roda o `prepare` do plugin com a confirmação de escrita e
 * espera o fim pelo SSE. O teste em si nunca instala — é este aceite que
 * instala. Quem não pode instalar vê a quem pedir.
 */
import { computed, ref } from 'vue'
import { confirm } from '@/composables/useConfirm'
import { useAuthStore } from '@/stores/auth'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTestInstallPackageFix } from '@/types/linkTests'
import { installPackageConfirmText, packagesText } from '@/utils/linkTests/wizard'

defineOptions({ inheritAttrs: false })

const props = defineProps<{ fix: LinkTestInstallPackageFix }>()

const emit = defineEmits<{
  /** O operador aceitou: a instalação começou. */
  started: []
  /** `warning`: instalou, mas há algo a saber (nulo se nada); `runId`: a execução. */
  installed: [warning: string | null, runId: number]
  /** `runId`: a execução, para "Ver execução" (nula se nem começou). */
  failed: [message: string, runId: number | null]
}>()

const auth = useAuthStore()
const store = useLinkTestsStore()
const installing = ref(false)
const packages = computed(() => packagesText(props.fix))

async function install() {
  const accepted = await confirm({
    title: `Instalar o ${packages.value} em ${props.fix.deviceName}?`,
    message: installPackageConfirmText(props.fix),
    confirmText: 'Instalar',
    confirmColor: 'primary',
    icon: 'mdi-package-down',
  })
  if (!accepted) return
  installing.value = true
  emit('started')
  try {
    const outcome = await store.installPackage(props.fix)
    if (outcome.ok) emit('installed', outcome.warning, outcome.runId)
    else emit('failed', outcome.message ?? 'A instalação falhou.', outcome.runId)
  } catch (err: unknown) {
    emit('failed', err instanceof Error && err.message ? err.message : 'A instalação falhou.', null)
  } finally {
    installing.value = false
  }
}
</script>

<style scoped>
.admin-only {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: rgb(var(--v-theme-on-surface));
}
</style>

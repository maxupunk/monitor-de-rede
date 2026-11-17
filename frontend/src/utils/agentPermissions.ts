import type { InstallPolicyView } from '@/bindings/InstallPolicyView'
import type { Permission } from '@/bindings/Permission'

/** Como a tela fala de cada permissão do agente (`AGENT_ALLOW`). */
export const PERMISSIONS: Record<Permission, { label: string; description: string }> = {
  read: { label: 'Leitura', description: 'Containers, logs e métricas do host.' },
  lifecycle: {
    label: 'Ciclo de vida',
    description: 'Iniciar, parar, reiniciar e remover containers, redes e volumes.',
  },
  update: { label: 'Pull / recriar', description: 'Baixar imagem nova e recriar o container.' },
  compose: { label: 'Compose', description: 'Ações de projeto docker compose.' },
  monitor: { label: 'Monitores', description: 'Rodar monitores da central a partir deste site.' },
  discovery: { label: 'Descoberta', description: 'Varrer a rede deste site.' },
  device_io: {
    label: 'Acesso a equipamentos (plugins)',
    description: 'SSH/HTTP/Telnet nos equipamentos da rede local.',
  },
  database: {
    label: 'Ponte de banco (backup)',
    description: 'Backup dos bancos que este host alcança, inclusive os dos containers.',
  },
  volume_export: { label: 'Exportar volumes', description: 'Baixar o conteúdo de um volume.' },
  throughput: { label: 'Teste de vazão (enlace)', description: 'iperf3/btest até um equipamento.' },
  deploy: { label: 'Deploy contínuo', description: 'Atualizar stacks a partir do Git.' },
  build: { label: 'Compilar imagens', description: 'Compilar imagens no builder dedicado.' },
}

/** Ordem de exibição: a do contrato. */
export const PERMISSION_ORDER = Object.keys(PERMISSIONS) as Permission[]

/** Permissões que mexem no host ou nos dados: destacadas como atenção. */
const SENSITIVE: readonly Permission[] = [
  'update',
  'compose',
  'database',
  'volume_export',
  'throughput',
  'deploy',
  'build',
]

export function permissionLabel(permission: Permission): string {
  return PERMISSIONS[permission].label
}

export function permissionColor(permission: Permission): string {
  return SENSITIVE.includes(permission) ? 'warning' : 'primary'
}

/** Mesma faixa inicial de bancos em container usada por `InstallPolicy::suggested()`. */
export const DEFAULT_DATABASE_TARGETS =
  '172.16.0.0/12:5432,172.16.0.0/12:3306,192.168.0.0/16:5432,192.168.0.0/16:3306'

/** Sugestão imediata para a tela abrir sem atraso nem salto de layout. */
export function defaultInstallPolicy(): InstallPolicyView {
  return {
    allow: [...PERMISSION_ORDER],
    databaseTargets: DEFAULT_DATABASE_TARGETS,
  }
}

export function cloneInstallPolicy(policy: InstallPolicyView): InstallPolicyView {
  return {
    allow: [...policy.allow],
    databaseTargets: policy.databaseTargets,
  }
}

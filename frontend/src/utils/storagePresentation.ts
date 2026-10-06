import type { StorageConfig } from '@/bindings/StorageConfig'
import type { StorageProvider } from '@/bindings/StorageProvider'

/** Formato da config que cada provider usa (o `type` do `StorageConfig`). */
export type StorageConfigType = StorageConfig['type']

export interface StorageProviderInfo {
  value: StorageProvider
  label: string
  /** Uma linha que ajuda a escolher entre os cards. */
  hint: string
  icon: string
  color: string
  configType: StorageConfigType
}

/**
 * Catálogo dos providers, na ordem em que aparecem na escolha.
 *
 * É o único lugar que conhece ícone, cor e rótulo de cada um: a lista, o
 * formulário e o diálogo de cópias leem daqui.
 */
export const STORAGE_PROVIDERS: StorageProviderInfo[] = [
  {
    value: 'local',
    label: 'Pasta no servidor',
    hint: 'Disco deste servidor ou um NAS montado nele',
    icon: 'mdi-harddisk',
    color: 'secondary',
    configType: 'local',
  },
  {
    value: 'sftp',
    label: 'SFTP',
    hint: 'NAS, outro servidor Linux, qualquer SSH',
    icon: 'mdi-server-network',
    color: 'success',
    configType: 'sftp',
  },
  {
    value: 'aws_s3',
    label: 'Amazon S3',
    hint: 'Bucket na AWS',
    icon: 'mdi-aws',
    color: 'warning',
    configType: 's3',
  },
  {
    value: 'cloudflare_r2',
    label: 'Cloudflare R2',
    hint: 'Sem custo de saída de dados',
    icon: 'mdi-cloud-lock-outline',
    color: 'warning',
    configType: 's3',
  },
  {
    value: 'minio',
    label: 'MinIO',
    hint: 'S3 próprio, na sua rede',
    icon: 'mdi-bucket-outline',
    color: 'error',
    configType: 's3',
  },
  {
    value: 's3_compatible',
    label: 'Outro S3-compatível',
    hint: 'Backblaze B2, Wasabi, DigitalOcean…',
    icon: 'mdi-database-arrow-up-outline',
    color: 'info',
    configType: 's3',
  },
  {
    value: 'google_gcs',
    label: 'Google Cloud Storage',
    hint: 'Bucket no GCP',
    icon: 'mdi-google-cloud',
    color: 'info',
    configType: 'gcs',
  },
  {
    value: 'azure_blob',
    label: 'Azure Blob Storage',
    hint: 'Container na Azure',
    icon: 'mdi-microsoft-azure',
    color: 'primary',
    configType: 'azure_blob',
  },
]

export function providerInfo(provider: StorageProvider): StorageProviderInfo {
  return STORAGE_PROVIDERS.find((item) => item.value === provider) ?? STORAGE_PROVIDERS[0]!
}

/** Frequências oferecidas no formulário, em horas. */
export const BACKUP_INTERVALS = [
  { value: 6, title: 'A cada 6 horas' },
  { value: 12, title: 'A cada 12 horas' },
  { value: 24, title: 'Diariamente' },
  { value: 168, title: 'Semanalmente' },
]

/** "Diário", "a cada 6 h", "semanal" — o que cabe num chip. */
export function intervalLabel(hours: number): string {
  if (hours === 24) return 'Diário'
  if (hours === 168) return 'Semanal'
  if (hours % 24 === 0) return `A cada ${hours / 24} dias`
  return `A cada ${hours} h`
}

/** Rótulos dos segredos, para a dica "definida — deixe em branco para manter". */
export const SECRET_LABELS: Record<string, string> = {
  secretAccessKey: 'Secret Access Key',
  credentialsJson: 'JSON da conta de serviço',
  connectionString: 'Connection string',
  password: 'Senha',
  privateKey: 'Chave privada',
  passphrase: 'Passphrase',
}

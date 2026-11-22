import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Catálogo de provedores de armazenamento de backup (Local, SFTP, S3/R2/MinIO, GCS, Azure).
@immutable
class StorageProviderInfo {
  const StorageProviderInfo({
    required this.value,
    required this.label,
    required this.hint,
    required this.icon,
    required this.tone,
    required this.configType,
  });

  final String value;
  final String label;
  final String hint;
  final IconData icon;
  final SemanticTone tone;
  final String configType; // 'local' | 'sftp' | 's3' | 'gcs' | 'azure_blob'
}

const List<StorageProviderInfo> storageProvidersCatalog = <StorageProviderInfo>[
  StorageProviderInfo(
    value: 'local',
    label: 'Pasta no servidor',
    hint: 'Disco deste servidor ou um NAS montado nele',
    icon: Icons.storage_outlined,
    tone: SemanticTone.secondary,
    configType: 'local',
  ),
  StorageProviderInfo(
    value: 'sftp',
    label: 'SFTP',
    hint: 'NAS, outro servidor Linux, qualquer SSH',
    icon: Icons.dns_outlined,
    tone: SemanticTone.success,
    configType: 'sftp',
  ),
  StorageProviderInfo(
    value: 'aws_s3',
    label: 'Amazon S3',
    hint: 'Bucket na AWS',
    icon: Icons.cloud_outlined,
    tone: SemanticTone.warning,
    configType: 's3',
  ),
  StorageProviderInfo(
    value: 'cloudflare_r2',
    label: 'Cloudflare R2',
    hint: 'Sem custo de saída de dados',
    icon: Icons.cloud_done_outlined,
    tone: SemanticTone.warning,
    configType: 's3',
  ),
  StorageProviderInfo(
    value: 'minio',
    label: 'MinIO',
    hint: 'S3 próprio, na sua rede',
    icon: Icons.inventory_outlined,
    tone: SemanticTone.error,
    configType: 's3',
  ),
  StorageProviderInfo(
    value: 's3_compatible',
    label: 'Outro S3-compatível',
    hint: 'Backblaze B2, Wasabi, DigitalOcean…',
    icon: Icons.cloud_upload_outlined,
    tone: SemanticTone.info,
    configType: 's3',
  ),
  StorageProviderInfo(
    value: 'google_gcs',
    label: 'Google Cloud Storage',
    hint: 'Bucket no GCP',
    icon: Icons.cloud_queue,
    tone: SemanticTone.info,
    configType: 'gcs',
  ),
  StorageProviderInfo(
    value: 'azure_blob',
    label: 'Azure Blob Storage',
    hint: 'Container na Azure',
    icon: Icons.cloud_circle_outlined,
    tone: SemanticTone.primary,
    configType: 'azure_blob',
  ),
];

StorageProviderInfo storageProviderInfo(String provider) {
  for (final item in storageProvidersCatalog) {
    if (item.value == provider) return item;
  }
  return storageProvidersCatalog.first;
}

import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/utils/status_visual.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Catálogo de permissões `AGENT_ALLOW` e apresentação de agentes (ADR 011 / ADR 013).
@immutable
class AgentPermissionMeta {
  const AgentPermissionMeta({
    required this.key,
    required this.label,
    required this.description,
    required this.sensitive,
  });

  final String key;
  final String label;
  final String description;
  final bool sensitive;
}

const List<AgentPermissionMeta> agentPermissionsCatalog = <AgentPermissionMeta>[
  AgentPermissionMeta(
    key: 'read',
    label: 'Leitura',
    description: 'Containers, logs e métricas do host.',
    sensitive: false,
  ),
  AgentPermissionMeta(
    key: 'lifecycle',
    label: 'Ciclo de vida',
    description:
        'Iniciar, parar, reiniciar e remover containers, redes e volumes.',
    sensitive: false,
  ),
  AgentPermissionMeta(
    key: 'update',
    label: 'Pull / recriar',
    description: 'Baixar imagem nova e recriar o container.',
    sensitive: true,
  ),
  AgentPermissionMeta(
    key: 'compose',
    label: 'Compose',
    description: 'Ações de projeto docker compose.',
    sensitive: true,
  ),
  AgentPermissionMeta(
    key: 'monitor',
    label: 'Monitores',
    description: 'Rodar monitores da central a partir deste site.',
    sensitive: false,
  ),
  AgentPermissionMeta(
    key: 'discovery',
    label: 'Descoberta',
    description: 'Varrer a rede deste site.',
    sensitive: false,
  ),
  AgentPermissionMeta(
    key: 'device_io',
    label: 'Acesso a equipamentos (plugins)',
    description: 'SSH/HTTP/Telnet nos equipamentos da rede local.',
    sensitive: false,
  ),
  AgentPermissionMeta(
    key: 'database',
    label: 'Ponte de banco (backup)',
    description:
        'Backup dos bancos que este host alcança, inclusive os dos containers.',
    sensitive: true,
  ),
  AgentPermissionMeta(
    key: 'volume_export',
    label: 'Exportar volumes',
    description: 'Baixar o conteúdo de um volume.',
    sensitive: true,
  ),
  AgentPermissionMeta(
    key: 'throughput',
    label: 'Teste de vazão (enlace)',
    description: 'iperf3/btest até um equipamento.',
    sensitive: true,
  ),
  AgentPermissionMeta(
    key: 'deploy',
    label: 'Deploy contínuo',
    description: 'Atualizar stacks a partir do Git.',
    sensitive: true,
  ),
  AgentPermissionMeta(
    key: 'build',
    label: 'Compilar imagens',
    description: 'Compilar imagens no builder dedicado.',
    sensitive: true,
  ),
];

const String defaultDatabaseTargets =
    '172.16.0.0/12:5432,172.16.0.0/12:3306,192.168.0.0/16:5432,192.168.0.0/16:3306';

InstallPolicyView defaultInstallPolicy() => InstallPolicyView(
  allow: agentPermissionsCatalog.map((e) => e.key).toList(growable: false),
  databaseTargets: defaultDatabaseTargets,
);

AgentPermissionMeta permissionMeta(String key) {
  for (final item in agentPermissionsCatalog) {
    if (item.key == key) return item;
  }
  return AgentPermissionMeta(
    key: key,
    label: key,
    description: '',
    sensitive: false,
  );
}

SemanticTone permissionTone(String key) =>
    permissionMeta(key).sensitive ? SemanticTone.warning : SemanticTone.primary;

StatusVisual agentStatusVisual(AgentView agent) {
  if (agent.connected || agent.status == 'online') {
    return const StatusVisual(
      label: 'Conectado',
      tone: SemanticTone.success,
      icon: Icons.check_circle_outline,
    );
  }
  return switch (agent.status) {
    'pending' => const StatusVisual(
      label: 'Aguardando instalação',
      tone: SemanticTone.info,
      icon: Icons.hourglass_top,
    ),
    'revoked' => const StatusVisual(
      label: 'Revogado',
      tone: SemanticTone.secondary,
      icon: Icons.block,
    ),
    _ => const StatusVisual(
      label: 'Desconectado',
      tone: SemanticTone.error,
      icon: Icons.cloud_off,
    ),
  };
}

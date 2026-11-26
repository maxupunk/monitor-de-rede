import 'package:flutter/foundation.dart';

/// Catálogo de permissões `AGENT_ALLOW` (ADR 011 / ADR 013): os mesmos
/// identificadores do `Permission` do backend. É do Núcleo porque o agente
/// é do Núcleo — Servidores e VPN montam a política com ele.
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

/// Todas as permissões do catálogo.
List<String> allAgentPermissionKeys() =>
    agentPermissionsCatalog.map((e) => e.key).toList(growable: false);

/// A ponte de banco liberada sem destino — o mesmo que o backend recusa
/// (`InstallPolicy::new`): `AGENT_DATABASE_TARGETS` é obrigatória no agente
/// (ADR 013) e, vazia, a permissão não alcança banco nenhum.
String? missingDatabaseTargetsError(List<String> allow, String targets) {
  if (!allow.contains('database')) return null;
  final hasTarget = targets.split(',').any((item) => item.trim().isNotEmpty);
  return hasTarget
      ? null
      : 'Informe ao menos um destino (IP:porta ou faixa:porta) ou desmarque '
            'a ponte de banco.';
}

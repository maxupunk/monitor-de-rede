import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ---------------------------------------------------------------------------
// 1. Agent Permissions & Transport Security (ADR 011 / ADR 013)
// ---------------------------------------------------------------------------

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

/// Verifica se o host pertence a rede privada/local (RFC1918, loopback, CGNAT, ULA/link-local).
bool isPrivateHost(String host) {
  final h = host.trim().toLowerCase().replaceAll(RegExp(r'^\[|\]$'), '');
  if (h.isEmpty) return true;
  if (h == 'localhost' ||
      h == '::1' ||
      h.endsWith('.local') ||
      h.endsWith('.internal')) {
    return true;
  }
  if (h.startsWith('127.') ||
      h.startsWith('10.') ||
      h.startsWith('192.168.') ||
      h.startsWith('169.254.') ||
      h.startsWith('fc') ||
      h.startsWith('fd') ||
      h.startsWith('fe80:')) {
    return true;
  }
  final rfc172 = RegExp(r'^172\.(1[6-9]|2\d|3[01])\.').hasMatch(h);
  if (rfc172) return true;
  final cgnat = RegExp(r'^100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.').hasMatch(h);
  return cgnat;
}

bool looksPrivateUrl(String rawUrl) {
  final uri = Uri.tryParse(rawUrl.trim());
  if (uri == null || uri.host.isEmpty) return false;
  return isPrivateHost(uri.host);
}

// ---------------------------------------------------------------------------
// 2. Docker Presentation & Strict `com.netmonitor.protected` Enforcement
// ---------------------------------------------------------------------------

@immutable
class StatusVisual {
  const StatusVisual({
    required this.label,
    required this.tone,
    required this.icon,
    this.live = false,
  });

  final String label;
  final SemanticTone tone;
  final IconData icon;
  final bool live;
}

StatusVisual containerStateMeta(String state) {
  return switch (state.toLowerCase()) {
    'running' => const StatusVisual(
      label: 'Executando',
      tone: SemanticTone.success,
      icon: Icons.play_circle_outline,
    ),
    'exited' => const StatusVisual(
      label: 'Parado',
      tone: SemanticTone.warning,
      icon: Icons.stop_circle_outlined,
    ),
    'created' => const StatusVisual(
      label: 'Criado',
      tone: SemanticTone.info,
      icon: Icons.add_circle_outline,
    ),
    'paused' => const StatusVisual(
      label: 'Pausado',
      tone: SemanticTone.warning,
      icon: Icons.pause_circle_outline,
    ),
    'restarting' => const StatusVisual(
      label: 'Reiniciando',
      tone: SemanticTone.info,
      icon: Icons.restart_alt,
    ),
    'removing' => const StatusVisual(
      label: 'Removendo',
      tone: SemanticTone.error,
      icon: Icons.delete_outline,
    ),
    'dead' => const StatusVisual(
      label: 'Inativo',
      tone: SemanticTone.error,
      icon: Icons.error_outline,
    ),
    _ => StatusVisual(
      label: state.isEmpty ? 'Desconhecido' : state,
      tone: SemanticTone.warning,
      icon: Icons.help_outline,
    ),
  };
}

String shortDockerId(String id) {
  final clean = id.startsWith('sha256:') ? id.substring(7) : id;
  return clean.length > 12 ? clean.substring(0, 12) : clean;
}

String containerDisplayName(DockerContainerSummary container) {
  if (container.names.isNotEmpty) {
    final first = container.names.first.replaceFirst(RegExp(r'^/'), '').trim();
    if (first.isNotEmpty) return first;
  }
  return shortDockerId(container.id);
}

enum ContainerStateFilter { all, running, stopped }

Map<ContainerStateFilter, int> containerFilterCounts(
  List<DockerContainerSummary> containers,
) {
  final running = containers.where((c) => c.state == 'running').length;
  return <ContainerStateFilter, int>{
    ContainerStateFilter.all: containers.length,
    ContainerStateFilter.running: running,
    ContainerStateFilter.stopped: containers.length - running,
  };
}

List<DockerContainerSummary> filterDockerContainers(
  List<DockerContainerSummary> containers,
  String search,
  ContainerStateFilter filter,
) {
  final term = search.trim().toLowerCase();
  return containers
      .where((c) {
        final stateOk = switch (filter) {
          ContainerStateFilter.all => true,
          ContainerStateFilter.running => c.state == 'running',
          ContainerStateFilter.stopped => c.state != 'running',
        };
        if (!stateOk) return false;
        if (term.isEmpty) return true;
        final haystack =
            '${containerDisplayName(c)} ${c.image} ${c.projectName ?? ''}'
                .toLowerCase();
        return haystack.contains(term);
      })
      .toList(growable: false);
}

@immutable
class ContainerGroup {
  const ContainerGroup({
    required this.key,
    required this.label,
    required this.containers,
    required this.running,
    required this.cpuPercent,
    required this.memoryBytes,
    required this.standalone,
  });

  final String key;
  final String label;
  final List<DockerContainerSummary> containers;
  final int running;
  final double cpuPercent;
  final int memoryBytes;
  final bool standalone;
}

List<ContainerGroup> groupDockerContainers(
  List<DockerContainerSummary> containers,
  DockerContainerMetrics? Function(String containerId) metricFor,
) {
  final map = <String, List<DockerContainerSummary>>{};
  final labels = <String, String>{};
  for (final c in containers) {
    final project = c.projectName?.trim() ?? '';
    final key = project.isEmpty ? '__standalone__' : project.toLowerCase();
    map.putIfAbsent(key, () => <DockerContainerSummary>[]).add(c);
    labels[key] = project.isEmpty ? 'Containers avulsos' : project;
  }

  final groups = map.entries.map((entry) {
    final list = List<DockerContainerSummary>.from(entry.value)
      ..sort(
        (a, b) => containerDisplayName(a).compareTo(containerDisplayName(b)),
      );
    var running = 0;
    var cpu = 0.0;
    var mem = 0;
    for (final c in list) {
      if (c.state == 'running') running++;
      final m = metricFor(c.id);
      if (m != null) {
        cpu += m.cpu.usagePercent;
        mem += m.memory.usageBytes;
      }
    }
    return ContainerGroup(
      key: entry.key,
      label: labels[entry.key]!,
      containers: List<DockerContainerSummary>.unmodifiable(list),
      running: running,
      cpuPercent: cpu,
      memoryBytes: mem,
      standalone: entry.key == '__standalone__',
    );
  }).toList();

  groups.sort((a, b) {
    if (a.standalone != b.standalone) return a.standalone ? 1 : -1;
    return a.label.compareTo(b.label);
  });
  return groups;
}

enum DockerContainerActionKey { start, stop, restart, update, remove }

@immutable
class DockerContainerActionMeta {
  const DockerContainerActionMeta({
    required this.key,
    required this.label,
    required this.hint,
    required this.tone,
    required this.icon,
    required this.requiresConfirm,
  });

  final DockerContainerActionKey key;
  final String label;
  final String hint;
  final SemanticTone tone;
  final IconData icon;
  final bool requiresConfirm;
}

const Map<DockerContainerActionKey, DockerContainerActionMeta>
dockerActionCatalog = <DockerContainerActionKey, DockerContainerActionMeta>{
  DockerContainerActionKey.start: DockerContainerActionMeta(
    key: DockerContainerActionKey.start,
    label: 'Iniciar',
    hint: 'Liga o container parado com a mesma configuração.',
    tone: SemanticTone.success,
    icon: Icons.play_arrow,
    requiresConfirm: false,
  ),
  DockerContainerActionKey.stop: DockerContainerActionMeta(
    key: DockerContainerActionKey.stop,
    label: 'Parar',
    hint: 'Desliga o container sem removê-lo.',
    tone: SemanticTone.warning,
    icon: Icons.stop,
    requiresConfirm: true,
  ),
  DockerContainerActionKey.restart: DockerContainerActionMeta(
    key: DockerContainerActionKey.restart,
    label: 'Reiniciar',
    hint: 'Para e liga de novo, sem recriar.',
    tone: SemanticTone.info,
    icon: Icons.restart_alt,
    requiresConfirm: false,
  ),
  DockerContainerActionKey.update: DockerContainerActionMeta(
    key: DockerContainerActionKey.update,
    label: 'Atualizar imagem',
    hint:
        'Baixa a versão atual da imagem e recria o container com a mesma configuração.',
    tone: SemanticTone.primary,
    icon: Icons.system_update_alt,
    requiresConfirm: true,
  ),
  DockerContainerActionKey.remove: DockerContainerActionMeta(
    key: DockerContainerActionKey.remove,
    label: 'Remover',
    hint: 'Apaga o container. Os volumes ficam.',
    tone: SemanticTone.error,
    icon: Icons.delete_outline,
    requiresConfirm: true,
  ),
};

/// Calcula as ações permitidas respeitando ADR 014 (`com.netmonitor.protected=true`):
/// Containers protegidos NUNCA permitem stop, restart, update ou remove — somente start se parado!
List<DockerContainerActionMeta> allowedContainerActions({
  required DockerContainerSummary container,
  required bool canLifecycle,
  required bool canUpdate,
}) {
  final running = container.state == 'running';
  final isProtected = container.isProtected;
  final keys = <DockerContainerActionKey>[];
  if (canLifecycle && !running) keys.add(DockerContainerActionKey.start);
  if (canLifecycle && running && !isProtected) {
    keys.add(DockerContainerActionKey.stop);
  }
  if (canLifecycle && !isProtected) keys.add(DockerContainerActionKey.restart);
  if (canUpdate && !isProtected) keys.add(DockerContainerActionKey.update);
  if (canLifecycle && !isProtected) keys.add(DockerContainerActionKey.remove);
  return keys.map((k) => dockerActionCatalog[k]!).toList(growable: false);
}

// ---------------------------------------------------------------------------
// 3. Continuous Deploys & Builds Presentation (ADR 021 / ADR 023)
// ---------------------------------------------------------------------------

StatusVisual deployRunStatusMeta(String status) {
  return switch (status) {
    'awaiting_approval' => const StatusVisual(
      label: 'Aguarda aprovação',
      tone: SemanticTone.warning,
      icon: Icons.pending_actions,
    ),
    'scheduled' => const StatusVisual(
      label: 'Agendado',
      tone: SemanticTone.info,
      icon: Icons.schedule,
    ),
    'queued' => const StatusVisual(
      label: 'Na fila',
      tone: SemanticTone.info,
      icon: Icons.hourglass_top,
      live: true,
    ),
    'running' => const StatusVisual(
      label: 'Publicando',
      tone: SemanticTone.primary,
      icon: Icons.rocket_launch,
      live: true,
    ),
    'succeeded' => const StatusVisual(
      label: 'No ar',
      tone: SemanticTone.success,
      icon: Icons.check_circle_outline,
    ),
    'failed' => const StatusVisual(
      label: 'Falhou',
      tone: SemanticTone.error,
      icon: Icons.error_outline,
    ),
    'rolled_back' => const StatusVisual(
      label: 'Revertido',
      tone: SemanticTone.warning,
      icon: Icons.settings_backup_restore,
    ),
    'superseded' => const StatusVisual(
      label: 'Substituído',
      tone: SemanticTone.secondary,
      icon: Icons.skip_next,
    ),
    'cancelled' => const StatusVisual(
      label: 'Cancelado',
      tone: SemanticTone.secondary,
      icon: Icons.cancel_outlined,
    ),
    'rejected' => const StatusVisual(
      label: 'Recusado',
      tone: SemanticTone.error,
      icon: Icons.block,
    ),
    _ => StatusVisual(
      label: status,
      tone: SemanticTone.secondary,
      icon: Icons.help_outline,
    ),
  };
}

bool isDeployRunPending(String status) =>
    status == 'awaiting_approval' ||
    status == 'scheduled' ||
    status == 'queued';

bool isDeployRunFinished(String status) =>
    !isDeployRunPending(status) && status != 'running';

StatusVisual deployBuildStatusMeta(String status) {
  return switch (status) {
    'queued' => const StatusVisual(
      label: 'Na fila',
      tone: SemanticTone.info,
      icon: Icons.hourglass_top,
      live: true,
    ),
    'running' => const StatusVisual(
      label: 'Compilando',
      tone: SemanticTone.primary,
      icon: Icons.build_outlined,
      live: true,
    ),
    'succeeded' => const StatusVisual(
      label: 'Compilada',
      tone: SemanticTone.success,
      icon: Icons.check_circle_outline,
    ),
    'failed' => const StatusVisual(
      label: 'Falhou',
      tone: SemanticTone.error,
      icon: Icons.error_outline,
    ),
    'cancelled' => const StatusVisual(
      label: 'Cancelada',
      tone: SemanticTone.secondary,
      icon: Icons.cancel_outlined,
    ),
    'superseded' => const StatusVisual(
      label: 'Substituída',
      tone: SemanticTone.secondary,
      icon: Icons.skip_next,
    ),
    _ => StatusVisual(
      label: status,
      tone: SemanticTone.secondary,
      icon: Icons.help_outline,
    ),
  };
}

const List<({String key, String label, IconData icon})> deployStagesOrder =
    <({String key, String label, IconData icon})>[
      (key: 'backup', label: 'Backup do banco', icon: Icons.backup_outlined),
      (
        key: 'image',
        label: 'Conferir e enviar imagem',
        icon: Icons.verified_user_outlined,
      ),
      (key: 'sync', label: 'Repositório', icon: Icons.sync),
      (key: 'env', label: 'Variáveis', icon: Icons.vpn_key_outlined),
      (key: 'pull', label: 'Baixar imagens', icon: Icons.download_outlined),
      (
        key: 'start',
        label: 'Subir e conferir saúde',
        icon: Icons.monitor_heart_outlined,
      ),
      (
        key: 'prune',
        label: 'Limpar imagens antigas',
        icon: Icons.cleaning_services_outlined,
      ),
      (
        key: 'rollback',
        label: 'Voltar versão anterior',
        icon: Icons.settings_backup_restore,
      ),
    ];

const List<({String key, String label, IconData icon})> buildStagesOrder =
    <({String key, String label, IconData icon})>[
      (
        key: 'prepare',
        label: 'Preparar o builder',
        icon: Icons.settings_suggest_outlined,
      ),
      (
        key: 'clone',
        label: 'Buscar o commit',
        icon: Icons.account_tree_outlined,
      ),
      (key: 'test', label: 'Testes', icon: Icons.science_outlined),
      (key: 'build', label: 'Compilar', icon: Icons.build_outlined),
      (
        key: 'store',
        label: 'Guardar no registry',
        icon: Icons.inventory_2_outlined,
      ),
      (key: 'scan', label: 'Vulnerabilidades', icon: Icons.shield_outlined),
    ];

Map<String, String> computeDeployStageStates(String? stage, String status) {
  final order = deployStagesOrder
      .where((e) => e.key != 'rollback')
      .map((e) => e.key)
      .toList(growable: false);
  final result = <String, String>{
    for (final s in deployStagesOrder) s.key: 'todo',
  };
  if (status == 'succeeded') {
    for (final k in order) {
      result[k] = 'done';
    }
    return result;
  }
  if (status == 'rolled_back') {
    for (final k in order) {
      result[k] = 'done';
    }
    result['start'] = 'failed';
    result['prune'] = 'todo';
    result['rollback'] = 'done';
    return result;
  }
  if (stage == null || stage.isEmpty) return result;
  if (stage == 'rollback') {
    for (final k in order) {
      result[k] = 'done';
    }
    result['start'] = 'failed';
    result['prune'] = 'todo';
    result['rollback'] = status == 'failed' ? 'failed' : 'current';
    return result;
  }
  final idx = order.indexOf(stage);
  for (var i = 0; i < order.length; i++) {
    if (i < idx) {
      result[order[i]] = 'done';
    } else if (i == idx) {
      result[order[i]] = status == 'failed' ? 'failed' : 'current';
    }
  }
  return result;
}

Map<String, String> computeBuildStageStates(String? stage, String status) {
  final order = buildStagesOrder.map((e) => e.key).toList(growable: false);
  final result = <String, String>{for (final k in order) k: 'todo'};
  if (status == 'succeeded') {
    for (final k in order) {
      result[k] = 'done';
    }
    return result;
  }
  final stopped = status == 'failed' || status == 'cancelled';
  final idx = (stage != null && stage.isNotEmpty)
      ? order.indexOf(stage)
      : (stopped ? 0 : -1);
  for (var i = 0; i < order.length; i++) {
    if (i < idx) {
      result[order[i]] = 'done';
    } else if (i == idx) {
      result[order[i]] = stopped ? 'failed' : 'current';
    }
  }
  return result;
}

String refShortName(String? reference) =>
    (reference ?? '').replaceFirst(RegExp(r'^refs/(heads|tags)/'), '');

String commitUrl(String repository, String sha) =>
    'https://github.com/$repository/commit/$sha';

String commitFirstLine(String? message) =>
    (message ?? '').split('\n').first.trim();

String deployWindowLabel(String? start, String? end) =>
    (start != null && start.isNotEmpty && end != null && end.isNotEmpty)
    ? '$start–$end'
    : '';

// ---------------------------------------------------------------------------
// 4. Lossless `.env` Parser & Serializer (envFile.ts parity)
// ---------------------------------------------------------------------------

final RegExp _envKeyPattern = RegExp(r'^[A-Za-z_][A-Za-z0-9_]*$');
final RegExp _envSecretPattern = RegExp(
  r'(PASS|SECRET|TOKEN|KEY|PRIVATE|CREDENTIAL|AUTH)',
  caseSensitive: false,
);

@immutable
class EnvLine {
  const EnvLine.variable({
    required this.key,
    required this.value,
    this.exported = false,
  }) : isVariable = true,
       rawText = '';

  const EnvLine.other(this.rawText)
    : isVariable = false,
      key = '',
      value = '',
      exported = false;

  final bool isVariable;
  final String key;
  final String value;
  final bool exported;
  final String rawText;
}

List<EnvLine> parseEnvFile(String content) {
  if (content.isEmpty) return const <EnvLine>[];
  final lines = content.replaceAll('\r\n', '\n').split('\n');
  if (lines.isNotEmpty && lines.last.isEmpty) {
    lines.removeLast();
  }
  return lines
      .map((text) {
        final trimmed = text.trim();
        if (trimmed.isEmpty || trimmed.startsWith('#')) {
          return EnvLine.other(text);
        }
        final exported = trimmed.startsWith('export ');
        final body = exported ? trimmed.substring('export '.length) : trimmed;
        final eq = body.indexOf('=');
        final key = eq > 0 ? body.substring(0, eq).trim() : '';
        if (!_envKeyPattern.hasMatch(key)) {
          return EnvLine.other(text);
        }
        return EnvLine.variable(
          key: key,
          value: body.substring(eq + 1),
          exported: exported,
        );
      })
      .toList(growable: false);
}

String serializeEnvFile(List<EnvLine> lines) {
  if (lines.isEmpty) return '';
  final rendered = lines
      .map((line) {
        if (!line.isVariable) return line.rawText;
        return '${line.exported ? 'export ' : ''}${line.key}=${line.value}';
      })
      .join('\n');
  return '$rendered\n';
}

bool isValidEnvKey(String key) => _envKeyPattern.hasMatch(key);
bool looksLikeSecretKey(String key) => _envSecretPattern.hasMatch(key);

// ---------------------------------------------------------------------------
// 5. Registry Presentation (ADR 022 / ADR 023)
// ---------------------------------------------------------------------------

bool isCommitTag(String tag) =>
    RegExp(r'^[0-9a-f]{40}$', caseSensitive: false).hasMatch(tag);

String shortRegistryTag(String tag) =>
    isCommitTag(tag) ? tag.substring(0, 7) : tag;

String shortRegistryDigest(String digest) =>
    digest.length > 19 ? digest.substring(0, 19) : digest;

String registryHostFromUrl(String baseUrl) {
  final uri = Uri.tryParse(baseUrl);
  if (uri == null || uri.host.isEmpty) return baseUrl;
  final hasExplicitPort =
      uri.hasPort &&
      !((uri.scheme == 'http' && uri.port == 80) ||
          (uri.scheme == 'https' && uri.port == 443));
  return hasExplicitPort ? '${uri.host}:${uri.port}' : uri.host;
}

String registryDaemonJson(String host) =>
    const JsonEncoder.withIndent('  ').convert(<String, dynamic>{
      'insecure-registries': <String>[host],
    });

String registryScanSentence(RegistryScanSummary? scan) {
  if (scan == null) return 'Ainda não analisada';
  switch (scan.status) {
    case 'pending':
      return 'Análise em andamento';
    case 'no_sbom':
      return 'Sem lista de pacotes para analisar';
    case 'failed':
      return 'A base de vulnerabilidades não respondeu';
  }
  final serious = scan.critical + scan.high;
  if (serious == 0 && scan.medium + scan.low == 0) {
    return 'Nenhuma vulnerabilidade conhecida';
  }
  if (serious == 0) {
    final minor = scan.medium + scan.low;
    return '${countNoun(minor, 'vulnerabilidade')} ${minor == 1 ? 'média ou baixa' : 'médias ou baixas'}';
  }
  final parts = <String>[
    if (scan.critical > 0) countNoun(scan.critical, 'crítica'),
    if (scan.high > 0) countNoun(scan.high, 'alta'),
  ];
  return parts.join(' e ');
}

SemanticTone registrySeverityTone(String severity) {
  return switch (severity.toLowerCase()) {
    'critical' => SemanticTone.error,
    'high' => SemanticTone.caution,
    'medium' => SemanticTone.warning,
    'low' => SemanticTone.info,
    _ => SemanticTone.secondary,
  };
}

String registrySeverityLabel(String severity) {
  return switch (severity.toLowerCase()) {
    'critical' => 'Crítica',
    'high' => 'Alta',
    'medium' => 'Média',
    'low' => 'Baixa',
    _ => 'Sem nota',
  };
}

SemanticTone registryScanTone(RegistryScanSummary? scan) {
  if (scan == null || scan.status != 'done') return SemanticTone.secondary;
  if (scan.critical > 0) return SemanticTone.error;
  if (scan.high > 0) return SemanticTone.caution;
  if (scan.medium > 0) return SemanticTone.warning;
  return SemanticTone.success;
}

String registryActionLabel(String action) {
  return switch (action) {
    'pull' => 'baixar',
    'push' => 'enviar',
    'delete' => 'apagar',
    _ => action,
  };
}

String registryPlatformsLabel(List<String> platforms) {
  if (platforms.length <= 2) return platforms.join(', ');
  return '${platforms.take(2).join(', ')} e mais ${platforms.length - 2}';
}

// ---------------------------------------------------------------------------
// 6. Storage Providers Presentation
// ---------------------------------------------------------------------------

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

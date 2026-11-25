import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/utils/status_visual.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Apresentação Docker, agrupamento por projeto e imposição estrita de
/// `com.netmonitor.protected=true` (ADR 014).

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

const String _composeProjectLabel = 'com.docker.compose.project';
const String _composeWorkingDirLabel = 'com.docker.compose.project.working_dir';
const String _composeConfigFilesLabel =
    'com.docker.compose.project.config_files';
const String _composeServiceLabel = 'com.docker.compose.service';

/// Projetos compose dos containers ao vivo (o snapshot do SSE), pela mesma
/// regra de `compose::projects` da central: as labels que o CLI grava, e só
/// projetos com diretório de trabalho conhecido — sem ele o agente não tem
/// onde rodar o `docker compose`. Lidos do estado ao vivo, os cartões mudam
/// junto com os containers, sem nova consulta.
List<ComposeProject> composeProjectsOf(
  List<DockerContainerSummary> containers,
) {
  final byName = <String, _ComposeDraft>{};
  for (final container in containers) {
    final name = container.labels[_composeProjectLabel] ?? '';
    final workingDir = container.labels[_composeWorkingDirLabel] ?? '';
    if (name.isEmpty || workingDir.isEmpty) continue;
    final draft = byName.putIfAbsent(
      name,
      () => _ComposeDraft(
        workingDir,
        (container.labels[_composeConfigFilesLabel] ?? '')
            .split(',')
            .map((file) => file.trim())
            .where((file) => file.isNotEmpty)
            .toList(growable: false),
      ),
    );
    final running = container.state == 'running';
    draft.protected = draft.protected || container.isProtected;
    draft.containers++;
    if (running) draft.running++;
    final service = container.labels[_composeServiceLabel];
    if (service != null) {
      final counts = draft.services.putIfAbsent(service, () => <int>[0, 0]);
      counts[0]++;
      if (running) counts[1]++;
    }
  }
  final names = byName.keys.toList()..sort();
  return List<ComposeProject>.unmodifiable(
    names.map((name) {
      final draft = byName[name]!;
      final services = draft.services.keys.toList()..sort();
      return ComposeProject(
        name: name,
        workingDir: draft.workingDir,
        configFiles: List<String>.unmodifiable(draft.configFiles),
        services: List<ComposeService>.unmodifiable(
          services.map(
            (service) => ComposeService(
              name: service,
              containers: draft.services[service]![0],
              running: draft.services[service]![1],
            ),
          ),
        ),
        containers: draft.containers,
        running: draft.running,
        protected: draft.protected,
      );
    }),
  );
}

class _ComposeDraft {
  _ComposeDraft(this.workingDir, this.configFiles);

  final String workingDir;
  final List<String> configFiles;
  final Map<String, List<int>> services = <String, List<int>>{};
  int containers = 0;
  int running = 0;
  bool protected = false;
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

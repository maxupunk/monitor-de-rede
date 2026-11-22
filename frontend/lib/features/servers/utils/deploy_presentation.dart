import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/servers/utils/status_visual.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Apresentação e transições de estágio para Deploys Contínuos e Compilações (ADR 021 / ADR 023).

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

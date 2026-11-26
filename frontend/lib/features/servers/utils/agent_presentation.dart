import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/core/models/agent_permission_models.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/utils/status_visual.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/core/models/agent_permission_models.dart';

InstallPolicyView defaultInstallPolicy() => InstallPolicyView(
  allow: allAgentPermissionKeys(),
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

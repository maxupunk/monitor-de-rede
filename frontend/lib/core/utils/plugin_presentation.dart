import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Cor semântica fixa para tudo que vem de plugin (AGENTS.md §12).
/// NÃO trocar por `primary` nem reaproveitar em outros domínios.
const SemanticTone pluginSemanticTone = SemanticTone.plugin;

/// Apresentação visual padronizada (rótulo, tom semântico e ícone).
@immutable
class SemanticPresentation {
  const SemanticPresentation({
    required this.label,
    required this.tone,
    required this.icon,
  });

  final String label;
  final SemanticTone tone;
  final IconData icon;
}

const SemanticPresentation _fallbackPresentation = SemanticPresentation(
  label: '—',
  tone: SemanticTone.secondary,
  icon: Icons.help_outline,
);

const Map<String, SemanticPresentation> _compatMap =
    <String, SemanticPresentation>{
      'validated': SemanticPresentation(
        label: 'Validado',
        tone: SemanticTone.success,
        icon: Icons.verified_outlined,
      ),
      'likely': SemanticPresentation(
        label: 'Compatível',
        tone: SemanticTone.info,
        icon: Icons.check_circle_outline,
      ),
      'possible': SemanticPresentation(
        label: 'Talvez compatível',
        tone: SemanticTone.warning,
        icon: Icons.help_outline,
      ),
      'incompatible': SemanticPresentation(
        label: 'Incompatível',
        tone: SemanticTone.error,
        icon: Icons.cancel_outlined,
      ),
    };

const Map<String, SemanticPresentation> _statusMap =
    <String, SemanticPresentation>{
      'active': SemanticPresentation(
        label: 'Ativo',
        tone: SemanticTone.success,
        icon: Icons.power_outlined,
      ),
      'tested': SemanticPresentation(
        label: 'Testado',
        tone: SemanticTone.info,
        icon: Icons.science_outlined,
      ),
      'draft': SemanticPresentation(
        label: 'Rascunho',
        tone: SemanticTone.warning,
        icon: Icons.edit_outlined,
      ),
      'quarantine': SemanticPresentation(
        label: 'Quarentena',
        tone: SemanticTone.error,
        icon: Icons.gpp_maybe_outlined,
      ),
      'disabled': SemanticPresentation(
        label: 'Desativado',
        tone: SemanticTone.secondary,
        icon: Icons.power_off_outlined,
      ),
    };

const Map<String, SemanticPresentation> _sourceMap =
    <String, SemanticPresentation>{
      'builtin': SemanticPresentation(
        label: 'Embutido',
        tone: SemanticTone.primary,
        icon: Icons.inventory_2_outlined,
      ),
      'user': SemanticPresentation(
        label: 'Próprio',
        tone: SemanticTone.secondary,
        icon: Icons.person_outline,
      ),
      'ai': SemanticPresentation(
        label: 'Criado pela IA',
        tone: SemanticTone.info,
        icon: Icons.smart_toy_outlined,
      ),
      'imported': SemanticPresentation(
        label: 'Importado',
        tone: SemanticTone.warning,
        icon: Icons.file_download_outlined,
      ),
    };

const Map<String, SemanticPresentation> _effectMap =
    <String, SemanticPresentation>{
      'read': SemanticPresentation(
        label: 'Leitura',
        tone: SemanticTone.info,
        icon: Icons.visibility_outlined,
      ),
      'write': SemanticPresentation(
        label: 'Altera o equipamento',
        tone: SemanticTone.error,
        icon: Icons.edit_notifications_outlined,
      ),
    };

const Map<String, SemanticPresentation> _severityMap =
    <String, SemanticPresentation>{
      'info': SemanticPresentation(
        label: 'Informativo',
        tone: SemanticTone.info,
        icon: Icons.info_outline,
      ),
      'low': SemanticPresentation(
        label: 'Baixo',
        tone: SemanticTone.success,
        icon: Icons.verified_user_outlined,
      ),
      'medium': SemanticPresentation(
        label: 'Médio',
        tone: SemanticTone.warning,
        icon: Icons.shield_outlined,
      ),
      'high': SemanticPresentation(
        label: 'Alto',
        tone: SemanticTone.error,
        icon: Icons.gpp_maybe_outlined,
      ),
      'critical': SemanticPresentation(
        label: 'Crítico',
        tone: SemanticTone.error,
        icon: Icons.dangerous_outlined,
      ),
    };

const Map<String, SemanticPresentation> _runStatusMap =
    <String, SemanticPresentation>{
      'running': SemanticPresentation(
        label: 'Executando',
        tone: SemanticTone.primary,
        icon: Icons.pending_outlined,
      ),
      'succeeded': SemanticPresentation(
        label: 'Concluído',
        tone: SemanticTone.success,
        icon: Icons.check_circle_outline,
      ),
      'failed': SemanticPresentation(
        label: 'Falhou',
        tone: SemanticTone.error,
        icon: Icons.error_outline,
      ),
      'cancelled': SemanticPresentation(
        label: 'Cancelado',
        tone: SemanticTone.warning,
        icon: Icons.cancel_outlined,
      ),
    };

const Map<String, SemanticPresentation> _batchStatusMap =
    <String, SemanticPresentation>{
      'running': SemanticPresentation(
        label: 'Em andamento',
        tone: SemanticTone.primary,
        icon: Icons.pending_outlined,
      ),
      'succeeded': SemanticPresentation(
        label: 'Concluído',
        tone: SemanticTone.success,
        icon: Icons.done_all,
      ),
      'partial': SemanticPresentation(
        label: 'Parcial',
        tone: SemanticTone.warning,
        icon: Icons.warning_amber_outlined,
      ),
      'failed': SemanticPresentation(
        label: 'Falhou',
        tone: SemanticTone.error,
        icon: Icons.error_outline,
      ),
      'cancelled': SemanticPresentation(
        label: 'Cancelado',
        tone: SemanticTone.warning,
        icon: Icons.cancel_outlined,
      ),
      'pending': SemanticPresentation(
        label: 'Na fila',
        tone: SemanticTone.secondary,
        icon: Icons.hourglass_empty,
      ),
      'skipped': SemanticPresentation(
        label: 'Pulado',
        tone: SemanticTone.warning,
        icon: Icons.skip_next_outlined,
      ),
    };

const Map<String, String> _originMap = <String, String>{
  'user': 'Operador',
  'ai': 'IA',
  'validation': 'Validação',
  'fleet': 'Frota',
  'link_test': 'Teste de velocidade',
  'wifi_watch': 'Vigia de Wi-Fi',
  'wifi_map': 'Mapa Wi-Fi',
};

const Map<String, SemanticTone> _stateTones = <String, SemanticTone>{
  'ok': SemanticTone.success,
  'sincronizado': SemanticTone.success,
  'ativo': SemanticTone.success,
  'online': SemanticTone.success,
  'no_ar': SemanticTone.success,
  'ativa': SemanticTone.success,
  'criar': SemanticTone.success,
  'manter': SemanticTone.success,
  'divergente': SemanticTone.warning,
  'pendente': SemanticTone.warning,
  'sem_radio': SemanticTone.warning,
  'fora_do_ar': SemanticTone.warning,
  'alterar': SemanticTone.warning,
  'desativada': SemanticTone.secondary,
  'desligado': SemanticTone.secondary,
  'desativado': SemanticTone.secondary,
  'ausente': SemanticTone.error,
  'falhou': SemanticTone.error,
  'offline': SemanticTone.error,
  'remover': SemanticTone.error,
};

SemanticPresentation compatPresentation(String value) =>
    _compatMap[value] ?? _fallbackPresentation;

SemanticPresentation pluginStatusPresentation(String value) =>
    _statusMap[value] ??
    SemanticPresentation(
      label: value,
      tone: SemanticTone.secondary,
      icon: Icons.help_outline,
    );

SemanticPresentation pluginSourcePresentation(String value) =>
    _sourceMap[value] ??
    SemanticPresentation(
      label: value,
      tone: SemanticTone.secondary,
      icon: Icons.help_outline,
    );

SemanticPresentation effectPresentation(String value) =>
    _effectMap[value] ?? _fallbackPresentation;

SemanticPresentation severityPresentation(String value) =>
    _severityMap[value] ?? _fallbackPresentation;

SemanticPresentation runStatusPresentation(String value) =>
    _runStatusMap[value] ??
    SemanticPresentation(
      label: value,
      tone: SemanticTone.secondary,
      icon: Icons.help_outline,
    );

SemanticPresentation batchStatusPresentation(String value) =>
    _batchStatusMap[value] ??
    _runStatusMap[value] ??
    SemanticPresentation(
      label: value,
      tone: SemanticTone.secondary,
      icon: Icons.help_outline,
    );

String originLabel(String value) => _originMap[value] ?? value;

SemanticTone pluginStateTone(Object? value) =>
    _stateTones[value?.toString().toLowerCase() ?? ''] ?? SemanticTone.info;

String pluginStateLabel(Object? value) {
  final text = (value?.toString() ?? '').replaceAll('_', ' ');
  if (text.isEmpty) return '';
  return '${text[0].toUpperCase()}${text.substring(1)}';
}

String keyLabel(String key) {
  final text = key.replaceAll('_', ' ');
  if (text.isEmpty) return '';
  return '${text[0].toUpperCase()}${text.substring(1)}';
}

String formatPluginOutputValue(Object? value, String format) {
  final number = value is num
      ? value
      : num.tryParse(value?.toString().trim() ?? '');
  final numeric = number != null && number.isFinite;
  switch (format) {
    case 'bytes':
      return numeric ? formatBytes(number) : '$value';
    case 'bps':
      return numeric ? formatBps(number) : '$value';
    case 'latency':
      return numeric ? formatLatency(number) : '$value';
    case 'percent':
      return numeric ? formatPercent(number) : '$value';
    case 'duration':
      return numeric ? formatElapsedMs(number) : '$value';
    case 'uptime':
      if (!numeric) return '$value';
      final now = DateTime.now();
      return formatTimeSpan(
        now.subtract(Duration(seconds: number.round())),
        now,
      );
    case 'count':
      return numeric ? formatCompactCount(number) : '$value';
    case 'datetime':
      final date = _parsePluginDate(value, number);
      return date != null ? formatDateTime(date) : '$value';
    case 'relative':
      final date = _parsePluginDate(value, number);
      return date != null ? formatRelativeTime(date) : '$value';
    case 'state':
      return pluginStateLabel(value);
    default:
      return '$value';
  }
}

DateTime? _parsePluginDate(Object? value, num? number) {
  if (number != null && number.isFinite) {
    return DateTime.fromMillisecondsSinceEpoch((number * 1000).round());
  }
  if (value is String) return DateTime.tryParse(value);
  return null;
}

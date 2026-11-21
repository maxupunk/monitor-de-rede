import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Indicador visual de status com ponto colorido semântico e texto legível.
class StatusIndicator extends StatelessWidget {
  const StatusIndicator({
    super.key,
    required this.label,
    required this.tone,
    this.dotSize = 10,
    this.compact = false,
  });

  final String label;
  final SemanticTone tone;
  final double dotSize;
  final bool compact;

  factory StatusIndicator.fromStatus(String status, {bool compact = false}) {
    final normalized = status.trim().toLowerCase();
    return switch (normalized) {
      'online' ||
      'up' ||
      'ok' ||
      'healthy' ||
      'running' ||
      'active' ||
      'connected' => StatusIndicator(
        label: _labelFor(normalized),
        tone: SemanticTone.success,
        compact: compact,
      ),
      'degraded' ||
      'warning' ||
      'paused' ||
      'reconnecting' ||
      'partial' => StatusIndicator(
        label: _labelFor(normalized),
        tone: SemanticTone.warning,
        compact: compact,
      ),
      'offline' ||
      'down' ||
      'error' ||
      'failed' ||
      'critical' ||
      'exited' ||
      'dead' => StatusIndicator(
        label: _labelFor(normalized),
        tone: SemanticTone.error,
        compact: compact,
      ),
      'maintenance' || 'silenced' || 'acknowledged' => StatusIndicator(
        label: _labelFor(normalized),
        tone: SemanticTone.info,
        compact: compact,
      ),
      _ => StatusIndicator(
        label: status.isEmpty ? 'Desconhecido' : status,
        tone: SemanticTone.secondary,
        compact: compact,
      ),
    };
  }

  static String _labelFor(String normalized) => switch (normalized) {
    'online' => 'Online',
    'up' => 'Ativo',
    'ok' => 'OK',
    'healthy' => 'Saudável',
    'running' => 'Em execução',
    'active' => 'Ativo',
    'connected' => 'Conectado',
    'degraded' => 'Degradado',
    'warning' => 'Atenção',
    'paused' => 'Pausado',
    'reconnecting' => 'Reconectando',
    'partial' => 'Parcial',
    'offline' => 'Offline',
    'down' => 'Indisponível',
    'error' => 'Erro',
    'failed' => 'Falhou',
    'critical' => 'Crítico',
    'exited' => 'Parado',
    'dead' => 'Inativo',
    'maintenance' => 'Manutenção',
    'silenced' => 'Silenciado',
    'acknowledged' => 'Reconhecido',
    _ => normalized,
  };

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(tone, scheme);

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Container(
          width: dotSize,
          height: dotSize,
          decoration: BoxDecoration(
            color: colors.foreground,
            shape: BoxShape.circle,
            boxShadow: <BoxShadow>[
              BoxShadow(
                color: colors.foreground.withValues(alpha: 0.4),
                blurRadius: 4,
              ),
            ],
          ),
        ),
        const SizedBox(width: 6),
        Flexible(
          child: Text(
            label,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              fontSize: compact ? 12 : 13,
              fontWeight: FontWeight.w600,
              color: scheme.onSurface,
            ),
          ),
        ),
      ],
    );
  }
}

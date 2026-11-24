import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Descritor visual semântico reutilizável para estados de containers, agentes e deploys.
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

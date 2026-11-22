import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const Set<String> unmanagedSwitchTypes = <String>{'unmanaged_switch', 'hub'};

/// Opções canônicas de tipos de equipamento para formulários e filtros.
const List<(String, String)> deviceTypeOptions = <(String, String)>[
  ('router', 'Roteador'),
  ('switch', 'Switch'),
  ('unmanaged_switch', 'Switch não gerenciável'),
  ('access_point', 'Access Point'),
  ('firewall', 'Firewall'),
  ('server', 'Servidor'),
  ('workstation', 'Estação de trabalho'),
  ('nas', 'NAS / Storage'),
  ('ups', 'Nobreak (UPS)'),
  ('printer', 'Impressora'),
  ('camera', 'Câmera'),
  ('voip', 'Telefone VoIP'),
  ('iot', 'IoT'),
  ('media', 'Mídia'),
  ('mobile', 'Dispositivo móvel'),
  ('vm', 'Máquina Virtual'),
  ('web_device', 'Dispositivo web'),
  ('other', 'Outro'),
];

/// Indica se o tipo informado corresponde a um switch não gerenciável ou hub.
bool isUnmanagedSwitchType(String? type) {
  if (type == null) return false;
  return unmanagedSwitchTypes.contains(type.trim().toLowerCase());
}

/// Rótulo legível em PT-BR para qualquer tipo de equipamento do inventário ou topologia.
String deviceTypeLabel(String? type, {bool isSystem = false}) {
  if (isSystem) return 'Servidor NetMonitor';
  final raw = (type ?? '').trim();
  final lower = raw.toLowerCase();
  if (unmanagedSwitchTypes.contains(lower)) return 'Switch não gerenciável';
  return switch (lower) {
    'router' || 'gateway' => 'Roteador',
    'firewall' => 'Firewall',
    'switch' => 'Switch',
    'access_point' || 'ap' => 'Access Point',
    'server' => 'Servidor',
    'nas' => 'NAS / Storage',
    'workstation' || 'desktop' || 'pc' => 'Estação de trabalho',
    'mobile' || 'phone' => 'Dispositivo móvel',
    'printer' => 'Impressora',
    'camera' || 'cctv' => 'Câmera',
    'voip' => 'Telefone VoIP',
    'iot' => 'IoT',
    'media' || 'tv' => 'Mídia',
    'ups' => 'Nobreak (UPS)',
    'vm' => 'Máquina Virtual',
    'web_device' => 'Dispositivo web',
    'other' => 'Outro',
    'unknown' || '' => 'Desconhecido',
    _ => raw,
  };
}

/// Ícone Material padronizado por tipo de equipamento (`Icons.lan_outlined` para
/// `switch` gerenciável e `Icons.hub_outlined` para `unmanaged_switch`/`hub`).
IconData deviceTypeIcon(String? type, {bool isSystem = false}) {
  if (isSystem) return Icons.monitor_heart_outlined;
  final lower = (type ?? '').trim().toLowerCase();
  if (unmanagedSwitchTypes.contains(lower)) return Icons.hub_outlined;
  return switch (lower) {
    'router' || 'gateway' => Icons.router_outlined,
    'firewall' => Icons.security_outlined,
    'switch' => Icons.lan_outlined,
    'access_point' || 'ap' => Icons.wifi_tethering,
    'server' => Icons.dns_outlined,
    'nas' => Icons.storage_outlined,
    'workstation' || 'desktop' || 'pc' => Icons.computer_outlined,
    'mobile' || 'phone' => Icons.smartphone_outlined,
    'printer' => Icons.print_outlined,
    'camera' || 'cctv' => Icons.videocam_outlined,
    'voip' => Icons.phone_in_talk_outlined,
    'iot' => Icons.sensors_outlined,
    'media' || 'tv' => Icons.tv_outlined,
    'ups' => Icons.battery_charging_full_outlined,
    'vm' => Icons.cloud_queue_outlined,
    'web_device' => Icons.language_outlined,
    _ => Icons.devices_other_outlined,
  };
}

/// Tom semântico da qualidade do sinal Wi-Fi (RSSI em dBm):
/// `>= -60` success, `>= -72` info, `>= -80` warning, `< -80` error.
SemanticTone wifiSignalTone(num? dbm) {
  if (dbm == null || !dbm.isFinite) return SemanticTone.secondary;
  if (dbm >= -60) return SemanticTone.success;
  if (dbm >= -72) return SemanticTone.info;
  if (dbm >= -80) return SemanticTone.warning;
  return SemanticTone.error;
}

/// Rótulo localizado em PT-BR para a classificação textual de sinal Wi-Fi.
String wifiSignalGradeLabel(String? grade, {String fallback = '—'}) {
  final raw = (grade ?? '').trim();
  if (raw.isEmpty) return fallback;
  return switch (raw.toLowerCase()) {
    'excellent' => 'excelente',
    'good' => 'bom',
    'fair' || 'medium' || 'moderate' => 'regular',
    'poor' || 'weak' || 'bad' => 'fraco',
    _ => raw,
  };
}

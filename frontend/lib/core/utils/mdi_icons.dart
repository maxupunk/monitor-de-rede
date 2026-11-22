import 'package:flutter/material.dart';

/// Traduz nomes de ícones Material Design Icons (`mdi-*`) vindos do backend
/// (catálogo do Dashboard, presets de VPN, áreas, etc.) em `IconData` nativo.
IconData iconFromMdiName(
  String? name, {
  IconData fallback = Icons.devices_other_outlined,
}) {
  final normalized = (name ?? '').trim().toLowerCase();
  return switch (normalized) {
    'mdi-laptop' => Icons.laptop_outlined,
    'mdi-cellphone' => Icons.smartphone_outlined,
    'mdi-router-wireless' || 'mdi-router' => Icons.router_outlined,
    'mdi-server' || 'mdi-server-network' => Icons.dns_outlined,
    'mdi-lan' || 'mdi-lan-connect' => Icons.lan_outlined,
    'mdi-vpn' ||
    'mdi-shield-lock' ||
    'mdi-shield-key' => Icons.vpn_lock_outlined,
    'mdi-shield-check' || 'mdi-shield-outline' => Icons.shield_outlined,
    'mdi-cctv' || 'mdi-video' => Icons.videocam_outlined,
    'mdi-printer' => Icons.print_outlined,
    'mdi-desktop-tower' || 'mdi-desktop-classic' => Icons.computer_outlined,
    'mdi-nas' || 'mdi-database' => Icons.storage_outlined,
    'mdi-access-point' || 'mdi-wifi' => Icons.wifi,
    'mdi-phone-voip' => Icons.phone_in_talk_outlined,
    'mdi-chip' || 'mdi-cpu-64-bit' => Icons.memory_outlined,
    'mdi-memory' => Icons.developer_board_outlined,
    'mdi-cloud' || 'mdi-cloud-outline' => Icons.cloud_outlined,
    'mdi-docker' => Icons.view_in_ar_outlined,
    'mdi-view-dashboard' ||
    'mdi-view-dashboard-outline' => Icons.dashboard_outlined,
    'mdi-heart-pulse' || 'mdi-pulse' => Icons.monitor_heart_outlined,
    'mdi-bell' ||
    'mdi-bell-outline' ||
    'mdi-bell-alert' ||
    'mdi-bell-alert-outline' => Icons.notifications_outlined,
    'mdi-chart-line' || 'mdi-chart-areaspline' => Icons.show_chart,
    'mdi-chart-donut' || 'mdi-chart-pie' => Icons.pie_chart_outline,
    'mdi-alert' ||
    'mdi-alert-outline' ||
    'mdi-alert-octagon' ||
    'mdi-alert-circle-outline' => Icons.warning_amber_outlined,
    'mdi-history' || 'mdi-clock-outline' => Icons.history,
    'mdi-check-network' ||
    'mdi-check-circle-outline' => Icons.check_circle_outline,
    'mdi-text-box-search-outline' ||
    'mdi-file-document-outline' => Icons.article_outlined,
    'mdi-puzzle' || 'mdi-puzzle-outline' => Icons.extension_outlined,
    'mdi-robot' || 'mdi-robot-outline' => Icons.smart_toy_outlined,
    _ => fallback,
  };
}

import 'package:flutter/material.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';

/// Registro declarativo de widgets do Dashboard (17 tipos across Core/Network/Servers).
class DashboardWidgetRegistry {
  DashboardWidgetRegistry._();

  static final Map<String, DashboardWidgetContribution> _extra =
      <String, DashboardWidgetContribution>{};

  static void register(DashboardWidgetContribution contribution) {
    _extra[contribution.type] = contribution;
  }

  static void registerAll(Iterable<DashboardWidgetContribution> contributions) {
    for (final item in contributions) {
      _extra[item.type] = item;
    }
  }

  static DashboardWidgetContribution? resolve(String type) {
    return _extra[type] ?? allDashboardWidgets[type];
  }

  static Map<String, DashboardWidgetContribution> all() =>
      <String, DashboardWidgetContribution>{
        ...allDashboardWidgets,
        ..._extra,
      };
}

/// Registro declarativo das abas e cartões de Configurações.
class SettingsSectionRegistry {
  SettingsSectionRegistry._();

  static List<SettingsTabView> visibleTabs(Set<ProductArea> visibleAreas) =>
      buildVisibleSettingsTabs(visibleAreas);
}

/// Registro declarativo das abas da tela de Backup (Core + Databases/Storages de Servers).
class BackupTabRegistry {
  BackupTabRegistry._();

  static final Map<String, BackupTabContribution> _extra =
      <String, BackupTabContribution>{};

  static void register(BackupTabContribution tab) {
    _extra[tab.id] = tab;
  }

  static List<BackupTabContribution> resolve({required bool isAdmin}) {
    final merged = <String, BackupTabContribution>{
      for (final area in registeredAreas)
        for (final tab in area.backupTabs) tab.id: tab,
      ..._extra,
    };
    final list = merged.values
        .where((tab) => !tab.requiresAdmin || isAdmin)
        .toList()
      ..sort((a, b) => a.order.compareTo(b.order));
    return List<BackupTabContribution>.unmodifiable(list);
  }
}

/// Registro declarativo das abas de `DeviceDetailPage` (Network + Core + VPN/Plugins).
class DeviceDetailTabRegistry {
  DeviceDetailTabRegistry._();

  static final Map<String, DeviceDetailTabContribution> _extra =
      <String, DeviceDetailTabContribution>{};

  static void register(DeviceDetailTabContribution tab) {
    _extra[tab.id] = tab;
  }

  static List<DeviceDetailTabContribution> resolve({
    required int deviceId,
    required Set<ProductArea> visibleAreas,
  }) {
    final merged = <String, DeviceDetailTabContribution>{
      for (final area in registeredAreas)
        for (final tab in area.deviceDetailTabs) tab.id: tab,
      ..._extra,
    };
    final list = merged.values
        .where(
          (tab) =>
              visibleAreas.contains(tab.area) &&
              (tab.visibleWhen == null || tab.visibleWhen!(deviceId)),
        )
        .toList()
      ..sort((a, b) => a.order.compareTo(b.order));
    return List<DeviceDetailTabContribution>.unmodifiable(list);
  }
}

/// Slot declarativo dos caminhos de conexão de servidor remoto (LAN, VPN, Internet HTTPS).
class ServerConnectSlot {
  ServerConnectSlot._();

  static List<ServerConnectPathContribution> resolve(
    Set<ProductArea> visibleAreas,
  ) =>
      visibleServerConnectPaths(visibleAreas);
}

/// Overlays globais declarados pelas áreas visíveis (inclui `shellOverlays` sob
/// `area.id` e `extraShellOverlays` com gating independente por área ADR 017).
List<WidgetBuilder> visibleShellOverlaysFrom(
  Iterable<AreaContribution> modules,
  Set<ProductArea> visibleAreas,
) {
  final overlays = <WidgetBuilder>[];
  for (final area in modules) {
    if (visibleAreas.contains(area.id)) {
      overlays.addAll(area.shellOverlays);
    }
    area.extraShellOverlays.forEach((targetArea, builders) {
      if (visibleAreas.contains(targetArea)) {
        overlays.addAll(builders);
      }
    });
  }
  return List<WidgetBuilder>.unmodifiable(overlays);
}

List<WidgetBuilder> visibleShellOverlays(Set<ProductArea> visibleAreas) =>
    visibleShellOverlaysFrom(registeredAreas, visibleAreas);

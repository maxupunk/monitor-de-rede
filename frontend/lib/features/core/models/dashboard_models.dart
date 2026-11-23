import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Tamanho nomeado de widget no Dashboard (paridade com `WIDGET_SIZES` em `dashboard.ts`).
enum WidgetSize {
  third('third', '1/3 da largura', 4),
  half('half', 'Metade', 6),
  twoThirds('two_thirds', '2/3 da largura', 8),
  full('full', 'Largura total', 12);

  const WidgetSize(this.id, this.label, this.lgCols);

  final String id;
  final String label;
  final int lgCols;

  int get mdCols => switch (this) {
    WidgetSize.third => 6,
    WidgetSize.half => 6,
    WidgetSize.twoThirds => 12,
    WidgetSize.full => 12,
  };

  static WidgetSize parse(Object? raw) {
    final str = raw?.toString();
    for (final v in WidgetSize.values) {
      if (v.id == str) return v;
    }
    return WidgetSize.half;
  }

  static WidgetSize fromColumns(int? lg, int? md) {
    final effective = lg ?? md ?? 12;
    if (effective <= 4) return WidgetSize.third;
    if (effective <= 6) return WidgetSize.half;
    if (effective <= 8) return WidgetSize.twoThirds;
    return WidgetSize.full;
  }
}

enum DashboardSyncMode {
  server('server', 'Sincronizado com o servidor'),
  local('local', 'Somente neste dispositivo');

  const DashboardSyncMode(this.id, this.label);

  final String id;
  final String label;

  static DashboardSyncMode parse(String? raw) =>
      raw == 'local' ? DashboardSyncMode.local : DashboardSyncMode.server;
}

@immutable
class WidgetCatalogEntry {
  const WidgetCatalogEntry({
    required this.type,
    required this.title,
    required this.description,
    required this.category,
    required this.icon,
    required this.allowMultiple,
    required this.defaultSize,
    required this.configKeys,
    required this.compatibleResources,
    required this.area,
  });

  final String type;
  final String title;
  final String description;
  final String category;
  final String icon;
  final bool allowMultiple;
  final WidgetSize defaultSize;
  final List<String> configKeys;
  final List<String> compatibleResources;
  final ProductArea area;

  factory WidgetCatalogEntry.fromJson(Map<String, dynamic> json) {
    return WidgetCatalogEntry(
      type: (json['type'] ?? '').toString(),
      title: (json['title'] ?? '').toString(),
      description: (json['description'] ?? '').toString(),
      category: (json['category'] ?? 'charts').toString(),
      icon: (json['icon'] ?? '').toString(),
      allowMultiple: json['allowMultiple'] == true,
      defaultSize: WidgetSize.parse(json['defaultSize']),
      configKeys: asStringList(json['configKeys']),
      compatibleResources: asStringList(json['compatibleResources']),
      area: ProductArea.tryParse(json['area']) ?? ProductArea.core,
    );
  }
}

@immutable
class DefaultWidgetSpec {
  const DefaultWidgetSpec({
    required this.type,
    required this.size,
    required this.visible,
  });

  final String type;
  final WidgetSize size;
  final bool visible;

  factory DefaultWidgetSpec.fromJson(Map<String, dynamic> json) {
    return DefaultWidgetSpec(
      type: (json['type'] ?? '').toString(),
      size: WidgetSize.parse(json['size']),
      visible: json['visible'] != false,
    );
  }
}

@immutable
class DashboardCatalog {
  const DashboardCatalog({required this.widgets, required this.defaultLayout});

  final List<WidgetCatalogEntry> widgets;
  final List<DefaultWidgetSpec> defaultLayout;

  WidgetCatalogEntry? findByType(String? type) {
    if (type == null) return null;
    for (final entry in widgets) {
      if (entry.type == type) return entry;
    }
    return null;
  }

  factory DashboardCatalog.fromJson(Map<String, dynamic> json) {
    return DashboardCatalog(
      widgets: asJsonList<WidgetCatalogEntry>(
        json['widgets'],
        WidgetCatalogEntry.fromJson,
      ),
      defaultLayout: asJsonList<DefaultWidgetSpec>(
        json['defaultLayout'],
        DefaultWidgetSpec.fromJson,
      ),
    );
  }
}

@immutable
class DashboardWidgetConfig {
  const DashboardWidgetConfig({
    required this.id,
    required this.type,
    required this.title,
    required this.category,
    required this.visible,
    required this.order,
    required this.description,
    required this.icon,
    this.cols = 12,
    this.sm = 12,
    this.md = 6,
    this.lg = 6,
    this.config = const <String, dynamic>{},
  });

  final String id;
  final String type;
  final String title;
  final String category;
  final int cols;
  final int sm;
  final int md;
  final int lg;
  final bool visible;
  final int order;
  final String description;
  final String icon;
  final Map<String, dynamic> config;

  WidgetSize get size => WidgetSize.fromColumns(lg, md);

  DashboardWidgetConfig copyWith({
    String? id,
    String? type,
    String? title,
    String? category,
    int? cols,
    int? sm,
    int? md,
    int? lg,
    bool? visible,
    int? order,
    String? description,
    String? icon,
    Map<String, dynamic>? config,
  }) {
    return DashboardWidgetConfig(
      id: id ?? this.id,
      type: type ?? this.type,
      title: title ?? this.title,
      category: category ?? this.category,
      cols: cols ?? this.cols,
      sm: sm ?? this.sm,
      md: md ?? this.md,
      lg: lg ?? this.lg,
      visible: visible ?? this.visible,
      order: order ?? this.order,
      description: description ?? this.description,
      icon: icon ?? this.icon,
      config: config ?? this.config,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'type': type,
    'title': title,
    'category': category,
    'cols': cols,
    'sm': sm,
    'md': md,
    'lg': lg,
    'visible': visible,
    'order': order,
    'description': description,
    'icon': icon,
    'config': config,
  };

  static DashboardWidgetConfig? tryFromJson(
    Object? raw, {
    int fallbackOrder = 0,
  }) {
    if (raw is! Map) return null;
    final json = asJsonMap(raw);
    final id = json['id']?.toString();
    if (id == null || id.isEmpty) return null;
    final type = json['type']?.toString().isNotEmpty == true
        ? json['type'].toString()
        : id;
    return DashboardWidgetConfig(
      id: id,
      type: type,
      title: (json['title'] ?? 'Widget').toString(),
      category: (json['category'] ?? 'charts').toString(),
      cols: parseIntOrNull(json['cols']) ?? 12,
      sm: parseIntOrNull(json['sm']) ?? 12,
      md: parseIntOrNull(json['md']) ?? 6,
      lg: parseIntOrNull(json['lg']) ?? 6,
      visible: json['visible'] != false,
      order: parseIntOrNull(json['order']) ?? fallbackOrder,
      description: (json['description'] ?? '').toString(),
      icon: (json['icon'] ?? '').toString(),
      config: asJsonMap(json['config']),
    );
  }
}

@immutable
class MonitorTimeSeriesPoint {
  const MonitorTimeSeriesPoint({
    required this.time,
    required this.timestamp,
    required this.latency,
    required this.loss,
  });

  final String time;
  final int timestamp;
  final double latency;
  final double loss;

  factory MonitorTimeSeriesPoint.fromJson(Map<String, dynamic> json) {
    return MonitorTimeSeriesPoint(
      time: (json['time'] ?? '').toString(),
      timestamp: parseIntOrNull(json['timestamp']) ?? 0,
      latency: parseDoubleOrNull(json['latency']) ?? 0,
      loss: parseDoubleOrNull(json['loss']) ?? 0,
    );
  }
}

@immutable
class MonitorTimeSeriesResponse {
  const MonitorTimeSeriesResponse({
    required this.timeframe,
    required this.samples,
    required this.avgLatency,
    required this.maxLatency,
    required this.minLatency,
    required this.packetLossPct,
    required this.totalChecks,
  });

  final String timeframe;
  final List<MonitorTimeSeriesPoint> samples;
  final double avgLatency;
  final double maxLatency;
  final double minLatency;
  final double packetLossPct;
  final int totalChecks;

  factory MonitorTimeSeriesResponse.fromJson(Map<String, dynamic> json) {
    return MonitorTimeSeriesResponse(
      timeframe: (json['timeframe'] ?? '1h').toString(),
      samples: asJsonList<MonitorTimeSeriesPoint>(
        json['samples'],
        MonitorTimeSeriesPoint.fromJson,
      ),
      avgLatency: parseDoubleOrNull(json['avgLatency']) ?? 0,
      maxLatency: parseDoubleOrNull(json['maxLatency']) ?? 0,
      minLatency: parseDoubleOrNull(json['minLatency']) ?? 0,
      packetLossPct: parseDoubleOrNull(json['packetLossPct']) ?? 0,
      totalChecks: parseIntOrNull(json['totalChecks']) ?? 0,
    );
  }
}

@immutable
class HourlyEventBucket {
  const HourlyEventBucket({
    required this.hour,
    required this.critical,
    required this.warning,
    required this.info,
    required this.total,
  });

  final String hour;
  final int critical;
  final int warning;
  final int info;
  final int total;

  factory HourlyEventBucket.fromJson(Map<String, dynamic> json) {
    final crit =
        parseIntOrNull(json['critical']) ?? parseIntOrNull(json['error']) ?? 0;
    final warn = parseIntOrNull(json['warning']) ?? 0;
    final inf = parseIntOrNull(json['info']) ?? 0;
    final tot =
        parseIntOrNull(json['total']) ??
        parseIntOrNull(json['count']) ??
        (crit + warn + inf);
    return HourlyEventBucket(
      hour: (json['label'] ?? json['hour'] ?? json['bucket'] ?? '').toString(),
      critical: crit,
      warning: warn,
      info: inf,
      total: tot,
    );
  }
}

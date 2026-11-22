import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Ação declarada no manifesto de um plugin (`PluginAction`).
@immutable
class PluginAction {
  const PluginAction({
    required this.name,
    required this.title,
    this.description,
    this.effect = 'read',
    this.reveals = false,
    this.paramsSchema,
    this.outputKind,
    this.confirmMessage,
  });

  final String name;
  final String title;
  final String? description;
  final String effect; // 'read' | 'write'
  final bool reveals;
  final Map<String, dynamic>? paramsSchema;
  final String? outputKind;
  final String? confirmMessage;

  bool get isWrite => effect == 'write';

  factory PluginAction.fromJson(Map<String, dynamic> json) {
    final rawParams = json['paramsSchema'] ?? json['params'];
    return PluginAction(
      name: (json['name'] ?? json['id'] ?? '').toString(),
      title: (json['title'] ?? json['name'] ?? json['id'] ?? '').toString(),
      description: json['description'] as String?,
      effect: (json['effect'] as String?) ?? 'read',
      reveals: parseBool(json['reveals']),
      paramsSchema: rawParams is Map ? asJsonMap(rawParams) : null,
      outputKind: (json['outputKind'] ?? json['output']) as String?,
      confirmMessage: json['confirmMessage'] as String?,
    );
  }
}

/// Especificação de ação de item em `ItemList`.
@immutable
class PluginItemAction {
  const PluginItemAction({
    required this.action,
    this.title,
    this.icon,
    this.effect,
    this.fields = const <String, String>{},
  });

  final String action;
  final String? title;
  final String? icon;
  final String? effect;
  final Map<String, String> fields;

  factory PluginItemAction.fromJson(Map<String, dynamic> json) {
    return PluginItemAction(
      action: (json['action'] as String?) ?? '',
      title: (json['title'] ?? json['label']) as String?,
      icon: json['icon'] as String?,
      effect: json['effect'] as String?,
      fields: asStringMap(json['fields'] ?? json['params']),
    );
  }
}

/// Especificação de QR Wi-Fi numa lista de redes (`list.wifiQr`).
@immutable
class PluginWifiQrSpec {
  const PluginWifiQrSpec({
    required this.ssid,
    required this.encryption,
    this.hidden,
    required this.revealAction,
    this.revealFields = const <String, String>{},
    required this.secretField,
  });

  final String ssid;
  final String encryption;
  final String? hidden;
  final String revealAction;
  final Map<String, String> revealFields;
  final String secretField;

  factory PluginWifiQrSpec.fromJson(Map<String, dynamic> json) {
    final rawReveal = json['reveal'];
    var revealAction = '';
    var revealFields = const <String, String>{};
    if (rawReveal is String) {
      revealAction = rawReveal;
    } else if (rawReveal is Map) {
      revealAction = (rawReveal['action'] as String?) ?? '';
      revealFields = asStringMap(rawReveal['fields']);
    }
    return PluginWifiQrSpec(
      ssid: (json['ssid'] as String?) ?? 'ssid',
      encryption: (json['encryption'] as String?) ?? 'encryption',
      hidden: json['hidden'] as String?,
      revealAction: revealAction,
      revealFields: revealFields,
      secretField: (json['secret'] as String?) ?? 'password',
    );
  }
}

/// Especificação de lista declarativa de um plugin (`ItemList`).
@immutable
class PluginItemListSpec {
  const PluginItemListSpec({
    required this.title,
    this.itemName,
    this.itemKey,
    this.layout = 'table',
    this.sourceAction,
    this.itemsKey = 'items',
    this.searchParam,
    this.columns = const <Map<String, dynamic>>[],
    this.addAction,
    this.editAction,
    this.removeAction,
    this.rowActions = const <PluginItemAction>[],
    this.wifiQr,
  });

  final String title;
  final String? itemName;
  final String? itemKey;
  final String layout; // 'cards' | 'table'
  final String? sourceAction;
  final String itemsKey;
  final String? searchParam;
  final List<Map<String, dynamic>> columns;
  final PluginItemAction? addAction;
  final PluginItemAction? editAction;
  final PluginItemAction? removeAction;
  final List<PluginItemAction> rowActions;
  final PluginWifiQrSpec? wifiQr;

  factory PluginItemListSpec.fromJson(Map<String, dynamic> json) {
    PluginItemAction? parseAction(Object? raw) {
      if (raw is String && raw.isNotEmpty) {
        return PluginItemAction(action: raw);
      }
      if (raw is Map) {
        return PluginItemAction.fromJson(asJsonMap(raw));
      }
      return null;
    }

    return PluginItemListSpec(
      title: (json['title'] as String?) ?? 'Itens',
      itemName: json['itemName'] as String?,
      itemKey: (json['itemKey'] ?? json['key']) as String?,
      layout: (json['layout'] as String?) ?? 'table',
      sourceAction: (json['action'] ?? json['source']) as String?,
      itemsKey: (json['itemsKey'] ?? json['field'] ?? 'items').toString(),
      searchParam: json['searchParam'] as String?,
      columns: asJsonMapList(json['columns']),
      addAction: parseAction(json['add']),
      editAction: parseAction(json['edit']),
      removeAction: parseAction(json['remove']),
      rowActions: asJsonList<PluginItemAction>(
        json['rowActions'],
        PluginItemAction.fromJson,
      ),
      wifiQr: json['wifiQr'] is Map
          ? PluginWifiQrSpec.fromJson(asJsonMap(json['wifiQr']))
          : null,
    );
  }
}

/// Especificação de frota (`FleetSpec`).
@immutable
class PluginFleetSpec {
  const PluginFleetSpec({
    this.title,
    this.icon,
    this.description,
    this.statusAction,
    this.deviceAction,
    this.actions = const <Map<String, dynamic>>[],
    this.tools = const <String>[],
    this.list,
  });

  final String? title;
  final String? icon;
  final String? description;
  final String? statusAction;
  final String? deviceAction;
  final List<Map<String, dynamic>> actions;
  final List<String> tools;
  final PluginItemListSpec? list;

  factory PluginFleetSpec.fromJson(Map<String, dynamic> json) {
    return PluginFleetSpec(
      title: json['title'] as String?,
      icon: json['icon'] as String?,
      description: json['description'] as String?,
      statusAction: json['statusAction'] as String?,
      deviceAction: json['deviceAction'] as String?,
      actions: asJsonMapList(json['actions']),
      tools: asStringList(json['tools']),
      list: json['list'] is Map
          ? PluginItemListSpec.fromJson(asJsonMap(json['list']))
          : null,
    );
  }
}

/// Esquemas de configuração guardada (`SettingsSpec`).
@immutable
class PluginSettingsSpec {
  const PluginSettingsSpec({this.fleet, this.device});

  final Map<String, dynamic>? fleet;
  final Map<String, dynamic>? device;

  factory PluginSettingsSpec.fromJson(Map<String, dynamic> json) {
    return PluginSettingsSpec(
      fleet: json['fleet'] is Map ? asJsonMap(json['fleet']) : null,
      device: json['device'] is Map ? asJsonMap(json['device']) : null,
    );
  }
}

/// Resumo de plugin na biblioteca (`PluginSummary`).
@immutable
class PluginSummary {
  const PluginSummary({
    required this.id,
    required this.slug,
    required this.name,
    required this.version,
    this.description,
    this.scope = 'model',
    this.deviceId,
    this.source = 'user',
    this.status = 'draft',
    this.transports = const <String>[],
    this.actions = const <PluginAction>[],
    this.matcher = const <String, dynamic>{},
    this.list,
    this.surfaces = const <String>[],
    this.settings,
    this.fleet,
    this.wifi = false,
    this.risk,
    this.lastTestAt,
    this.lastTestOk,
    this.validatedCount = 0,
    this.updatedAt = '',
  });

  final int id;
  final String slug;
  final String name;
  final String version;
  final String? description;
  final String scope; // 'model' | 'device'
  final int? deviceId;
  final String source; // 'builtin' | 'user' | 'ai' | 'imported'
  final String
  status; // 'quarantine' | 'draft' | 'tested' | 'active' | 'disabled'
  final List<String> transports;
  final List<PluginAction> actions;
  final Map<String, dynamic> matcher;
  final PluginItemListSpec? list;
  final List<String> surfaces;
  final PluginSettingsSpec? settings;
  final PluginFleetSpec? fleet;
  final bool wifi;
  final String? risk;
  final String? lastTestAt;
  final bool? lastTestOk;
  final int validatedCount;
  final String updatedAt;

  bool get opensApp => fleet != null && status == 'active';

  PluginAction? actionByName(String name) {
    for (final a in actions) {
      if (a.name == name) return a;
    }
    return null;
  }

  factory PluginSummary.fromJson(Map<String, dynamic> json) {
    return PluginSummary(
      id: parseIntOrNull(json['id']) ?? 0,
      slug: (json['slug'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      version: (json['version'] as String?) ?? '1.0.0',
      description: json['description'] as String?,
      scope: (json['scope'] as String?) ?? 'model',
      deviceId: parseIntOrNull(json['deviceId']),
      source: (json['source'] as String?) ?? 'user',
      status: (json['status'] as String?) ?? 'draft',
      transports: asStringList(json['transports']),
      actions: asJsonList<PluginAction>(json['actions'], PluginAction.fromJson),
      matcher: asJsonMap(json['matcher']),
      list: json['list'] is Map
          ? PluginItemListSpec.fromJson(asJsonMap(json['list']))
          : null,
      surfaces: asStringList(json['surfaces']),
      settings: json['settings'] is Map
          ? PluginSettingsSpec.fromJson(asJsonMap(json['settings']))
          : null,
      fleet: json['fleet'] is Map
          ? PluginFleetSpec.fromJson(asJsonMap(json['fleet']))
          : null,
      wifi: parseBool(json['wifi']),
      risk: json['risk'] as String?,
      lastTestAt: json['lastTestAt'] as String?,
      lastTestOk: json['lastTestOk'] as bool?,
      validatedCount: parseIntOrNull(json['validatedCount']) ?? 0,
      updatedAt: (json['updatedAt'] as String?) ?? '',
    );
  }
}

/// Pacote completo editável/exportável de um plugin (`PluginPackage`).
@immutable
class PluginPackage {
  const PluginPackage({
    required this.manifest,
    required this.script,
    this.usage = '',
    this.tests = const <String, dynamic>{},
    this.raw = const <String, dynamic>{},
  });

  final Map<String, dynamic> manifest;
  final String script;
  final String usage;
  final Map<String, dynamic> tests;
  final Map<String, dynamic> raw;

  Map<String, dynamic> toJson() => <String, dynamic>{
    ...raw,
    'manifest': manifest,
    'script': script,
    'usage': usage,
    'tests': tests,
  };

  factory PluginPackage.fromJson(Map<String, dynamic> json) {
    return PluginPackage(
      manifest: asJsonMap(json['manifest']),
      script: (json['script'] as String?) ?? '',
      usage: (json['usage'] as String?) ?? '',
      tests: asJsonMap(json['tests']),
      raw: Map<String, dynamic>.from(json),
    );
  }
}

/// Achado de revisão de segurança (`ReviewFinding`).
@immutable
class ReviewFinding {
  const ReviewFinding({
    required this.severity,
    required this.message,
    this.line,
    this.rule,
    this.excerpt,
    this.source = 'static',
  });

  final String severity;
  final String message;
  final int? line;
  final String? rule;
  final String? excerpt;
  final String source;

  factory ReviewFinding.fromJson(
    Map<String, dynamic> json, {
    String defaultSource = 'static',
  }) {
    return ReviewFinding(
      severity: (json['severity'] as String?) ?? 'info',
      message: (json['message'] as String?) ?? '',
      line: parseIntOrNull(json['line']),
      rule: json['rule'] as String?,
      excerpt: json['excerpt'] as String?,
      source: (json['source'] as String?) ?? defaultSource,
    );
  }
}

@immutable
class PluginAiReview {
  const PluginAiReview({
    required this.summary,
    this.mismatchWithUsage = false,
    this.findings = const <ReviewFinding>[],
  });

  final String summary;
  final bool mismatchWithUsage;
  final List<ReviewFinding> findings;

  factory PluginAiReview.fromJson(Map<String, dynamic> json) {
    return PluginAiReview(
      summary: (json['summary'] as String?) ?? '',
      mismatchWithUsage: parseBool(json['mismatchWithUsage']),
      findings: asJsonList<ReviewFinding>(
        json['findings'],
        (f) => ReviewFinding.fromJson(f, defaultSource: 'ai'),
      ),
    );
  }
}

/// Relatório de revisão de quarentena (`ReviewReport`).
@immutable
class ReviewReport {
  const ReviewReport({
    required this.risk,
    this.reviewedAt,
    this.staticFindings = const <ReviewFinding>[],
    this.ai,
  });

  final String risk; // 'low' | 'medium' | 'high' | 'critical'
  final String? reviewedAt;
  final List<ReviewFinding> staticFindings;
  final PluginAiReview? ai;

  List<ReviewFinding> get allFindings => <ReviewFinding>[
    ...staticFindings,
    ...?ai?.findings,
  ];

  bool get isBlocked => risk == 'critical';
  bool get requiresAcknowledgement => ai == null || risk == 'high';

  factory ReviewReport.fromJson(Map<String, dynamic> json) {
    final rawFindings = json['findings'] ?? json['staticFindings'];
    return ReviewReport(
      risk: (json['risk'] as String?) ?? 'medium',
      reviewedAt: json['reviewedAt'] as String?,
      staticFindings: asJsonList<ReviewFinding>(
        rawFindings,
        ReviewFinding.fromJson,
      ),
      ai: json['ai'] is Map
          ? PluginAiReview.fromJson(asJsonMap(json['ai']))
          : null,
    );
  }
}

@immutable
class PluginDetail {
  const PluginDetail({
    required this.summary,
    required this.package,
    this.review,
  });

  final PluginSummary summary;
  final PluginPackage package;
  final ReviewReport? review;

  factory PluginDetail.fromJson(Map<String, dynamic> json) {
    return PluginDetail(
      summary: PluginSummary.fromJson(asJsonMap(json['summary'])),
      package: PluginPackage.fromJson(asJsonMap(json['package'])),
      review: json['review'] is Map
          ? ReviewReport.fromJson(asJsonMap(json['review']))
          : null,
    );
  }
}

/// Resultado de testes e prévia no sandbox (`TestReport` / `PluginPreview`).
@immutable
class TestCaseResult {
  const TestCaseResult({
    required this.name,
    required this.passed,
    this.error,
    this.output,
  });

  final String name;
  final bool passed;
  final String? error;
  final Object? output;

  factory TestCaseResult.fromJson(Map<String, dynamic> json) {
    return TestCaseResult(
      name: (json['name'] as String?) ?? '',
      passed: parseBool(json['passed']),
      error: json['error'] as String?,
      output: json['output'],
    );
  }
}

@immutable
class PluginTestReport {
  const PluginTestReport({
    required this.passed,
    required this.total,
    required this.failed,
    this.problems = const <String>[],
    this.cases = const <TestCaseResult>[],
    this.hints = const <String>[],
  });

  final int passed;
  final int total;
  final int failed;
  final List<String> problems;
  final List<TestCaseResult> cases;
  final List<String> hints;

  factory PluginTestReport.fromJson(Map<String, dynamic> json) {
    final rawHints = json['hints'];
    final hints = <String>[];
    if (rawHints is List) {
      for (final h in rawHints) {
        if (h is String) {
          hints.add(h);
        } else if (h is Map && h['message'] is String) {
          hints.add(h['message'] as String);
        }
      }
    }
    return PluginTestReport(
      passed: parseIntOrNull(json['passed']) ?? 0,
      total: parseIntOrNull(json['total']) ?? 0,
      failed: parseIntOrNull(json['failed']) ?? 0,
      problems: asStringList(json['problems']),
      cases: asJsonList<TestCaseResult>(json['cases'], TestCaseResult.fromJson),
      hints: hints,
    );
  }
}

@immutable
class PluginPreviewResult {
  const PluginPreviewResult({required this.report});

  final PluginTestReport report;

  factory PluginPreviewResult.fromJson(Map<String, dynamic> json) {
    return PluginPreviewResult(
      report: PluginTestReport.fromJson(
        json['report'] is Map ? asJsonMap(json['report']) : json,
      ),
    );
  }
}

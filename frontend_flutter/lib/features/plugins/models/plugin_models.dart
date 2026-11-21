import 'package:flutter/foundation.dart';

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
    return PluginAction(
      name: (json['name'] as String?) ?? '',
      title: (json['title'] as String?) ?? (json['name'] as String?) ?? '',
      description: json['description'] as String?,
      effect: (json['effect'] as String?) ?? 'read',
      reveals: json['reveals'] == true,
      paramsSchema: json['paramsSchema'] is Map
          ? Map<String, dynamic>.from(json['paramsSchema'] as Map)
          : (json['params'] is Map
                ? Map<String, dynamic>.from(json['params'] as Map)
                : null),
      outputKind: json['outputKind'] as String?,
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
    final rawFields = json['fields'];
    final fields = <String, String>{};
    if (rawFields is Map) {
      rawFields.forEach((k, v) => fields[k.toString()] = v.toString());
    }
    return PluginItemAction(
      action: (json['action'] as String?) ?? '',
      title: json['title'] as String?,
      icon: json['icon'] as String?,
      effect: json['effect'] as String?,
      fields: fields,
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
    final revealFields = <String, String>{};
    if (rawReveal is String) {
      revealAction = rawReveal;
    } else if (rawReveal is Map) {
      revealAction = (rawReveal['action'] as String?) ?? '';
      if (rawReveal['fields'] is Map) {
        (rawReveal['fields'] as Map).forEach(
          (k, v) => revealFields[k.toString()] = v.toString(),
        );
      }
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
        return PluginItemAction.fromJson(Map<String, dynamic>.from(raw));
      }
      return null;
    }

    return PluginItemListSpec(
      title: (json['title'] as String?) ?? 'Itens',
      layout: (json['layout'] as String?) ?? 'table',
      sourceAction: (json['action'] ?? json['source']) as String?,
      itemsKey: (json['itemsKey'] ?? json['key'] ?? 'items').toString(),
      searchParam: json['searchParam'] as String?,
      columns: json['columns'] is List
          ? (json['columns'] as List)
                .whereType<Map>()
                .map((c) => Map<String, dynamic>.from(c))
                .toList(growable: false)
          : const <Map<String, dynamic>>[],
      addAction: parseAction(json['add']),
      editAction: parseAction(json['edit']),
      removeAction: parseAction(json['remove']),
      rowActions: json['rowActions'] is List
          ? (json['rowActions'] as List)
                .whereType<Map>()
                .map(
                  (a) =>
                      PluginItemAction.fromJson(Map<String, dynamic>.from(a)),
                )
                .toList(growable: false)
          : const <PluginItemAction>[],
      wifiQr: json['wifiQr'] is Map
          ? PluginWifiQrSpec.fromJson(
              Map<String, dynamic>.from(json['wifiQr'] as Map),
            )
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
    this.statusAction,
    this.deviceAction,
    this.actions = const <Map<String, dynamic>>[],
    this.list,
  });

  final String? title;
  final String? icon;
  final String? statusAction;
  final String? deviceAction;
  final List<Map<String, dynamic>> actions;
  final PluginItemListSpec? list;

  factory PluginFleetSpec.fromJson(Map<String, dynamic> json) {
    return PluginFleetSpec(
      title: json['title'] as String?,
      icon: json['icon'] as String?,
      statusAction: json['statusAction'] as String?,
      deviceAction: json['deviceAction'] as String?,
      actions: json['actions'] is List
          ? (json['actions'] as List)
                .whereType<Map>()
                .map((a) => Map<String, dynamic>.from(a))
                .toList(growable: false)
          : const <Map<String, dynamic>>[],
      list: json['list'] is Map
          ? PluginItemListSpec.fromJson(
              Map<String, dynamic>.from(json['list'] as Map),
            )
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
      fleet: json['fleet'] is Map
          ? Map<String, dynamic>.from(json['fleet'] as Map)
          : null,
      device: json['device'] is Map
          ? Map<String, dynamic>.from(json['device'] as Map)
          : null,
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
      id: (json['id'] as num?)?.toInt() ?? 0,
      slug: (json['slug'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      version: (json['version'] as String?) ?? '1.0.0',
      description: json['description'] as String?,
      scope: (json['scope'] as String?) ?? 'model',
      deviceId: (json['deviceId'] as num?)?.toInt(),
      source: (json['source'] as String?) ?? 'user',
      status: (json['status'] as String?) ?? 'draft',
      transports: json['transports'] is List
          ? (json['transports'] as List)
                .map((e) => e.toString())
                .toList(growable: false)
          : const <String>[],
      actions: json['actions'] is List
          ? (json['actions'] as List)
                .whereType<Map>()
                .map((a) => PluginAction.fromJson(Map<String, dynamic>.from(a)))
                .toList(growable: false)
          : const <PluginAction>[],
      matcher: json['matcher'] is Map
          ? Map<String, dynamic>.from(json['matcher'] as Map)
          : const <String, dynamic>{},
      list: json['list'] is Map
          ? PluginItemListSpec.fromJson(
              Map<String, dynamic>.from(json['list'] as Map),
            )
          : null,
      surfaces: json['surfaces'] is List
          ? (json['surfaces'] as List)
                .map((e) => e.toString())
                .toList(growable: false)
          : const <String>[],
      settings: json['settings'] is Map
          ? PluginSettingsSpec.fromJson(
              Map<String, dynamic>.from(json['settings'] as Map),
            )
          : null,
      fleet: json['fleet'] is Map
          ? PluginFleetSpec.fromJson(
              Map<String, dynamic>.from(json['fleet'] as Map),
            )
          : null,
      wifi: json['wifi'] == true,
      risk: json['risk'] as String?,
      lastTestAt: json['lastTestAt'] as String?,
      lastTestOk: json['lastTestOk'] as bool?,
      validatedCount: (json['validatedCount'] as num?)?.toInt() ?? 0,
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
      manifest: json['manifest'] is Map
          ? Map<String, dynamic>.from(json['manifest'] as Map)
          : <String, dynamic>{},
      script: (json['script'] as String?) ?? '',
      usage: (json['usage'] as String?) ?? '',
      tests: json['tests'] is Map
          ? Map<String, dynamic>.from(json['tests'] as Map)
          : <String, dynamic>{},
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
      line: (json['line'] as num?)?.toInt(),
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
      mismatchWithUsage: json['mismatchWithUsage'] == true,
      findings: json['findings'] is List
          ? (json['findings'] as List)
                .whereType<Map>()
                .map(
                  (f) => ReviewFinding.fromJson(
                    Map<String, dynamic>.from(f),
                    defaultSource: 'ai',
                  ),
                )
                .toList(growable: false)
          : const <ReviewFinding>[],
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
      staticFindings: rawFindings is List
          ? rawFindings
                .whereType<Map>()
                .map(
                  (f) => ReviewFinding.fromJson(Map<String, dynamic>.from(f)),
                )
                .toList(growable: false)
          : const <ReviewFinding>[],
      ai: json['ai'] is Map
          ? PluginAiReview.fromJson(
              Map<String, dynamic>.from(json['ai'] as Map),
            )
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
      summary: PluginSummary.fromJson(
        Map<String, dynamic>.from(
          (json['summary'] as Map?) ?? const <String, dynamic>{},
        ),
      ),
      package: PluginPackage.fromJson(
        Map<String, dynamic>.from(
          (json['package'] as Map?) ?? const <String, dynamic>{},
        ),
      ),
      review: json['review'] is Map
          ? ReviewReport.fromJson(
              Map<String, dynamic>.from(json['review'] as Map),
            )
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
      passed: json['passed'] == true,
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
      passed: (json['passed'] as num?)?.toInt() ?? 0,
      total: (json['total'] as num?)?.toInt() ?? 0,
      failed: (json['failed'] as num?)?.toInt() ?? 0,
      problems: json['problems'] is List
          ? (json['problems'] as List)
                .map((e) => e.toString())
                .toList(growable: false)
          : const <String>[],
      cases: json['cases'] is List
          ? (json['cases'] as List)
                .whereType<Map>()
                .map(
                  (c) => TestCaseResult.fromJson(Map<String, dynamic>.from(c)),
                )
                .toList(growable: false)
          : const <TestCaseResult>[],
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
        Map<String, dynamic>.from((json['report'] as Map?) ?? json),
      ),
    );
  }
}

// --- Credenciais e Acesso Dedicado (ADR 016) ---

@immutable
class DedicatedAccess {
  const DedicatedAccess({
    required this.username,
    this.fingerprint,
    this.hostKey,
    this.platform,
    this.rotatedAt,
  });

  final String username;
  final String? fingerprint;
  final String? hostKey;
  final String? platform;
  final String? rotatedAt;

  factory DedicatedAccess.fromJson(Map<String, dynamic> json) {
    return DedicatedAccess(
      username: (json['username'] as String?) ?? '',
      fingerprint: json['fingerprint'] as String?,
      hostKey: json['hostKey'] as String?,
      platform: json['platform'] as String?,
      rotatedAt: json['rotatedAt'] as String?,
    );
  }
}

@immutable
class CredentialView {
  const CredentialView({
    required this.kind,
    required this.username,
    required this.storage,
    this.port,
    this.https = false,
    this.viaProbeId,
    this.sessionActive = false,
    this.sessionExpiresAt,
    this.dedicated,
  });

  final String kind; // 'ssh' | 'telnet' | 'http'
  final String username;
  final String storage; // 'ask' | 'key' | 'vault'
  final int? port;
  final bool https;
  final int? viaProbeId;
  final bool sessionActive;
  final String? sessionExpiresAt;
  final DedicatedAccess? dedicated;

  factory CredentialView.fromJson(Map<String, dynamic> json) {
    return CredentialView(
      kind: (json['kind'] as String?) ?? 'ssh',
      username: (json['username'] as String?) ?? '',
      storage: (json['storage'] as String?) ?? 'ask',
      port: (json['port'] as num?)?.toInt(),
      https: json['https'] == true,
      viaProbeId: (json['viaProbeId'] as num?)?.toInt(),
      sessionActive: json['sessionActive'] == true,
      sessionExpiresAt: json['sessionExpiresAt'] as String?,
      dedicated: json['dedicated'] is Map
          ? DedicatedAccess.fromJson(
              Map<String, dynamic>.from(json['dedicated'] as Map),
            )
          : null,
    );
  }
}

@immutable
class AgentRouteOption {
  const AgentRouteOption({
    required this.probeId,
    required this.name,
    this.online = true,
    this.hasDeviceIo = true,
  });

  final int probeId;
  final String name;
  final bool online;
  final bool hasDeviceIo;

  factory AgentRouteOption.fromJson(Map<String, dynamic> json) {
    return AgentRouteOption(
      probeId: (json['probeId'] ?? json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? 'Agente',
      online: json['online'] != false,
      hasDeviceIo: json['hasDeviceIo'] != false && json['deviceIo'] != false,
    );
  }
}

@immutable
class DeviceAccessView {
  const DeviceAccessView({
    required this.deviceId,
    required this.deviceName,
    this.credentials = const <CredentialView>[],
    this.agents = const <AgentRouteOption>[],
    this.dedicatedPlatform,
  });

  final int deviceId;
  final String deviceName;
  final List<CredentialView> credentials;
  final List<AgentRouteOption> agents;
  final String? dedicatedPlatform;

  factory DeviceAccessView.fromJson(Map<String, dynamic> json) {
    return DeviceAccessView(
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      deviceName: (json['deviceName'] as String?) ?? '',
      credentials: json['credentials'] is List
          ? (json['credentials'] as List)
                .whereType<Map>()
                .map(
                  (c) => CredentialView.fromJson(Map<String, dynamic>.from(c)),
                )
                .toList(growable: false)
          : const <CredentialView>[],
      agents: json['agents'] is List
          ? (json['agents'] as List)
                .whereType<Map>()
                .map(
                  (a) =>
                      AgentRouteOption.fromJson(Map<String, dynamic>.from(a)),
                )
                .toList(growable: false)
          : const <AgentRouteOption>[],
      dedicatedPlatform: json['dedicatedPlatform'] as String?,
    );
  }
}

// --- Execuções, Transcrição e Aprovações de Plugin ---

@immutable
class TranscriptEntry {
  const TranscriptEntry({
    required this.seq,
    required this.kind,
    required this.request,
    this.response,
    this.durationMs = 0,
    this.ok = true,
  });

  final int seq;
  final String
  kind; // 'ssh' | 'telnet' | 'http' | 'log' | 'denied' | 'error' | 'step:*'
  final String request;
  final String? response;
  final int durationMs;
  final bool ok;

  factory TranscriptEntry.fromJson(Map<String, dynamic> json) {
    return TranscriptEntry(
      seq: (json['seq'] as num?)?.toInt() ?? 0,
      kind: (json['kind'] as String?) ?? 'log',
      request: (json['request'] as String?) ?? '',
      response: json['response'] as String?,
      durationMs: (json['durationMs'] as num?)?.toInt() ?? 0,
      ok: json['ok'] != false,
    );
  }
}

@immutable
class PluginRunView {
  const PluginRunView({
    required this.id,
    required this.pluginId,
    this.pluginName,
    required this.deviceId,
    required this.action,
    this.origin = 'user',
    required this.status,
    this.params = const <String, dynamic>{},
    this.output,
    this.transcript = const <TranscriptEntry>[],
    this.error,
    this.createdAt = '',
    this.finishedAt,
  });

  final int id;
  final int pluginId;
  final String? pluginName;
  final int deviceId;
  final String action;
  final String origin;
  final String status; // 'running' | 'succeeded' | 'failed' | 'cancelled'
  final Map<String, dynamic> params;
  final Object? output;
  final List<TranscriptEntry> transcript;
  final String? error;
  final String createdAt;
  final String? finishedAt;

  bool get isFinished => status != 'running' && status != 'queued';

  PluginRunView copyWith({
    String? pluginName,
    String? status,
    Object? output,
    List<TranscriptEntry>? transcript,
    String? error,
    String? finishedAt,
  }) {
    return PluginRunView(
      id: id,
      pluginId: pluginId,
      pluginName: pluginName ?? this.pluginName,
      deviceId: deviceId,
      action: action,
      origin: origin,
      status: status ?? this.status,
      params: params,
      output: output ?? this.output,
      transcript: transcript ?? this.transcript,
      error: error ?? this.error,
      createdAt: createdAt,
      finishedAt: finishedAt ?? this.finishedAt,
    );
  }

  factory PluginRunView.fromJson(Map<String, dynamic> json) {
    return PluginRunView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      pluginId: (json['pluginId'] as num?)?.toInt() ?? 0,
      pluginName: json['pluginName'] as String?,
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      action: (json['action'] as String?) ?? '',
      origin: (json['origin'] as String?) ?? 'user',
      status: (json['status'] as String?) ?? 'running',
      params: json['params'] is Map
          ? Map<String, dynamic>.from(json['params'] as Map)
          : const <String, dynamic>{},
      output: json['output'],
      transcript: json['transcript'] is List
          ? (json['transcript'] as List)
                .whereType<Map>()
                .map(
                  (t) => TranscriptEntry.fromJson(Map<String, dynamic>.from(t)),
                )
                .toList(growable: false)
          : const <TranscriptEntry>[],
      error: json['error'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

@immutable
class DevicePluginItem {
  const DevicePluginItem({
    required this.plugin,
    required this.installed,
    required this.compatibility,
    this.reasons = const <String>[],
    this.deviceSettings,
  });

  final PluginSummary plugin;
  final bool installed;
  final String
  compatibility; // 'validated' | 'likely' | 'possible' | 'incompatible'
  final List<String> reasons;
  final Map<String, dynamic>? deviceSettings;

  DevicePluginItem copyWith({Map<String, dynamic>? deviceSettings}) {
    return DevicePluginItem(
      plugin: plugin,
      installed: installed,
      compatibility: compatibility,
      reasons: reasons,
      deviceSettings: deviceSettings ?? this.deviceSettings,
    );
  }

  factory DevicePluginItem.fromJson(Map<String, dynamic> json) {
    return DevicePluginItem(
      plugin: PluginSummary.fromJson(
        Map<String, dynamic>.from(
          (json['plugin'] as Map?) ?? const <String, dynamic>{},
        ),
      ),
      installed: json['installed'] == true,
      compatibility: (json['compatibility'] as String?) ?? 'possible',
      reasons: json['reasons'] is List
          ? (json['reasons'] as List)
                .map((e) => e.toString())
                .toList(growable: false)
          : const <String>[],
      deviceSettings: json['deviceSettings'] is Map
          ? Map<String, dynamic>.from(json['deviceSettings'] as Map)
          : null,
    );
  }
}

@immutable
class DevicePluginsView {
  const DevicePluginsView({
    required this.deviceId,
    required this.platform,
    this.firmware,
    this.plugins = const <DevicePluginItem>[],
    this.credentials = const <CredentialView>[],
    this.runs = const <PluginRunView>[],
    this.agents = const <AgentRouteOption>[],
  });

  final int deviceId;
  final String platform;
  final String? firmware;
  final List<DevicePluginItem> plugins;
  final List<CredentialView> credentials;
  final List<PluginRunView> runs;
  final List<AgentRouteOption> agents;

  DevicePluginsView copyWith({
    String? firmware,
    List<DevicePluginItem>? plugins,
    List<CredentialView>? credentials,
    List<PluginRunView>? runs,
  }) {
    return DevicePluginsView(
      deviceId: deviceId,
      platform: platform,
      firmware: firmware ?? this.firmware,
      plugins: plugins ?? this.plugins,
      credentials: credentials ?? this.credentials,
      runs: runs ?? this.runs,
      agents: agents,
    );
  }

  factory DevicePluginsView.fromJson(Map<String, dynamic> json) {
    return DevicePluginsView(
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      platform: (json['platform'] as String?) ?? 'unknown',
      firmware: json['firmware'] as String?,
      plugins: json['plugins'] is List
          ? (json['plugins'] as List)
                .whereType<Map>()
                .map(
                  (p) =>
                      DevicePluginItem.fromJson(Map<String, dynamic>.from(p)),
                )
                .toList(growable: false)
          : const <DevicePluginItem>[],
      credentials: json['credentials'] is List
          ? (json['credentials'] as List)
                .whereType<Map>()
                .map(
                  (c) => CredentialView.fromJson(Map<String, dynamic>.from(c)),
                )
                .toList(growable: false)
          : const <CredentialView>[],
      runs: json['runs'] is List
          ? (json['runs'] as List)
                .whereType<Map>()
                .map(
                  (r) => PluginRunView.fromJson(Map<String, dynamic>.from(r)),
                )
                .toList(growable: false)
          : const <PluginRunView>[],
      agents: json['agents'] is List
          ? (json['agents'] as List)
                .whereType<Map>()
                .map(
                  (a) =>
                      AgentRouteOption.fromJson(Map<String, dynamic>.from(a)),
                )
                .toList(growable: false)
          : const <AgentRouteOption>[],
    );
  }
}

@immutable
class ProgressSample {
  const ProgressSample({
    required this.tMs,
    required this.flow,
    required this.bps,
  });

  final int tMs;
  final String flow; // 'tx' | 'rx'
  final double bps;
}

/// Pedido de aprovação de acesso individual (`plugin:approval_required`).
@immutable
class PluginApprovalRequest {
  const PluginApprovalRequest({
    required this.key,
    required this.runId,
    required this.deviceId,
    required this.deviceName,
    required this.pluginName,
    required this.action,
    required this.transport,
    required this.effect,
    required this.summary,
    this.reason,
    required this.expiresAt,
  });

  final String key;
  final int runId;
  final int deviceId;
  final String deviceName;
  final String pluginName;
  final String action;
  final String transport;
  final String effect; // 'read' | 'write'
  final String summary;
  final String? reason;
  final String expiresAt;

  bool get isWrite => effect == 'write';

  static PluginApprovalRequest? tryFromJson(Map<String, dynamic> json) {
    final key = json['key'] as String?;
    final runId = json['runId'];
    final req = json['request'];
    if (key == null || key.isEmpty || runId is! num || req is! Map) return null;
    final reqMap = Map<String, dynamic>.from(req);
    return PluginApprovalRequest(
      key: key,
      runId: runId.toInt(),
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      deviceName: (json['deviceName'] as String?) ?? '',
      pluginName: (json['pluginName'] as String?) ?? '',
      action: (json['action'] as String?) ?? '',
      transport: (reqMap['transport'] as String?) ?? 'ssh',
      effect: (reqMap['effect'] as String?) ?? 'read',
      summary: (reqMap['summary'] ?? reqMap['command'] ?? '').toString(),
      reason: reqMap['reason'] as String?,
      expiresAt: (json['expiresAt'] as String?) ?? '',
    );
  }
}

// --- Fleet Apps & Batches (`PluginApp`, `FleetView`, `PluginBatchView`) ---

@immutable
class PluginApp {
  const PluginApp({
    required this.id,
    required this.slug,
    required this.title,
    this.icon = 'mdi-apps',
    this.description,
    this.members = 0,
  });

  final int id;
  final String slug;
  final String title;
  final String icon;
  final String? description;
  final int members;

  factory PluginApp.fromJson(Map<String, dynamic> json) {
    return PluginApp(
      id: (json['id'] as num?)?.toInt() ?? 0,
      slug: (json['slug'] as String?) ?? '',
      title: (json['title'] as String?) ?? '',
      icon: (json['icon'] as String?) ?? 'mdi-apps',
      description: json['description'] as String?,
      members: (json['members'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class FleetMember {
  const FleetMember({
    required this.deviceId,
    required this.deviceName,
    required this.ipAddress,
    this.platform,
    this.compatibility = 'likely',
    this.credentialsReady = true,
    this.missingTransport,
    this.status,
    this.settings,
    this.card = const <String, dynamic>{},
    this.error,
  });

  final int deviceId;
  final String deviceName;
  final String ipAddress;
  final String? platform;
  final String compatibility;
  final bool credentialsReady;
  final String? missingTransport;
  final String? status;
  final Map<String, dynamic>? settings;
  final Map<String, dynamic> card;
  final String? error;

  FleetMember copyWith({Map<String, dynamic>? settings}) {
    return FleetMember(
      deviceId: deviceId,
      deviceName: deviceName,
      ipAddress: ipAddress,
      platform: platform,
      compatibility: compatibility,
      credentialsReady: credentialsReady,
      missingTransport: missingTransport,
      status: status,
      settings: settings ?? this.settings,
      card: card,
      error: error,
    );
  }

  factory FleetMember.fromJson(Map<String, dynamic> json) {
    return FleetMember(
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      deviceName: (json['deviceName'] ?? json['name'] ?? '').toString(),
      ipAddress: (json['ipAddress'] as String?) ?? '',
      platform: json['platform'] as String?,
      compatibility: (json['compatibility'] as String?) ?? 'likely',
      credentialsReady: json['credentialsReady'] != false,
      missingTransport: json['missingTransport'] as String?,
      status: json['status'] as String?,
      settings: json['settings'] is Map
          ? Map<String, dynamic>.from(json['settings'] as Map)
          : null,
      card: json['card'] is Map
          ? Map<String, dynamic>.from(json['card'] as Map)
          : const <String, dynamic>{},
      error: json['error'] as String?,
    );
  }
}

@immutable
class FleetCandidate {
  const FleetCandidate({
    required this.deviceId,
    required this.deviceName,
    required this.ipAddress,
    this.compatibility = 'possible',
    this.reasons = const <String>[],
  });

  final int deviceId;
  final String deviceName;
  final String ipAddress;
  final String compatibility;
  final List<String> reasons;

  bool get isIncompatible => compatibility == 'incompatible';

  factory FleetCandidate.fromJson(Map<String, dynamic> json) {
    return FleetCandidate(
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      deviceName: (json['deviceName'] ?? json['name'] ?? '').toString(),
      ipAddress: (json['ipAddress'] as String?) ?? '',
      compatibility: (json['compatibility'] as String?) ?? 'possible',
      reasons: json['reasons'] is List
          ? (json['reasons'] as List)
                .map((e) => e.toString())
                .toList(growable: false)
          : const <String>[],
    );
  }
}

@immutable
class BatchDevice {
  const BatchDevice({
    required this.deviceId,
    required this.deviceName,
    required this.status,
    this.runId,
    this.output,
    this.error,
    this.missingTransport,
  });

  final int deviceId;
  final String deviceName;
  final String status;
  final int? runId;
  final Object? output;
  final String? error;
  final String? missingTransport;

  factory BatchDevice.fromJson(Map<String, dynamic> json) {
    return BatchDevice(
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      deviceName: (json['deviceName'] ?? json['name'] ?? '').toString(),
      status: (json['status'] as String?) ?? 'pending',
      runId: (json['runId'] as num?)?.toInt(),
      output: json['output'],
      error: json['error'] as String?,
      missingTransport: json['missingTransport'] as String?,
    );
  }
}

@immutable
class PluginBatchView {
  const PluginBatchView({
    required this.id,
    required this.pluginId,
    required this.action,
    required this.title,
    required this.status,
    this.devices = const <BatchDevice>[],
    this.result,
    this.hasPatch = false,
    this.error,
    this.createdAt = '',
    this.finishedAt,
  });

  final int id;
  final int pluginId;
  final String action;
  final String title;
  final String status;
  final List<BatchDevice> devices;
  final Object? result;
  final bool hasPatch;
  final String? error;
  final String createdAt;
  final String? finishedAt;

  bool get isRunning => status == 'running' || status == 'pending';

  factory PluginBatchView.fromJson(Map<String, dynamic> json) {
    return PluginBatchView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      pluginId: (json['pluginId'] as num?)?.toInt() ?? 0,
      action: (json['action'] as String?) ?? '',
      title: (json['title'] as String?) ?? (json['action'] as String?) ?? '',
      status: (json['status'] as String?) ?? 'running',
      devices: json['devices'] is List
          ? (json['devices'] as List)
                .whereType<Map>()
                .map((d) => BatchDevice.fromJson(Map<String, dynamic>.from(d)))
                .toList(growable: false)
          : const <BatchDevice>[],
      result: json['result'],
      hasPatch: json['hasPatch'] == true,
      error: json['error'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

@immutable
class FleetView {
  const FleetView({
    required this.plugin,
    this.settings,
    this.members = const <FleetMember>[],
    this.candidates = const <FleetCandidate>[],
    this.batches = const <PluginBatchView>[],
  });

  final PluginSummary plugin;
  final Map<String, dynamic>? settings;
  final List<FleetMember> members;
  final List<FleetCandidate> candidates;
  final List<PluginBatchView> batches;

  FleetView copyWith({
    Map<String, dynamic>? settings,
    List<FleetMember>? members,
    List<PluginBatchView>? batches,
  }) {
    return FleetView(
      plugin: plugin,
      settings: settings ?? this.settings,
      members: members ?? this.members,
      candidates: candidates,
      batches: batches ?? this.batches,
    );
  }

  factory FleetView.fromJson(Map<String, dynamic> json) {
    return FleetView(
      plugin: PluginSummary.fromJson(
        Map<String, dynamic>.from(
          (json['plugin'] as Map?) ?? const <String, dynamic>{},
        ),
      ),
      settings: json['settings'] is Map
          ? Map<String, dynamic>.from(json['settings'] as Map)
          : null,
      members: json['members'] is List
          ? (json['members'] as List)
                .whereType<Map>()
                .map((m) => FleetMember.fromJson(Map<String, dynamic>.from(m)))
                .toList(growable: false)
          : const <FleetMember>[],
      candidates: json['candidates'] is List
          ? (json['candidates'] as List)
                .whereType<Map>()
                .map(
                  (c) => FleetCandidate.fromJson(Map<String, dynamic>.from(c)),
                )
                .toList(growable: false)
          : const <FleetCandidate>[],
      batches: json['batches'] is List
          ? (json['batches'] as List)
                .whereType<Map>()
                .map(
                  (b) => PluginBatchView.fromJson(Map<String, dynamic>.from(b)),
                )
                .toList(growable: false)
          : const <PluginBatchView>[],
    );
  }
}

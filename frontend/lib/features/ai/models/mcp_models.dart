import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Configurações globais do servidor MCP (`GET /api/mcp/overview`, `PUT /api/mcp/settings`).
@immutable
class McpSettings {
  const McpSettings({
    this.enabled = true,
    this.allowPublicOrigin = false,
    this.endpointPath = '/mcp',
  });

  final bool enabled;
  final bool allowPublicOrigin;
  final String endpointPath;

  factory McpSettings.fromJson(Map<String, dynamic> json) {
    final rawPath = (json['endpointPath'] as String?)?.trim();
    return McpSettings(
      enabled: parseBool(json['enabled'], fallback: true),
      allowPublicOrigin: parseBool(json['allowPublicOrigin']),
      endpointPath: rawPath != null && rawPath.isNotEmpty ? rawPath : '/mcp',
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'enabled': enabled,
    'allowPublicOrigin': allowPublicOrigin,
  };

  McpSettings copyWith({
    bool? enabled,
    bool? allowPublicOrigin,
    String? endpointPath,
  }) {
    return McpSettings(
      enabled: enabled ?? this.enabled,
      allowPublicOrigin: allowPublicOrigin ?? this.allowPublicOrigin,
      endpointPath: endpointPath ?? this.endpointPath,
    );
  }
}

/// Definição de um escopo MCP disponível e se o papel atual pode concedê-lo.
@immutable
class McpScopeDescriptor {
  const McpScopeDescriptor({
    required this.id,
    required this.label,
    this.description = '',
    this.grantedByRole = true,
    this.requiresAdmin = false,
  });

  final String id;
  final String label;
  final String description;
  final bool grantedByRole;
  final bool requiresAdmin;

  factory McpScopeDescriptor.fromJson(Map<String, dynamic> json) {
    final id = (json['id'] as String?)?.trim() ?? '';
    final label = (json['label'] as String?)?.trim();
    return McpScopeDescriptor(
      id: id,
      label: label != null && label.isNotEmpty ? label : id,
      description: (json['description'] as String?)?.trim() ?? '',
      grantedByRole: parseBool(json['grantedByRole'], fallback: true),
      requiresAdmin: parseBool(json['requiresAdmin']),
    );
  }
}

/// Referência enxuta de aplicação de deploy retornada por `/api/mcp/overview`
/// para restringir tokens MCP sem acoplar `features/ai/` a `features/servers/` (ADR 017).
@immutable
class McpDeployApplicationRef {
  const McpDeployApplicationRef({
    required this.id,
    required this.name,
    this.slug = '',
    this.repository = '',
  });

  final int id;
  final String name;
  final String slug;
  final String repository;

  factory McpDeployApplicationRef.fromJson(Map<String, dynamic> json) {
    return McpDeployApplicationRef(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?)?.trim() ?? '',
      slug: (json['slug'] as String?)?.trim() ?? '',
      repository: (json['repository'] as String?)?.trim() ?? '',
    );
  }
}

/// Visão geral do servidor MCP (`GET /api/mcp/overview`).
@immutable
class McpOverview {
  const McpOverview({
    this.settings = const McpSettings(),
    this.scopes = const <McpScopeDescriptor>[],
    this.deployApplications = const <McpDeployApplicationRef>[],
    this.toolsCount = 0,
    this.canManageSettings = false,
  });

  final McpSettings settings;
  final List<McpScopeDescriptor> scopes;
  final List<McpDeployApplicationRef> deployApplications;
  final int toolsCount;
  final bool canManageSettings;

  McpScopeDescriptor? scopeById(String id) {
    for (final scope in scopes) {
      if (scope.id == id) return scope;
    }
    return null;
  }

  McpDeployApplicationRef? applicationById(int id) {
    for (final app in deployApplications) {
      if (app.id == id) return app;
    }
    return null;
  }

  McpOverview copyWith({
    McpSettings? settings,
    List<McpScopeDescriptor>? scopes,
    List<McpDeployApplicationRef>? deployApplications,
    int? toolsCount,
    bool? canManageSettings,
  }) {
    return McpOverview(
      settings: settings ?? this.settings,
      scopes: scopes ?? this.scopes,
      deployApplications: deployApplications ?? this.deployApplications,
      toolsCount: toolsCount ?? this.toolsCount,
      canManageSettings: canManageSettings ?? this.canManageSettings,
    );
  }

  factory McpOverview.fromJson(Map<String, dynamic> json) {
    return McpOverview(
      settings: McpSettings.fromJson(asJsonMap(json['settings'])),
      scopes: List<McpScopeDescriptor>.unmodifiable(
        asJsonList(json['scopes'], McpScopeDescriptor.fromJson),
      ),
      deployApplications: List<McpDeployApplicationRef>.unmodifiable(
        asJsonList(
          json['deployApplications'],
          McpDeployApplicationRef.fromJson,
        ),
      ),
      toolsCount: parseIntOrNull(json['toolsCount']) ?? 0,
      canManageSettings: parseBool(json['canManageSettings']),
    );
  }
}

/// Token pessoal de acesso ao servidor MCP (`GET /api/mcp/tokens`).
@immutable
class McpTokenView {
  const McpTokenView({
    required this.id,
    this.userId = 0,
    this.owner = '',
    required this.name,
    required this.hint,
    this.scopes = const <String>[],
    this.allowedApplicationIds,
    this.expiresAt,
    this.lastUsedAt,
    this.lastUsedIp,
    this.revoked = false,
    this.expired = false,
    this.createdAt,
  });

  final int id;
  final int userId;
  final String owner;
  final String name;
  final String hint;
  final List<String> scopes;
  final List<int>? allowedApplicationIds;
  final String? expiresAt;
  final String? lastUsedAt;
  final String? lastUsedIp;
  final bool revoked;
  final bool expired;
  final String? createdAt;

  bool get isActive => !revoked && !expired;

  factory McpTokenView.fromJson(Map<String, dynamic> json) {
    final rawAllowed = json['allowedApplicationIds'];
    final allowedIds = rawAllowed is List
        ? List<int>.unmodifiable(asIntList(rawAllowed))
        : null;
    final rawLastIp = (json['lastUsedIp'] as String?)?.trim();
    return McpTokenView(
      id: parseIntOrNull(json['id']) ?? 0,
      userId: parseIntOrNull(json['userId']) ?? 0,
      owner: (json['owner'] as String?)?.trim() ?? '',
      name: (json['name'] as String?)?.trim() ?? '',
      hint: (json['hint'] as String?)?.trim() ?? '',
      scopes: List<String>.unmodifiable(asStringList(json['scopes'])),
      allowedApplicationIds: allowedIds,
      expiresAt: json['expiresAt'] as String?,
      lastUsedAt: json['lastUsedAt'] as String?,
      lastUsedIp: rawLastIp != null && rawLastIp.isNotEmpty ? rawLastIp : null,
      revoked: parseBool(json['revoked']),
      expired: parseBool(json['expired']),
      createdAt: json['createdAt'] as String?,
    );
  }
}

/// Resposta de criação única de token MCP (`POST /api/mcp/tokens`).
@immutable
class McpTokenCreated {
  const McpTokenCreated({required this.token, required this.secret});

  final McpTokenView token;
  final String secret;

  factory McpTokenCreated.fromJson(Map<String, dynamic> json) {
    return McpTokenCreated(
      token: McpTokenView.fromJson(asJsonMap(json['token'])),
      secret: (json['secret'] as String?) ?? '',
    );
  }
}

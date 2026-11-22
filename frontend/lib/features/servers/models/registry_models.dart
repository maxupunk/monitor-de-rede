import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class RegistrySettings {
  const RegistrySettings({required this.keepVersions, required this.quotaGib});

  final int keepVersions;
  final int quotaGib;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'keepVersions': keepVersions,
    'quotaGib': quotaGib,
  };

  factory RegistrySettings.fromJson(Map<String, dynamic> json) {
    return RegistrySettings(
      keepVersions: parseIntOrNull(json['keepVersions']) ?? 10,
      quotaGib: parseIntOrNull(json['quotaGib']) ?? 20,
    );
  }
}

@immutable
class RegistryScanSummary {
  const RegistryScanSummary({
    required this.status,
    this.critical = 0,
    this.high = 0,
    this.medium = 0,
    this.low = 0,
    this.unknown = 0,
    this.scannedAt,
  });

  final String status; // 'done' | 'pending' | 'no_sbom' | 'failed'
  final int critical;
  final int high;
  final int medium;
  final int low;
  final int unknown;
  final String? scannedAt;

  factory RegistryScanSummary.fromJson(Map<String, dynamic> json) {
    return RegistryScanSummary(
      status: (json['status'] as String?) ?? 'pending',
      critical: parseIntOrNull(json['critical']) ?? 0,
      high: parseIntOrNull(json['high']) ?? 0,
      medium: parseIntOrNull(json['medium']) ?? 0,
      low: parseIntOrNull(json['low']) ?? 0,
      unknown: parseIntOrNull(json['unknown']) ?? 0,
      scannedAt: json['scannedAt'] as String?,
    );
  }
}

@immutable
class RegistryRepositoryView {
  const RegistryRepositoryView({
    required this.id,
    required this.name,
    this.applicationId,
    this.applicationName,
    required this.versions,
    required this.bytes,
    this.lastPush,
    this.latestScan,
  });

  final int id;
  final String name;
  final int? applicationId;
  final String? applicationName;
  final int versions;
  final int bytes;
  final String? lastPush;
  final RegistryScanSummary? latestScan;

  factory RegistryRepositoryView.fromJson(Map<String, dynamic> json) {
    final rawScan = json['latestScan'] ?? json['scan'];
    return RegistryRepositoryView(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      applicationId: parseIntOrNull(json['applicationId']),
      applicationName:
          json['applicationName'] as String? ?? json['application'] as String?,
      versions: parseIntOrNull(json['versions']) ?? 0,
      bytes: parseIntOrNull(json['bytes']) ?? 0,
      lastPush: json['lastPush'] as String? ?? json['lastPushedAt'] as String?,
      latestScan: rawScan is Map
          ? RegistryScanSummary.fromJson(asJsonMap(rawScan))
          : null,
    );
  }
}

@immutable
class RegistryVersionView {
  const RegistryVersionView({
    required this.id,
    required this.digest,
    required this.tags,
    required this.immutable,
    required this.mediaType,
    required this.platforms,
    required this.bytes,
    this.pushedBy,
    this.provenance,
    required this.pinned,
    required this.createdAt,
    this.scan,
    required this.usedBy,
    required this.protected,
  });

  final int id;
  final String digest;
  final List<String> tags;
  final bool immutable;
  final String mediaType;
  final List<String> platforms;
  final int bytes;
  final String? pushedBy;
  final Map<String, dynamic>? provenance;
  final bool pinned;
  final String createdAt;
  final RegistryScanSummary? scan;
  final List<String> usedBy;
  final bool protected;

  bool get canDelete => !protected && !pinned && usedBy.isEmpty;

  factory RegistryVersionView.fromJson(Map<String, dynamic> json) {
    return RegistryVersionView(
      id: parseIntOrNull(json['id']) ?? 0,
      digest: (json['digest'] as String?) ?? '',
      tags: List<String>.unmodifiable(asStringList(json['tags'])),
      immutable: parseBool(json['immutable']),
      mediaType: (json['mediaType'] as String?) ?? '',
      platforms: List<String>.unmodifiable(asStringList(json['platforms'])),
      bytes: parseIntOrNull(json['bytes']) ?? 0,
      pushedBy: json['pushedBy'] as String?,
      provenance: json['provenance'] is Map
          ? asJsonMap(json['provenance'])
          : null,
      pinned: parseBool(json['pinned']),
      createdAt: (json['createdAt'] as String?) ?? '',
      scan: json['scan'] is Map
          ? RegistryScanSummary.fromJson(asJsonMap(json['scan']))
          : null,
      usedBy: List<String>.unmodifiable(asStringList(json['usedBy'])),
      protected: parseBool(json['protected']),
    );
  }
}

@immutable
class RegistryRepositoryDetail {
  const RegistryRepositoryDetail({
    required this.repository,
    required this.versions,
  });

  final RegistryRepositoryView repository;
  final List<RegistryVersionView> versions;

  factory RegistryRepositoryDetail.fromJson(Map<String, dynamic> json) {
    return RegistryRepositoryDetail(
      repository: RegistryRepositoryView.fromJson(
        asJsonMap(json['repository']),
      ),
      versions: List<RegistryVersionView>.unmodifiable(
        asJsonList(json['versions'], RegistryVersionView.fromJson),
      ),
    );
  }
}

@immutable
class RegistryFindingView {
  const RegistryFindingView({
    required this.advisory,
    required this.aliases,
    required this.package,
    required this.ecosystem,
    required this.installed,
    this.fixed,
    required this.severity,
    this.summary,
    required this.excepted,
  });

  final String advisory;
  final List<String> aliases;
  final String package;
  final String ecosystem;
  final String installed;
  final String? fixed;
  final String severity;
  final String? summary;
  final bool excepted;

  factory RegistryFindingView.fromJson(Map<String, dynamic> json) {
    return RegistryFindingView(
      advisory: (json['advisory'] as String?) ?? '',
      aliases: List<String>.unmodifiable(asStringList(json['aliases'])),
      package: (json['package'] as String?) ?? '',
      ecosystem: (json['ecosystem'] as String?) ?? '',
      installed: (json['installed'] as String?) ?? '',
      fixed: json['fixed'] as String?,
      severity: (json['severity'] as String?) ?? 'unknown',
      summary: json['summary'] as String?,
      excepted: parseBool(json['excepted']),
    );
  }
}

@immutable
class RegistryFeedView {
  const RegistryFeedView({
    required this.ecosystem,
    required this.entries,
    this.syncedAt,
    this.error,
  });

  final String ecosystem;
  final int entries;
  final String? syncedAt;
  final String? error;

  factory RegistryFeedView.fromJson(Map<String, dynamic> json) {
    return RegistryFeedView(
      ecosystem: (json['ecosystem'] as String?) ?? '',
      entries: parseIntOrNull(json['entries']) ?? 0,
      syncedAt: json['syncedAt'] as String?,
      error: json['error'] as String?,
    );
  }
}

@immutable
class RegistryOverview {
  const RegistryOverview({
    required this.settings,
    required this.usedBytes,
    required this.quotaBytes,
    required this.repositories,
    required this.feeds,
  });

  final RegistrySettings settings;
  final int usedBytes;
  final int quotaBytes;
  final List<RegistryRepositoryView> repositories;
  final List<RegistryFeedView> feeds;

  double get usedPercent => quotaBytes > 0 ? (usedBytes / quotaBytes) * 100 : 0;

  factory RegistryOverview.fromJson(Map<String, dynamic> json) {
    return RegistryOverview(
      settings: json['settings'] is Map
          ? RegistrySettings.fromJson(asJsonMap(json['settings']))
          : const RegistrySettings(keepVersions: 10, quotaGib: 20),
      usedBytes: parseIntOrNull(json['usedBytes']) ?? 0,
      quotaBytes: parseIntOrNull(json['quotaBytes']) ?? 0,
      repositories: List<RegistryRepositoryView>.unmodifiable(
        asJsonList(json['repositories'], RegistryRepositoryView.fromJson),
      ),
      feeds: List<RegistryFeedView>.unmodifiable(
        asJsonList(json['feeds'], RegistryFeedView.fromJson),
      ),
    );
  }
}

@immutable
class RegistryTokenView {
  const RegistryTokenView({
    required this.id,
    required this.name,
    required this.owner,
    required this.hint,
    required this.repositories,
    required this.actions,
    this.expiresAt,
    this.lastUsedAt,
    this.lastUsedIp,
    required this.revoked,
    required this.expired,
    required this.createdAt,
  });

  final int id;
  final String name;
  final String owner;
  final String hint;
  final List<String> repositories;
  final List<String> actions;
  final String? expiresAt;
  final String? lastUsedAt;
  final String? lastUsedIp;
  final bool revoked;
  final bool expired;
  final String createdAt;

  factory RegistryTokenView.fromJson(Map<String, dynamic> json) {
    final scope = asJsonMap(json['scope']);
    return RegistryTokenView(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      owner: (json['owner'] as String?) ?? '',
      hint: (json['hint'] as String?) ?? '',
      repositories: List<String>.unmodifiable(
        asStringList(scope['repositories']),
      ),
      actions: List<String>.unmodifiable(asStringList(scope['actions'])),
      expiresAt: json['expiresAt'] as String?,
      lastUsedAt: json['lastUsedAt'] as String?,
      lastUsedIp: json['lastUsedIp'] as String?,
      revoked: parseBool(json['revoked']),
      expired: parseBool(json['expired']),
      createdAt: (json['createdAt'] as String?) ?? '',
    );
  }
}

@immutable
class RegistryTokenCreated {
  const RegistryTokenCreated({required this.token, required this.secret});

  final RegistryTokenView token;
  final String secret;

  factory RegistryTokenCreated.fromJson(Map<String, dynamic> json) {
    return RegistryTokenCreated(
      token: RegistryTokenView.fromJson(asJsonMap(json['token'])),
      secret: (json['secret'] as String?) ?? '',
    );
  }
}

@immutable
class RegistryExceptionView {
  const RegistryExceptionView({
    required this.id,
    this.applicationId,
    this.application,
    required this.advisory,
    required this.reason,
    required this.expiresAt,
    required this.expired,
    this.createdBy,
  });

  final int id;
  final int? applicationId;
  final String? application;
  final String advisory;
  final String reason;
  final String expiresAt;
  final bool expired;
  final String? createdBy;

  factory RegistryExceptionView.fromJson(Map<String, dynamic> json) {
    return RegistryExceptionView(
      id: parseIntOrNull(json['id']) ?? 0,
      applicationId: parseIntOrNull(json['applicationId']),
      application: json['application'] as String?,
      advisory: (json['advisory'] as String?) ?? '',
      reason: (json['reason'] as String?) ?? '',
      expiresAt: (json['expiresAt'] as String?) ?? '',
      expired: parseBool(json['expired']),
      createdBy: json['createdBy'] as String?,
    );
  }
}

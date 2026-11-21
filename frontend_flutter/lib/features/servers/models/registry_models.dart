import 'package:flutter/foundation.dart';

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
      keepVersions: (json['keepVersions'] as num?)?.toInt() ?? 10,
      quotaGib: (json['quotaGib'] as num?)?.toInt() ?? 20,
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
      critical: (json['critical'] as num?)?.toInt() ?? 0,
      high: (json['high'] as num?)?.toInt() ?? 0,
      medium: (json['medium'] as num?)?.toInt() ?? 0,
      low: (json['low'] as num?)?.toInt() ?? 0,
      unknown: (json['unknown'] as num?)?.toInt() ?? 0,
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
    return RegistryRepositoryView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      applicationId: (json['applicationId'] as num?)?.toInt(),
      applicationName:
          json['applicationName'] as String? ?? json['application'] as String?,
      versions: (json['versions'] as num?)?.toInt() ?? 0,
      bytes: (json['bytes'] as num?)?.toInt() ?? 0,
      lastPush: json['lastPush'] as String? ?? json['lastPushedAt'] as String?,
      latestScan: json['latestScan'] is Map
          ? RegistryScanSummary.fromJson(
              Map<String, dynamic>.from(json['latestScan'] as Map),
            )
          : (json['scan'] is Map
                ? RegistryScanSummary.fromJson(
                    Map<String, dynamic>.from(json['scan'] as Map),
                  )
                : null),
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
    List<String> strList(Object? raw) => raw is List
        ? List<String>.unmodifiable(raw.whereType<String>())
        : const <String>[];
    return RegistryVersionView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      digest: (json['digest'] as String?) ?? '',
      tags: strList(json['tags']),
      immutable: json['immutable'] == true,
      mediaType: (json['mediaType'] as String?) ?? '',
      platforms: strList(json['platforms']),
      bytes: (json['bytes'] as num?)?.toInt() ?? 0,
      pushedBy: json['pushedBy'] as String?,
      provenance: json['provenance'] is Map
          ? Map<String, dynamic>.from(json['provenance'] as Map)
          : null,
      pinned: json['pinned'] == true,
      createdAt: (json['createdAt'] as String?) ?? '',
      scan: json['scan'] is Map
          ? RegistryScanSummary.fromJson(
              Map<String, dynamic>.from(json['scan'] as Map),
            )
          : null,
      usedBy: strList(json['usedBy']),
      protected: json['protected'] == true,
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
    final rawVersions = json['versions'];
    return RegistryRepositoryDetail(
      repository: RegistryRepositoryView.fromJson(
        Map<String, dynamic>.from(json['repository'] as Map),
      ),
      versions: rawVersions is List
          ? List<RegistryVersionView>.unmodifiable(
              rawVersions.whereType<Map>().map(
                (e) =>
                    RegistryVersionView.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <RegistryVersionView>[],
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
    final rawAliases = json['aliases'];
    return RegistryFindingView(
      advisory: (json['advisory'] as String?) ?? '',
      aliases: rawAliases is List
          ? List<String>.unmodifiable(rawAliases.whereType<String>())
          : const <String>[],
      package: (json['package'] as String?) ?? '',
      ecosystem: (json['ecosystem'] as String?) ?? '',
      installed: (json['installed'] as String?) ?? '',
      fixed: json['fixed'] as String?,
      severity: (json['severity'] as String?) ?? 'unknown',
      summary: json['summary'] as String?,
      excepted: json['excepted'] == true,
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
      entries: (json['entries'] as num?)?.toInt() ?? 0,
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
    final rawRepos = json['repositories'];
    final rawFeeds = json['feeds'];
    return RegistryOverview(
      settings: json['settings'] is Map
          ? RegistrySettings.fromJson(
              Map<String, dynamic>.from(json['settings'] as Map),
            )
          : const RegistrySettings(keepVersions: 10, quotaGib: 20),
      usedBytes: (json['usedBytes'] as num?)?.toInt() ?? 0,
      quotaBytes: (json['quotaBytes'] as num?)?.toInt() ?? 0,
      repositories: rawRepos is List
          ? List<RegistryRepositoryView>.unmodifiable(
              rawRepos.whereType<Map>().map(
                (e) => RegistryRepositoryView.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : const <RegistryRepositoryView>[],
      feeds: rawFeeds is List
          ? List<RegistryFeedView>.unmodifiable(
              rawFeeds.whereType<Map>().map(
                (e) => RegistryFeedView.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <RegistryFeedView>[],
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
    final scope = json['scope'] is Map
        ? Map<String, dynamic>.from(json['scope'] as Map)
        : const <String, dynamic>{};
    List<String> strList(Object? raw) => raw is List
        ? List<String>.unmodifiable(raw.whereType<String>())
        : const <String>[];
    return RegistryTokenView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      owner: (json['owner'] as String?) ?? '',
      hint: (json['hint'] as String?) ?? '',
      repositories: strList(scope['repositories']),
      actions: strList(scope['actions']),
      expiresAt: json['expiresAt'] as String?,
      lastUsedAt: json['lastUsedAt'] as String?,
      lastUsedIp: json['lastUsedIp'] as String?,
      revoked: json['revoked'] == true,
      expired: json['expired'] == true,
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
      token: RegistryTokenView.fromJson(
        Map<String, dynamic>.from(json['token'] as Map),
      ),
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
      id: (json['id'] as num?)?.toInt() ?? 0,
      applicationId: (json['applicationId'] as num?)?.toInt(),
      application: json['application'] as String?,
      advisory: (json['advisory'] as String?) ?? '',
      reason: (json['reason'] as String?) ?? '',
      expiresAt: (json['expiresAt'] as String?) ?? '',
      expired: json['expired'] == true,
      createdBy: json['createdBy'] as String?,
    );
  }
}

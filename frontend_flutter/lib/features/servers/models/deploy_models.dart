import 'package:flutter/foundation.dart';

@immutable
class DeployAgentView {
  const DeployAgentView({
    required this.id,
    required this.name,
    required this.connected,
    required this.allowsDeploy,
    required this.allowsBuild,
  });

  final int id;
  final String name;
  final bool connected;
  final bool allowsDeploy;
  final bool allowsBuild;

  factory DeployAgentView.fromJson(Map<String, dynamic> json) {
    return DeployAgentView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      connected: json['connected'] == true,
      allowsDeploy: json['allowsDeploy'] == true,
      allowsBuild: json['allowsBuild'] == true,
    );
  }
}

@immutable
class DeployBuildImage {
  const DeployBuildImage({
    required this.name,
    required this.context,
    required this.dockerfile,
    required this.image,
    this.target,
  });

  final String name;
  final String context;
  final String dockerfile;
  final String image;
  final String? target;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'context': context,
    'dockerfile': dockerfile,
    'image': image,
    if (target != null && target!.isNotEmpty) 'target': target,
  };

  factory DeployBuildImage.fromJson(Map<String, dynamic> json) {
    return DeployBuildImage(
      name: (json['name'] as String?) ?? '',
      context: (json['context'] as String?) ?? '.',
      dockerfile: (json['dockerfile'] as String?) ?? 'Dockerfile',
      image: (json['image'] as String?) ?? '',
      target: json['target'] as String?,
    );
  }
}

@immutable
class DeployBuildArg {
  const DeployBuildArg({required this.name, required this.value});
  final String name;
  final String value;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'value': value,
  };

  factory DeployBuildArg.fromJson(Map<String, dynamic> json) {
    return DeployBuildArg(
      name: (json['name'] as String?) ?? '',
      value: (json['value'] as String?) ?? '',
    );
  }
}

@immutable
class DeployEnvVersionView {
  const DeployEnvVersionView({
    required this.id,
    required this.targetId,
    required this.keys,
    this.createdBy,
    this.note,
    required this.createdAt,
    required this.current,
  });

  final int id;
  final int targetId;
  final List<String> keys;
  final String? createdBy;
  final String? note;
  final String createdAt;
  final bool current;

  factory DeployEnvVersionView.fromJson(Map<String, dynamic> json) {
    final rawKeys = json['keys'];
    return DeployEnvVersionView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      targetId: (json['targetId'] as num?)?.toInt() ?? 0,
      keys: rawKeys is List
          ? List<String>.unmodifiable(rawKeys.whereType<String>())
          : const <String>[],
      createdBy: json['createdBy'] as String?,
      note: json['note'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      current: json['current'] == true,
    );
  }
}

@immutable
class DeployRunView {
  const DeployRunView({
    required this.id,
    required this.applicationId,
    required this.applicationName,
    required this.repository,
    required this.targetId,
    required this.targetName,
    required this.trigger,
    this.triggeredBy,
    this.reference,
    required this.commitSha,
    required this.shortSha,
    this.commitMessage,
    required this.imageTag,
    this.envVersionId,
    required this.status,
    this.stage,
    this.rollbackOfRunId,
    this.afterRunId,
    this.backupId,
    this.approvedBy,
    this.approvedAt,
    this.scheduledFor,
    this.error,
    required this.createdAt,
    this.startedAt,
    this.finishedAt,
    this.durationMs,
    this.buildId,
  });

  final int id;
  final int applicationId;
  final String applicationName;
  final String repository;
  final int targetId;
  final String targetName;
  final String trigger;
  final String? triggeredBy;
  final String? reference;
  final String commitSha;
  final String shortSha;
  final String? commitMessage;
  final String imageTag;
  final int? envVersionId;
  final String status;
  final String? stage;
  final int? rollbackOfRunId;
  final int? afterRunId;
  final int? backupId;
  final String? approvedBy;
  final String? approvedAt;
  final String? scheduledFor;
  final String? error;
  final String createdAt;
  final String? startedAt;
  final String? finishedAt;
  final int? durationMs;
  final int? buildId;

  static DeployRunView? tryFromJson(Map<String, dynamic> json) {
    if (json['id'] is! num ||
        json['targetId'] is! num ||
        json['status'] is! String ||
        json['commitSha'] is! String) {
      return null;
    }
    return DeployRunView.fromJson(json);
  }

  factory DeployRunView.fromJson(Map<String, dynamic> json) {
    final sha = (json['commitSha'] as String?) ?? '';
    return DeployRunView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      applicationId: (json['applicationId'] as num?)?.toInt() ?? 0,
      applicationName: (json['applicationName'] as String?) ?? '',
      repository: (json['repository'] as String?) ?? '',
      targetId: (json['targetId'] as num?)?.toInt() ?? 0,
      targetName: (json['targetName'] as String?) ?? '',
      trigger: (json['trigger'] as String?) ?? 'manual',
      triggeredBy: json['triggeredBy'] as String?,
      reference: json['reference'] as String?,
      commitSha: sha,
      shortSha:
          (json['shortSha'] as String?) ??
          (sha.length >= 7 ? sha.substring(0, 7) : sha),
      commitMessage: json['commitMessage'] as String?,
      imageTag: (json['imageTag'] as String?) ?? '',
      envVersionId: (json['envVersionId'] as num?)?.toInt(),
      status: (json['status'] as String?) ?? 'queued',
      stage: json['stage'] as String?,
      rollbackOfRunId: (json['rollbackOfRunId'] as num?)?.toInt(),
      afterRunId: (json['afterRunId'] as num?)?.toInt(),
      backupId: (json['backupId'] as num?)?.toInt(),
      approvedBy: json['approvedBy'] as String?,
      approvedAt: json['approvedAt'] as String?,
      scheduledFor: json['scheduledFor'] as String?,
      error: json['error'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      startedAt: json['startedAt'] as String?,
      finishedAt: json['finishedAt'] as String?,
      durationMs: (json['durationMs'] as num?)?.toInt(),
      buildId: (json['buildId'] as num?)?.toInt(),
    );
  }
}

@immutable
class DeployTargetView {
  const DeployTargetView({
    required this.id,
    required this.applicationId,
    required this.name,
    this.agent,
    required this.project,
    required this.reference,
    required this.requiresApproval,
    this.windowStart,
    this.windowEnd,
    this.backupConnectionId,
    this.backupConnectionName,
    required this.waitTimeoutSecs,
    required this.autoRollback,
    required this.keepImages,
    required this.enabled,
    required this.position,
    required this.vulnerabilityPolicy,
    required this.requireProvenance,
    this.live,
    this.pending,
    this.env,
  });

  final int id;
  final int applicationId;
  final String name;
  final DeployAgentView? agent;
  final String project;
  final String reference;
  final bool requiresApproval;
  final String? windowStart;
  final String? windowEnd;
  final int? backupConnectionId;
  final String? backupConnectionName;
  final int waitTimeoutSecs;
  final bool autoRollback;
  final int keepImages;
  final bool enabled;
  final int position;
  final String vulnerabilityPolicy;
  final bool requireProvenance;
  final DeployRunView? live;
  final DeployRunView? pending;
  final DeployEnvVersionView? env;

  DeployTargetView copyWith({
    DeployRunView? live,
    DeployRunView? pending,
    bool clearPending = false,
  }) {
    return DeployTargetView(
      id: id,
      applicationId: applicationId,
      name: name,
      agent: agent,
      project: project,
      reference: reference,
      requiresApproval: requiresApproval,
      windowStart: windowStart,
      windowEnd: windowEnd,
      backupConnectionId: backupConnectionId,
      backupConnectionName: backupConnectionName,
      waitTimeoutSecs: waitTimeoutSecs,
      autoRollback: autoRollback,
      keepImages: keepImages,
      enabled: enabled,
      position: position,
      vulnerabilityPolicy: vulnerabilityPolicy,
      requireProvenance: requireProvenance,
      live: live ?? this.live,
      pending: clearPending ? null : (pending ?? this.pending),
      env: env,
    );
  }

  factory DeployTargetView.fromJson(Map<String, dynamic> json) {
    return DeployTargetView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      applicationId: (json['applicationId'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      agent: json['agent'] is Map
          ? DeployAgentView.fromJson(
              Map<String, dynamic>.from(json['agent'] as Map),
            )
          : null,
      project: (json['project'] as String?) ?? '',
      reference: (json['reference'] as String?) ?? 'main',
      requiresApproval: json['requiresApproval'] == true,
      windowStart: json['windowStart'] as String?,
      windowEnd: json['windowEnd'] as String?,
      backupConnectionId: (json['backupConnectionId'] as num?)?.toInt(),
      backupConnectionName: json['backupConnectionName'] as String?,
      waitTimeoutSecs: (json['waitTimeoutSecs'] as num?)?.toInt() ?? 180,
      autoRollback: json['autoRollback'] != false,
      keepImages: (json['keepImages'] as num?)?.toInt() ?? 3,
      enabled: json['enabled'] != false,
      position: (json['position'] as num?)?.toInt() ?? 0,
      vulnerabilityPolicy:
          (json['vulnerabilityPolicy'] as String?) ?? 'block_critical',
      requireProvenance: json['requireProvenance'] == true,
      live: json['live'] is Map
          ? DeployRunView.fromJson(
              Map<String, dynamic>.from(json['live'] as Map),
            )
          : null,
      pending: json['pending'] is Map
          ? DeployRunView.fromJson(
              Map<String, dynamic>.from(json['pending'] as Map),
            )
          : null,
      env: json['env'] is Map
          ? DeployEnvVersionView.fromJson(
              Map<String, dynamic>.from(json['env'] as Map),
            )
          : null,
    );
  }
}

@immutable
class DeployApplicationView {
  const DeployApplicationView({
    required this.id,
    required this.name,
    required this.slug,
    required this.repository,
    required this.repositoryUrl,
    this.credentialId,
    this.credentialName,
    this.githubConnectionId,
    this.githubAccount,
    required this.composeFiles,
    required this.imageTagVariable,
    required this.images,
    required this.rollout,
    required this.builder,
    this.buildProbeId,
    this.testTarget,
    required this.buildArgs,
    required this.targets,
    required this.createdAt,
  });

  final int id;
  final String name;
  final String slug;
  final String repository;
  final String repositoryUrl;
  final int? credentialId;
  final String? credentialName;
  final int? githubConnectionId;
  final String? githubAccount;
  final List<String> composeFiles;
  final String imageTagVariable;
  final List<DeployBuildImage> images;
  final String rollout; // 'sequential' | 'parallel'
  final String builder; // 'netmonitor' | 'github_actions'
  final int? buildProbeId;
  final String? testTarget;
  final List<DeployBuildArg> buildArgs;
  final List<DeployTargetView> targets;
  final String createdAt;

  DeployApplicationView copyWith({List<DeployTargetView>? targets}) {
    return DeployApplicationView(
      id: id,
      name: name,
      slug: slug,
      repository: repository,
      repositoryUrl: repositoryUrl,
      credentialId: credentialId,
      credentialName: credentialName,
      githubConnectionId: githubConnectionId,
      githubAccount: githubAccount,
      composeFiles: composeFiles,
      imageTagVariable: imageTagVariable,
      images: images,
      rollout: rollout,
      builder: builder,
      buildProbeId: buildProbeId,
      testTarget: testTarget,
      buildArgs: buildArgs,
      targets: targets ?? this.targets,
      createdAt: createdAt,
    );
  }

  factory DeployApplicationView.fromJson(Map<String, dynamic> json) {
    final rawFiles = json['composeFiles'];
    final rawImages = json['images'];
    final rawArgs = json['buildArgs'];
    final rawTargets = json['targets'];
    return DeployApplicationView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      slug: (json['slug'] as String?) ?? '',
      repository: (json['repository'] as String?) ?? '',
      repositoryUrl: (json['repositoryUrl'] as String?) ?? '',
      credentialId: (json['credentialId'] as num?)?.toInt(),
      credentialName: json['credentialName'] as String?,
      githubConnectionId: (json['githubConnectionId'] as num?)?.toInt(),
      githubAccount: json['githubAccount'] as String?,
      composeFiles: rawFiles is List
          ? List<String>.unmodifiable(rawFiles.whereType<String>())
          : const <String>['docker-compose.yml'],
      imageTagVariable: (json['imageTagVariable'] as String?) ?? 'IMAGE_TAG',
      images: rawImages is List
          ? List<DeployBuildImage>.unmodifiable(
              rawImages.whereType<Map>().map(
                (e) => DeployBuildImage.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployBuildImage>[],
      rollout: (json['rollout'] as String?) ?? 'sequential',
      builder: (json['builder'] as String?) ?? 'netmonitor',
      buildProbeId: (json['buildProbeId'] as num?)?.toInt(),
      testTarget: json['testTarget'] as String?,
      buildArgs: rawArgs is List
          ? List<DeployBuildArg>.unmodifiable(
              rawArgs.whereType<Map>().map(
                (e) => DeployBuildArg.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployBuildArg>[],
      targets: rawTargets is List
          ? List<DeployTargetView>.unmodifiable(
              rawTargets.whereType<Map>().map(
                (e) => DeployTargetView.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployTargetView>[],
      createdAt: (json['createdAt'] as String?) ?? '',
    );
  }
}

@immutable
class DeployCounts {
  const DeployCounts({
    this.applications = 0,
    this.targets = 0,
    this.running = 0,
    this.awaitingApproval = 0,
    this.succeeded24h = 0,
    this.failed24h = 0,
  });

  final int applications;
  final int targets;
  final int running;
  final int awaitingApproval;
  final int succeeded24h;
  final int failed24h;

  factory DeployCounts.fromJson(Map<String, dynamic> json) {
    return DeployCounts(
      applications: (json['applications'] as num?)?.toInt() ?? 0,
      targets: (json['targets'] as num?)?.toInt() ?? 0,
      running: (json['running'] as num?)?.toInt() ?? 0,
      awaitingApproval: (json['awaitingApproval'] as num?)?.toInt() ?? 0,
      succeeded24h: (json['succeeded24h'] as num?)?.toInt() ?? 0,
      failed24h: (json['failed24h'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class DeployCredentialView {
  const DeployCredentialView({
    required this.id,
    required this.name,
    this.registry,
    required this.username,
    required this.hasSecret,
    required this.createdAt,
  });

  final int id;
  final String name;
  final String? registry;
  final String username;
  final bool hasSecret;
  final String createdAt;

  factory DeployCredentialView.fromJson(Map<String, dynamic> json) {
    return DeployCredentialView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      registry: json['registry'] as String?,
      username: (json['username'] as String?) ?? '',
      hasSecret: json['hasSecret'] == true,
      createdAt: (json['createdAt'] as String?) ?? '',
    );
  }
}

@immutable
class DeployGithubConnectionView {
  const DeployGithubConnectionView({
    required this.id,
    required this.account,
    this.organization,
    required this.appSlug,
    required this.installed,
    required this.canWrite,
    required this.installUrl,
    required this.settingsUrl,
    required this.createdAt,
  });

  final int id;
  final String account;
  final String? organization;
  final String appSlug;
  final bool installed;
  final bool canWrite;
  final String installUrl;
  final String settingsUrl;
  final String createdAt;

  factory DeployGithubConnectionView.fromJson(Map<String, dynamic> json) {
    return DeployGithubConnectionView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      account: (json['account'] as String?) ?? '',
      organization: json['organization'] as String?,
      appSlug: (json['appSlug'] as String?) ?? '',
      installed: json['installed'] == true,
      canWrite: json['canWrite'] == true,
      installUrl: (json['installUrl'] as String?) ?? '',
      settingsUrl: (json['settingsUrl'] as String?) ?? '',
      createdAt: (json['createdAt'] as String?) ?? '',
    );
  }
}

@immutable
class DeployGuideAction {
  const DeployGuideAction({
    required this.kind,
    this.applicationId,
    this.targetId,
    this.runId,
    this.url,
  });

  final String kind;
  final int? applicationId;
  final int? targetId;
  final int? runId;
  final String? url;

  factory DeployGuideAction.fromJson(Map<String, dynamic> json) {
    return DeployGuideAction(
      kind: (json['kind'] as String?) ?? '',
      applicationId: (json['applicationId'] as num?)?.toInt(),
      targetId: (json['targetId'] as num?)?.toInt(),
      runId: (json['runId'] as num?)?.toInt(),
      url: json['url'] as String?,
    );
  }
}

@immutable
class DeployGuideStep {
  const DeployGuideStep({
    required this.key,
    required this.title,
    required this.detail,
    required this.status,
    this.action,
  });

  final String key;
  final String title;
  final String detail;
  final String status; // 'done' | 'current' | 'todo' | 'optional'
  final DeployGuideAction? action;

  factory DeployGuideStep.fromJson(Map<String, dynamic> json) {
    return DeployGuideStep(
      key: (json['key'] as String?) ?? '',
      title: (json['title'] as String?) ?? '',
      detail: (json['detail'] as String?) ?? '',
      status: (json['status'] as String?) ?? 'todo',
      action: json['action'] is Map
          ? DeployGuideAction.fromJson(
              Map<String, dynamic>.from(json['action'] as Map),
            )
          : null,
    );
  }
}

@immutable
class DeployGuideView {
  const DeployGuideView({required this.complete, required this.steps});

  final bool complete;
  final List<DeployGuideStep> steps;

  factory DeployGuideView.fromJson(Map<String, dynamic> json) {
    final rawSteps = json['steps'];
    return DeployGuideView(
      complete: json['complete'] == true,
      steps: rawSteps is List
          ? List<DeployGuideStep>.unmodifiable(
              rawSteps.whereType<Map>().map(
                (e) => DeployGuideStep.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployGuideStep>[],
    );
  }
}

@immutable
class DeployOverview {
  const DeployOverview({
    required this.applications,
    required this.recentRuns,
    required this.counts,
    required this.credentials,
    required this.githubConnections,
    required this.hookPath,
    required this.oidcAudience,
    required this.guide,
  });

  final List<DeployApplicationView> applications;
  final List<DeployRunView> recentRuns;
  final DeployCounts counts;
  final List<DeployCredentialView> credentials;
  final List<DeployGithubConnectionView> githubConnections;
  final String hookPath;
  final String oidcAudience;
  final DeployGuideView guide;

  DeployOverview copyWith({List<DeployApplicationView>? applications}) {
    return DeployOverview(
      applications: applications ?? this.applications,
      recentRuns: recentRuns,
      counts: counts,
      credentials: credentials,
      githubConnections: githubConnections,
      hookPath: hookPath,
      oidcAudience: oidcAudience,
      guide: guide,
    );
  }

  factory DeployOverview.fromJson(Map<String, dynamic> json) {
    List<T> parseList<T>(Object? raw, T Function(Map<String, dynamic>) fn) {
      if (raw is! List) return const [];
      return List<T>.unmodifiable(
        raw.whereType<Map>().map((e) => fn(Map<String, dynamic>.from(e))),
      );
    }

    return DeployOverview(
      applications: parseList(
        json['applications'],
        DeployApplicationView.fromJson,
      ),
      recentRuns: parseList(json['recentRuns'], DeployRunView.fromJson),
      counts: json['counts'] is Map
          ? DeployCounts.fromJson(
              Map<String, dynamic>.from(json['counts'] as Map),
            )
          : const DeployCounts(),
      credentials: parseList(
        json['credentials'],
        DeployCredentialView.fromJson,
      ),
      githubConnections: parseList(
        json['githubConnections'],
        DeployGithubConnectionView.fromJson,
      ),
      hookPath: (json['hookPath'] as String?) ?? '/api/deploy-hooks/github',
      oidcAudience: (json['oidcAudience'] as String?) ?? 'netmonitor',
      guide: json['guide'] is Map
          ? DeployGuideView.fromJson(
              Map<String, dynamic>.from(json['guide'] as Map),
            )
          : const DeployGuideView(complete: false, steps: <DeployGuideStep>[]),
    );
  }
}

@immutable
class DeployStoredImage {
  const DeployStoredImage({
    required this.repository,
    required this.shortDigest,
    required this.bytes,
  });

  final String repository;
  final String shortDigest;
  final int bytes;

  factory DeployStoredImage.fromJson(Map<String, dynamic> json) {
    return DeployStoredImage(
      repository: (json['repository'] as String?) ?? '',
      shortDigest:
          (json['shortDigest'] as String?) ?? (json['digest'] as String?) ?? '',
      bytes: (json['bytes'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class DeployServerBuildView {
  const DeployServerBuildView({
    required this.id,
    required this.applicationId,
    required this.application,
    required this.reference,
    required this.commitSha,
    required this.shortSha,
    this.commitMessage,
    required this.trigger,
    this.triggeredBy,
    this.agent,
    required this.status,
    this.stage,
    this.currentImage,
    required this.noCache,
    required this.images,
    this.error,
    this.startedAt,
    this.finishedAt,
    this.durationMs,
    required this.createdAt,
  });

  final int id;
  final int applicationId;
  final String application;
  final String reference;
  final String commitSha;
  final String shortSha;
  final String? commitMessage;
  final String trigger;
  final String? triggeredBy;
  final String? agent;
  final String status;
  final String? stage;
  final String? currentImage;
  final bool noCache;
  final List<DeployStoredImage> images;
  final String? error;
  final String? startedAt;
  final String? finishedAt;
  final int? durationMs;
  final String createdAt;

  static DeployServerBuildView? tryFromJson(Map<String, dynamic> json) {
    if (json['id'] is! num ||
        json['applicationId'] is! num ||
        json['status'] is! String ||
        json['commitSha'] is! String) {
      return null;
    }
    return DeployServerBuildView.fromJson(json);
  }

  factory DeployServerBuildView.fromJson(Map<String, dynamic> json) {
    final sha = (json['commitSha'] as String?) ?? '';
    final rawImages = json['images'];
    return DeployServerBuildView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      applicationId: (json['applicationId'] as num?)?.toInt() ?? 0,
      application: (json['application'] as String?) ?? '',
      reference: (json['reference'] as String?) ?? '',
      commitSha: sha,
      shortSha:
          (json['shortSha'] as String?) ??
          (sha.length >= 7 ? sha.substring(0, 7) : sha),
      commitMessage: json['commitMessage'] as String?,
      trigger: (json['trigger'] as String?) ?? 'manual',
      triggeredBy: json['triggeredBy'] as String?,
      agent: json['agent'] as String?,
      status: (json['status'] as String?) ?? 'queued',
      stage: json['stage'] as String?,
      currentImage: json['currentImage'] as String?,
      noCache: json['noCache'] == true,
      images: rawImages is List
          ? List<DeployStoredImage>.unmodifiable(
              rawImages.whereType<Map>().map(
                (e) => DeployStoredImage.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployStoredImage>[],
      error: json['error'] as String?,
      startedAt: json['startedAt'] as String?,
      finishedAt: json['finishedAt'] as String?,
      durationMs: (json['durationMs'] as num?)?.toInt(),
      createdAt: (json['createdAt'] as String?) ?? '',
    );
  }
}

@immutable
class DeployBuildSecretView {
  const DeployBuildSecretView({required this.name, required this.updatedAt});

  final String name;
  final String updatedAt;

  factory DeployBuildSecretView.fromJson(Map<String, dynamic> json) {
    return DeployBuildSecretView(
      name: (json['name'] as String?) ?? '',
      updatedAt: (json['updatedAt'] as String?) ?? '',
    );
  }
}

@immutable
class BuilderReadiness {
  const BuilderReadiness({
    this.buildxVersion,
    required this.builderReady,
    this.gitVersion,
    this.freeBytes = 0,
    this.cpus = 0,
    this.cpuLimit,
    this.memoryLimit,
  });

  final String? buildxVersion;
  final bool builderReady;
  final String? gitVersion;
  final int freeBytes;
  final int cpus;
  final String? cpuLimit;
  final String? memoryLimit;

  factory BuilderReadiness.fromJson(Map<String, dynamic> json) {
    return BuilderReadiness(
      buildxVersion: json['buildxVersion'] as String?,
      builderReady: json['builderReady'] == true,
      gitVersion: json['gitVersion'] as String?,
      freeBytes: (json['freeBytes'] as num?)?.toInt() ?? 0,
      cpus: (json['cpus'] as num?)?.toInt() ?? 0,
      cpuLimit: json['cpuLimit']?.toString(),
      memoryLimit: json['memoryLimit']?.toString(),
    );
  }
}

@immutable
class DeployReadinessCheck {
  const DeployReadinessCheck({
    required this.level,
    required this.title,
    this.detail,
  });

  final String level; // 'ok' | 'warning' | 'error'
  final String title;
  final String? detail;

  factory DeployReadinessCheck.fromJson(Map<String, dynamic> json) {
    return DeployReadinessCheck(
      level: (json['level'] as String?) ?? 'ok',
      title: (json['title'] as String?) ?? (json['message'] as String?) ?? '',
      detail: json['detail'] as String?,
    );
  }
}

@immutable
class DeployReadinessView {
  const DeployReadinessView({required this.ready, required this.checks});

  final bool ready;
  final List<DeployReadinessCheck> checks;

  factory DeployReadinessView.fromJson(Map<String, dynamic> json) {
    final rawChecks = json['checks'];
    return DeployReadinessView(
      ready: json['ready'] == true,
      checks: rawChecks is List
          ? List<DeployReadinessCheck>.unmodifiable(
              rawChecks.whereType<Map>().map(
                (e) =>
                    DeployReadinessCheck.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployReadinessCheck>[],
    );
  }
}

@immutable
class DeployEnvContentView {
  const DeployEnvContentView({this.id, required this.content});
  final int? id;
  final String content;

  factory DeployEnvContentView.fromJson(Map<String, dynamic> json) {
    return DeployEnvContentView(
      id: (json['id'] as num?)?.toInt(),
      content: (json['content'] as String?) ?? '',
    );
  }
}

@immutable
class DeployEnvDiff {
  const DeployEnvDiff({
    required this.added,
    required this.changed,
    required this.removed,
  });

  final List<String> added;
  final List<String> changed;
  final List<String> removed;

  bool get isEmpty => added.isEmpty && changed.isEmpty && removed.isEmpty;

  factory DeployEnvDiff.fromJson(Map<String, dynamic> json) {
    List<String> list(Object? v) => v is List
        ? List<String>.unmodifiable(v.whereType<String>())
        : const <String>[];
    return DeployEnvDiff(
      added: list(json['added']),
      changed: list(json['changed']),
      removed: list(json['removed']),
    );
  }
}

@immutable
class DeployEnvSaved {
  const DeployEnvSaved({required this.version, required this.diff, this.run});

  final DeployEnvVersionView version;
  final DeployEnvDiff diff;
  final DeployRunView? run;

  factory DeployEnvSaved.fromJson(Map<String, dynamic> json) {
    return DeployEnvSaved(
      version: DeployEnvVersionView.fromJson(
        Map<String, dynamic>.from(json['version'] as Map),
      ),
      diff: DeployEnvDiff.fromJson(
        Map<String, dynamic>.from(json['diff'] as Map),
      ),
      run: json['run'] is Map
          ? DeployRunView.fromJson(
              Map<String, dynamic>.from(json['run'] as Map),
            )
          : null,
    );
  }
}

@immutable
class DeployCommitView {
  const DeployCommitView({
    required this.sha,
    required this.shortSha,
    required this.message,
    this.author,
    this.date,
    this.url,
  });

  final String sha;
  final String shortSha;
  final String message;
  final String? author;
  final String? date;
  final String? url;

  factory DeployCommitView.fromJson(Map<String, dynamic> json) {
    final sha = (json['sha'] as String?) ?? '';
    return DeployCommitView(
      sha: sha,
      shortSha:
          (json['shortSha'] as String?) ??
          (sha.length >= 7 ? sha.substring(0, 7) : sha),
      message: (json['message'] as String?) ?? '',
      author: json['author'] as String?,
      date: json['date'] as String?,
      url: json['url'] as String?,
    );
  }
}

@immutable
class DeployChangesView {
  const DeployChangesView({
    this.baseSha,
    this.headSha,
    this.compareUrl,
    required this.commits,
    this.total = 0,
    this.unavailableReason,
  });

  final String? baseSha;
  final String? headSha;
  final String? compareUrl;
  final List<DeployCommitView> commits;
  final int total;
  final String? unavailableReason;

  factory DeployChangesView.fromJson(Map<String, dynamic> json) {
    final rawCommits = json['commits'];
    return DeployChangesView(
      baseSha: json['baseSha'] as String?,
      headSha: json['headSha'] as String?,
      compareUrl: json['compareUrl'] as String?,
      commits: rawCommits is List
          ? List<DeployCommitView>.unmodifiable(
              rawCommits.whereType<Map>().map(
                (e) => DeployCommitView.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployCommitView>[],
      total: (json['total'] as num?)?.toInt() ?? 0,
      unavailableReason: json['unavailableReason'] as String?,
    );
  }
}

@immutable
class DeployBackupView {
  const DeployBackupView({
    required this.id,
    required this.connectionId,
    required this.databaseName,
    required this.status,
    required this.startedAt,
  });

  final int id;
  final int connectionId;
  final String databaseName;
  final String status;
  final String startedAt;

  factory DeployBackupView.fromJson(Map<String, dynamic> json) {
    return DeployBackupView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      connectionId: (json['connectionId'] as num?)?.toInt() ?? 0,
      databaseName: (json['databaseName'] as String?) ?? '',
      status: (json['status'] as String?) ?? '',
      startedAt: (json['startedAt'] as String?) ?? '',
    );
  }
}

@immutable
class DeployWorkflowView {
  const DeployWorkflowView({
    required this.path,
    required this.yaml,
    required this.urlVariable,
    required this.hookPath,
    required this.audience,
  });

  final String path;
  final String yaml;
  final String urlVariable;
  final String hookPath;
  final String audience;

  factory DeployWorkflowView.fromJson(Map<String, dynamic> json) {
    return DeployWorkflowView(
      path: (json['path'] as String?) ?? '.github/workflows/deploy.yml',
      yaml: (json['yaml'] as String?) ?? '',
      urlVariable: (json['urlVariable'] as String?) ?? 'NETMONITOR_URL',
      hookPath: (json['hookPath'] as String?) ?? '',
      audience: (json['audience'] as String?) ?? '',
    );
  }
}

@immutable
class DeployGithubManifestView {
  const DeployGithubManifestView({
    required this.actionUrl,
    required this.manifest,
  });

  final String actionUrl;
  final String manifest;

  factory DeployGithubManifestView.fromJson(Map<String, dynamic> json) {
    return DeployGithubManifestView(
      actionUrl: (json['actionUrl'] as String?) ?? '',
      manifest: (json['manifest'] as String?) ?? '',
    );
  }
}

@immutable
class DeployRepositoryView {
  const DeployRepositoryView({
    required this.fullName,
    required this.isPrivate,
    required this.defaultBranch,
    this.description,
    this.pushedAt,
  });

  final String fullName;
  final bool isPrivate;
  final String defaultBranch;
  final String? description;
  final String? pushedAt;

  factory DeployRepositoryView.fromJson(Map<String, dynamic> json) {
    return DeployRepositoryView(
      fullName: (json['fullName'] as String?) ?? '',
      isPrivate: json['private'] == true,
      defaultBranch: (json['defaultBranch'] as String?) ?? 'main',
      description: json['description'] as String?,
      pushedAt: json['pushedAt'] as String?,
    );
  }
}

@immutable
class DeployReferencesView {
  const DeployReferencesView({
    required this.defaultBranch,
    required this.branches,
    required this.tags,
  });

  final String defaultBranch;
  final List<String> branches;
  final List<String> tags;

  factory DeployReferencesView.fromJson(Map<String, dynamic> json) {
    List<String> list(Object? v) => v is List
        ? List<String>.unmodifiable(v.whereType<String>())
        : const <String>[];
    return DeployReferencesView(
      defaultBranch: (json['defaultBranch'] as String?) ?? 'main',
      branches: list(json['branches']),
      tags: list(json['tags']),
    );
  }
}

@immutable
class DeployComposeDiscovery {
  const DeployComposeDiscovery({
    required this.files,
    required this.suggestedFiles,
    required this.services,
    required this.images,
    required this.imageTagVariable,
    required this.notes,
  });

  final List<String> files;
  final List<String> suggestedFiles;
  final List<String> services;
  final List<DeployBuildImage> images;
  final String imageTagVariable;
  final List<String> notes;

  factory DeployComposeDiscovery.fromJson(Map<String, dynamic> json) {
    List<String> list(Object? v) => v is List
        ? List<String>.unmodifiable(v.whereType<String>())
        : const <String>[];
    final rawImages = json['images'];
    return DeployComposeDiscovery(
      files: list(json['files']),
      suggestedFiles: list(json['suggestedFiles']),
      services: list(json['services']),
      images: rawImages is List
          ? List<DeployBuildImage>.unmodifiable(
              rawImages.whereType<Map>().map(
                (e) => DeployBuildImage.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DeployBuildImage>[],
      imageTagVariable: (json['imageTagVariable'] as String?) ?? 'IMAGE_TAG',
      notes: list(json['notes']),
    );
  }
}

@immutable
class DeployGithubStatusView {
  const DeployGithubStatusView({
    required this.workflow,
    this.workflowUrl,
    this.pullRequestNumber,
    this.pullRequestUrl,
    this.canWrite = false,
    this.unavailableReason,
  });

  final String workflow; // 'missing' | 'current' | 'different' | 'unknown'
  final String? workflowUrl;
  final int? pullRequestNumber;
  final String? pullRequestUrl;
  final bool canWrite;
  final String? unavailableReason;

  factory DeployGithubStatusView.fromJson(Map<String, dynamic> json) {
    final pr = json['pullRequest'];
    return DeployGithubStatusView(
      workflow: (json['workflow'] as String?) ?? 'unknown',
      workflowUrl: json['workflowUrl'] as String?,
      pullRequestNumber: pr is Map ? (pr['number'] as num?)?.toInt() : null,
      pullRequestUrl: pr is Map ? pr['url'] as String? : null,
      canWrite: json['canWrite'] == true,
      unavailableReason: json['unavailableReason'] as String?,
    );
  }
}

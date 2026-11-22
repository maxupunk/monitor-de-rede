import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

export 'deploy_build_and_github_models.dart';

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
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      connected: parseBool(json['connected']),
      allowsDeploy: parseBool(json['allowsDeploy']),
      allowsBuild: parseBool(json['allowsBuild']),
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
    return DeployEnvVersionView(
      id: parseIntOrNull(json['id']) ?? 0,
      targetId: parseIntOrNull(json['targetId']) ?? 0,
      keys: List<String>.unmodifiable(asStringList(json['keys'])),
      createdBy: json['createdBy'] as String?,
      note: json['note'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      current: parseBool(json['current']),
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
    if (parseIntOrNull(json['id']) == null ||
        parseIntOrNull(json['targetId']) == null ||
        json['status'] is! String ||
        json['commitSha'] is! String) {
      return null;
    }
    return DeployRunView.fromJson(json);
  }

  factory DeployRunView.fromJson(Map<String, dynamic> json) {
    final sha = (json['commitSha'] as String?) ?? '';
    return DeployRunView(
      id: parseIntOrNull(json['id']) ?? 0,
      applicationId: parseIntOrNull(json['applicationId']) ?? 0,
      applicationName: (json['applicationName'] as String?) ?? '',
      repository: (json['repository'] as String?) ?? '',
      targetId: parseIntOrNull(json['targetId']) ?? 0,
      targetName: (json['targetName'] as String?) ?? '',
      trigger: (json['trigger'] as String?) ?? 'manual',
      triggeredBy: json['triggeredBy'] as String?,
      reference: json['reference'] as String?,
      commitSha: sha,
      shortSha:
          (json['shortSha'] as String?) ?? shortGitSha(sha, fallback: sha),
      commitMessage: json['commitMessage'] as String?,
      imageTag: (json['imageTag'] as String?) ?? '',
      envVersionId: parseIntOrNull(json['envVersionId']),
      status: (json['status'] as String?) ?? 'queued',
      stage: json['stage'] as String?,
      rollbackOfRunId: parseIntOrNull(json['rollbackOfRunId']),
      afterRunId: parseIntOrNull(json['afterRunId']),
      backupId: parseIntOrNull(json['backupId']),
      approvedBy: json['approvedBy'] as String?,
      approvedAt: json['approvedAt'] as String?,
      scheduledFor: json['scheduledFor'] as String?,
      error: json['error'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      startedAt: json['startedAt'] as String?,
      finishedAt: json['finishedAt'] as String?,
      durationMs: parseIntOrNull(json['durationMs']),
      buildId: parseIntOrNull(json['buildId']),
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
      id: parseIntOrNull(json['id']) ?? 0,
      applicationId: parseIntOrNull(json['applicationId']) ?? 0,
      name: (json['name'] as String?) ?? '',
      agent: json['agent'] is Map
          ? DeployAgentView.fromJson(asJsonMap(json['agent']))
          : null,
      project: (json['project'] as String?) ?? '',
      reference: (json['reference'] as String?) ?? 'main',
      requiresApproval: parseBool(json['requiresApproval']),
      windowStart: json['windowStart'] as String?,
      windowEnd: json['windowEnd'] as String?,
      backupConnectionId: parseIntOrNull(json['backupConnectionId']),
      backupConnectionName: json['backupConnectionName'] as String?,
      waitTimeoutSecs: parseIntOrNull(json['waitTimeoutSecs']) ?? 180,
      autoRollback: parseBool(json['autoRollback'], fallback: true),
      keepImages: parseIntOrNull(json['keepImages']) ?? 3,
      enabled: parseBool(json['enabled'], fallback: true),
      position: parseIntOrNull(json['position']) ?? 0,
      vulnerabilityPolicy:
          (json['vulnerabilityPolicy'] as String?) ?? 'block_critical',
      requireProvenance: parseBool(json['requireProvenance']),
      live: json['live'] is Map
          ? DeployRunView.fromJson(asJsonMap(json['live']))
          : null,
      pending: json['pending'] is Map
          ? DeployRunView.fromJson(asJsonMap(json['pending']))
          : null,
      env: json['env'] is Map
          ? DeployEnvVersionView.fromJson(asJsonMap(json['env']))
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
    final files = asStringList(json['composeFiles']);
    return DeployApplicationView(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      slug: (json['slug'] as String?) ?? '',
      repository: (json['repository'] as String?) ?? '',
      repositoryUrl: (json['repositoryUrl'] as String?) ?? '',
      credentialId: parseIntOrNull(json['credentialId']),
      credentialName: json['credentialName'] as String?,
      githubConnectionId: parseIntOrNull(json['githubConnectionId']),
      githubAccount: json['githubAccount'] as String?,
      composeFiles: files.isNotEmpty
          ? List<String>.unmodifiable(files)
          : const <String>['docker-compose.yml'],
      imageTagVariable: (json['imageTagVariable'] as String?) ?? 'IMAGE_TAG',
      images: List<DeployBuildImage>.unmodifiable(
        asJsonList(json['images'], DeployBuildImage.fromJson),
      ),
      rollout: (json['rollout'] as String?) ?? 'sequential',
      builder: (json['builder'] as String?) ?? 'netmonitor',
      buildProbeId: parseIntOrNull(json['buildProbeId']),
      testTarget: json['testTarget'] as String?,
      buildArgs: List<DeployBuildArg>.unmodifiable(
        asJsonList(json['buildArgs'], DeployBuildArg.fromJson),
      ),
      targets: List<DeployTargetView>.unmodifiable(
        asJsonList(json['targets'], DeployTargetView.fromJson),
      ),
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
      applications: parseIntOrNull(json['applications']) ?? 0,
      targets: parseIntOrNull(json['targets']) ?? 0,
      running: parseIntOrNull(json['running']) ?? 0,
      awaitingApproval: parseIntOrNull(json['awaitingApproval']) ?? 0,
      succeeded24h: parseIntOrNull(json['succeeded24h']) ?? 0,
      failed24h: parseIntOrNull(json['failed24h']) ?? 0,
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
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      registry: json['registry'] as String?,
      username: (json['username'] as String?) ?? '',
      hasSecret: parseBool(json['hasSecret']),
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
      id: parseIntOrNull(json['id']) ?? 0,
      account: (json['account'] as String?) ?? '',
      organization: json['organization'] as String?,
      appSlug: (json['appSlug'] as String?) ?? '',
      installed: parseBool(json['installed']),
      canWrite: parseBool(json['canWrite']),
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
      applicationId: parseIntOrNull(json['applicationId']),
      targetId: parseIntOrNull(json['targetId']),
      runId: parseIntOrNull(json['runId']),
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
          ? DeployGuideAction.fromJson(asJsonMap(json['action']))
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
    return DeployGuideView(
      complete: parseBool(json['complete']),
      steps: List<DeployGuideStep>.unmodifiable(
        asJsonList(json['steps'], DeployGuideStep.fromJson),
      ),
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
    return DeployOverview(
      applications: List<DeployApplicationView>.unmodifiable(
        asJsonList(json['applications'], DeployApplicationView.fromJson),
      ),
      recentRuns: List<DeployRunView>.unmodifiable(
        asJsonList(json['recentRuns'], DeployRunView.fromJson),
      ),
      counts: json['counts'] is Map
          ? DeployCounts.fromJson(asJsonMap(json['counts']))
          : const DeployCounts(),
      credentials: List<DeployCredentialView>.unmodifiable(
        asJsonList(json['credentials'], DeployCredentialView.fromJson),
      ),
      githubConnections: List<DeployGithubConnectionView>.unmodifiable(
        asJsonList(
          json['githubConnections'],
          DeployGithubConnectionView.fromJson,
        ),
      ),
      hookPath: (json['hookPath'] as String?) ?? '/api/deploy-hooks/github',
      oidcAudience: (json['oidcAudience'] as String?) ?? 'netmonitor',
      guide: json['guide'] is Map
          ? DeployGuideView.fromJson(asJsonMap(json['guide']))
          : const DeployGuideView(complete: false, steps: <DeployGuideStep>[]),
    );
  }
}

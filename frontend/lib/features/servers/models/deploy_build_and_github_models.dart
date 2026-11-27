import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';

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
      bytes: parseIntOrNull(json['bytes']) ?? 0,
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
    if (parseIntOrNull(json['id']) == null ||
        parseIntOrNull(json['applicationId']) == null ||
        json['status'] is! String ||
        json['commitSha'] is! String) {
      return null;
    }
    return DeployServerBuildView.fromJson(json);
  }

  factory DeployServerBuildView.fromJson(Map<String, dynamic> json) {
    final sha = (json['commitSha'] as String?) ?? '';
    return DeployServerBuildView(
      id: parseIntOrNull(json['id']) ?? 0,
      applicationId: parseIntOrNull(json['applicationId']) ?? 0,
      application: (json['application'] as String?) ?? '',
      reference: (json['reference'] as String?) ?? '',
      commitSha: sha,
      shortSha:
          (json['shortSha'] as String?) ?? shortGitSha(sha, fallback: sha),
      commitMessage: json['commitMessage'] as String?,
      trigger: (json['trigger'] as String?) ?? 'manual',
      triggeredBy: json['triggeredBy'] as String?,
      agent: json['agent'] as String?,
      status: (json['status'] as String?) ?? 'queued',
      stage: json['stage'] as String?,
      currentImage: json['currentImage'] as String?,
      noCache: parseBool(json['noCache']),
      images: List<DeployStoredImage>.unmodifiable(
        asJsonList(json['images'], DeployStoredImage.fromJson),
      ),
      error: json['error'] as String?,
      startedAt: json['startedAt'] as String?,
      finishedAt: json['finishedAt'] as String?,
      durationMs: parseIntOrNull(json['durationMs']),
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
      builderReady: parseBool(json['builderReady']),
      gitVersion: json['gitVersion'] as String?,
      freeBytes: parseIntOrNull(json['freeBytes']) ?? 0,
      cpus: parseIntOrNull(json['cpus']) ?? 0,
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
      title:
          (json['label'] as String?) ??
          (json['title'] as String?) ??
          (json['message'] as String?) ??
          '',
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
    return DeployReadinessView(
      ready: parseBool(json['ready']),
      checks: List<DeployReadinessCheck>.unmodifiable(
        asJsonList(json['checks'], DeployReadinessCheck.fromJson),
      ),
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
      id: parseIntOrNull(json['id']),
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
    return DeployEnvDiff(
      added: List<String>.unmodifiable(asStringList(json['added'])),
      changed: List<String>.unmodifiable(asStringList(json['changed'])),
      removed: List<String>.unmodifiable(asStringList(json['removed'])),
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
      version: DeployEnvVersionView.fromJson(asJsonMap(json['version'])),
      diff: DeployEnvDiff.fromJson(asJsonMap(json['diff'])),
      run: json['run'] is Map
          ? DeployRunView.fromJson(asJsonMap(json['run']))
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
          (json['shortSha'] as String?) ?? shortGitSha(sha, fallback: sha),
      message: (json['message'] as String?) ?? '',
      author: json['author'] as String?,
      date: json['date'] as String?,
      url: json['url'] as String?,
    );
  }
}

/// Os commits recentes da branch de um alvo (`GET /deploys/targets/{id}/github/commits`).
@immutable
class DeployBranchCommitsView {
  const DeployBranchCommitsView({
    this.branch,
    required this.commits,
    this.unavailableReason,
  });

  /// A branch do alvo; `null` quando ele acompanha tags (`v*`).
  final String? branch;

  /// O mais novo primeiro.
  final List<DeployCommitView> commits;

  /// Por que a lista não veio — o mesmo texto que o "Publicar" daria.
  final String? unavailableReason;

  DeployCommitView? get latest => commits.isEmpty ? null : commits.first;

  factory DeployBranchCommitsView.fromJson(Map<String, dynamic> json) {
    return DeployBranchCommitsView(
      branch: json['branch'] as String?,
      commits: List<DeployCommitView>.unmodifiable(
        asJsonList(json['commits'], DeployCommitView.fromJson),
      ),
      unavailableReason: json['unavailableReason'] as String?,
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
    return DeployChangesView(
      baseSha: json['baseSha'] as String?,
      headSha: json['headSha'] as String?,
      compareUrl: json['compareUrl'] as String?,
      commits: List<DeployCommitView>.unmodifiable(
        asJsonList(json['commits'], DeployCommitView.fromJson),
      ),
      total: parseIntOrNull(json['total']) ?? 0,
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
      id: parseIntOrNull(json['id']) ?? 0,
      connectionId: parseIntOrNull(json['connectionId']) ?? 0,
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
      isPrivate: parseBool(json['private']),
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
    return DeployReferencesView(
      defaultBranch: (json['defaultBranch'] as String?) ?? 'main',
      branches: List<String>.unmodifiable(asStringList(json['branches'])),
      tags: List<String>.unmodifiable(asStringList(json['tags'])),
    );
  }
}

@immutable
class DeployEnvTemplateView {
  const DeployEnvTemplateView({required this.name, required this.content});

  final String name;
  final String content;

  factory DeployEnvTemplateView.fromJson(Map<String, dynamic> json) {
    return DeployEnvTemplateView(
      name: (json['name'] as String?) ?? '',
      content: (json['content'] as String?) ?? '',
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
    this.envTemplates = const <DeployEnvTemplateView>[],
  });

  final List<String> files;
  final List<String> suggestedFiles;
  final List<String> services;
  final List<DeployBuildImage> images;
  final String imageTagVariable;
  final List<String> notes;
  final List<DeployEnvTemplateView> envTemplates;

  factory DeployComposeDiscovery.fromJson(Map<String, dynamic> json) {
    return DeployComposeDiscovery(
      files: List<String>.unmodifiable(asStringList(json['files'])),
      suggestedFiles: List<String>.unmodifiable(
        asStringList(json['suggestedFiles']),
      ),
      services: List<String>.unmodifiable(asStringList(json['services'])),
      images: List<DeployBuildImage>.unmodifiable(
        asJsonList(json['images'], DeployBuildImage.fromJson),
      ),
      imageTagVariable: (json['imageTagVariable'] as String?) ?? 'IMAGE_TAG',
      notes: List<String>.unmodifiable(asStringList(json['notes'])),
      envTemplates: List<DeployEnvTemplateView>.unmodifiable(
        asJsonList(json['envTemplates'], DeployEnvTemplateView.fromJson),
      ),
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
    final pr = asJsonMap(json['pullRequest']);
    return DeployGithubStatusView(
      workflow: (json['workflow'] as String?) ?? 'unknown',
      workflowUrl: json['workflowUrl'] as String?,
      pullRequestNumber: parseIntOrNull(pr['number']),
      pullRequestUrl: pr['url'] as String?,
      canWrite: parseBool(json['canWrite']),
      unavailableReason: json['unavailableReason'] as String?,
    );
  }
}

// ---------------------------------------------------------------------------
// DTOs fortemente tipados de entrada para mutações de Deploy (ISP — §10)
// ---------------------------------------------------------------------------

@immutable
class DeployCredentialInput {
  const DeployCredentialInput({
    required this.name,
    this.registry,
    required this.username,
    required this.secret,
  });

  final String name;
  final String? registry;
  final String username;
  final String secret;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    if (registry != null && registry!.isNotEmpty) 'registry': registry,
    'username': username,
    'secret': secret,
  };
}

@immutable
class DeployApplicationInput {
  const DeployApplicationInput({
    required this.name,
    required this.slug,
    required this.repository,
    this.credentialId,
    this.githubConnectionId,
    required this.composeFiles,
    required this.imageTagVariable,
    this.images = const <DeployBuildImage>[],
    this.rollout = 'sequential',
    this.builder = 'netmonitor',
    this.buildProbeId,
    this.testTarget,
    this.buildArgs = const <DeployBuildArg>[],
  });

  final String name;
  final String slug;
  final String repository;
  final int? credentialId;
  final int? githubConnectionId;
  final List<String> composeFiles;
  final String imageTagVariable;
  final List<DeployBuildImage> images;
  final String rollout;
  final String builder;
  final int? buildProbeId;
  final String? testTarget;
  final List<DeployBuildArg> buildArgs;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'slug': slug,
    'repository': repository,
    'credentialId': credentialId,
    'githubConnectionId': githubConnectionId,
    'composeFiles': composeFiles,
    'imageTagVariable': imageTagVariable,
    'images': images.map((e) => e.toJson()).toList(growable: false),
    'rollout': rollout,
    'builder': builder,
    'buildProbeId': buildProbeId,
    if (testTarget != null && testTarget!.isNotEmpty) 'testTarget': testTarget,
    'buildArgs': buildArgs.map((e) => e.toJson()).toList(growable: false),
  };
}

@immutable
class DeployTargetInput {
  const DeployTargetInput({
    required this.name,
    required this.probeId,
    required this.project,
    required this.reference,
    this.requiresApproval = false,
    this.windowStart,
    this.windowEnd,
    this.backupConnectionId,
    this.waitTimeoutSecs = 180,
    this.autoRollback = true,
    this.keepImages = 3,
    this.enabled = true,
    this.position = 0,
    this.vulnerabilityPolicy = 'block_critical',
    this.requireProvenance = false,
  });

  final String name;
  final int probeId;
  final String project;
  final String reference;
  final bool requiresApproval;
  final String? windowStart;
  final String? windowEnd;
  final int? backupConnectionId;
  final int waitTimeoutSecs;
  final bool autoRollback;
  final int keepImages;
  final bool enabled;
  final int position;
  final String vulnerabilityPolicy;
  final bool requireProvenance;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'probeId': probeId,
    'project': project,
    'reference': reference,
    'requiresApproval': requiresApproval,
    'windowStart': (windowStart != null && windowStart!.isNotEmpty)
        ? windowStart
        : null,
    'windowEnd': (windowEnd != null && windowEnd!.isNotEmpty)
        ? windowEnd
        : null,
    'backupConnectionId': backupConnectionId,
    'waitTimeoutSecs': waitTimeoutSecs,
    'autoRollback': autoRollback,
    'keepImages': keepImages,
    'enabled': enabled,
    'position': position,
    'vulnerabilityPolicy': vulnerabilityPolicy,
    'requireProvenance': requireProvenance,
  };
}

@immutable
class DeploySetupInput {
  const DeploySetupInput({
    this.applicationId,
    this.application,
    required this.target,
    this.env,
  });

  final int? applicationId;
  final DeployApplicationInput? application;
  final DeployTargetInput target;
  final String? env;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'applicationId': applicationId,
    if (applicationId == null && application != null)
      'application': application!.toJson(),
    'target': target.toJson(),
    if (env != null && env!.trim().isNotEmpty) 'env': env,
  };
}

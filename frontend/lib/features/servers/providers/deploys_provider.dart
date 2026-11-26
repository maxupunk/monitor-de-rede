import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/utils/collection_helpers.dart';
import 'package:netmonitor_app/features/servers/utils/deploy_presentation.dart';

const int _maxConsoleLines = 4000;

@immutable
class DeploysState {
  const DeploysState({
    this.overview,
    this.loading = false,
    this.error,
    this.runs = const <int, DeployRunView>{},
    this.runConsoles = const <int, List<String>>{},
    this.agents = const <DeployAgentView>[],
    this.builds = const <int, DeployServerBuildView>{},
    this.buildConsoles = const <int, List<String>>{},
    this.buildsLoaded = false,
  });

  final DeployOverview? overview;
  final bool loading;
  final String? error;
  final Map<int, DeployRunView> runs;
  final Map<int, List<String>> runConsoles;
  final List<DeployAgentView> agents;
  final Map<int, DeployServerBuildView> builds;
  final Map<int, List<String>> buildConsoles;
  final bool buildsLoaded;

  List<DeployApplicationView> get applications =>
      overview?.applications ?? const <DeployApplicationView>[];
  List<DeployCredentialView> get credentials =>
      overview?.credentials ?? const <DeployCredentialView>[];
  List<DeployGithubConnectionView> get githubConnections =>
      overview?.githubConnections ?? const <DeployGithubConnectionView>[];

  List<DeployRunView> get recentRuns {
    final merged = <int, DeployRunView>{};
    for (final run in overview?.recentRuns ?? const <DeployRunView>[]) {
      merged[run.id] = run;
    }
    merged.addAll(runs);
    final list = merged.values.toList()..sort((a, b) => b.id.compareTo(a.id));
    return list.length > 30 ? list.sublist(0, 30) : list;
  }

  List<DeployRunView> get awaitingApproval => recentRuns
      .where((r) => r.status == 'awaiting_approval')
      .toList(growable: false);

  List<DeployServerBuildView> get recentBuilds {
    final list = builds.values.toList()..sort((a, b) => b.id.compareTo(a.id));
    return list.length > 30 ? list.sublist(0, 30) : list;
  }

  DeployServerBuildView? latestBuildOf(int applicationId) {
    for (final b in recentBuilds) {
      if (b.applicationId == applicationId) return b;
    }
    return null;
  }

  DeployRunView? runById(int id) {
    if (runs.containsKey(id)) return runs[id];
    for (final r in recentRuns) {
      if (r.id == id) return r;
    }
    return null;
  }

  DeploysState copyWith({
    DeployOverview? overview,
    bool? loading,
    String? error,
    bool clearError = false,
    Map<int, DeployRunView>? runs,
    Map<int, List<String>>? runConsoles,
    List<DeployAgentView>? agents,
    Map<int, DeployServerBuildView>? builds,
    Map<int, List<String>>? buildConsoles,
    bool? buildsLoaded,
  }) {
    return DeploysState(
      overview: overview ?? this.overview,
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
      runs: runs ?? this.runs,
      runConsoles: runConsoles ?? this.runConsoles,
      agents: agents ?? this.agents,
      builds: builds ?? this.builds,
      buildConsoles: buildConsoles ?? this.buildConsoles,
      buildsLoaded: buildsLoaded ?? this.buildsLoaded,
    );
  }
}

/// Estado de Deploy Contínuo & Compilações no Servidor (ADR 021 / ADR 023).
/// 40 endpoints de deploy + 9 endpoints de build + 5 eventos SSE. ZERO polling!
class DeploysNotifier extends Notifier<DeploysState> {
  @override
  DeploysState build() => const DeploysState();

  ApiClient get _api => ref.read(apiClientProvider);

  Future<bool> fetchOverview() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await _api.get<Map<String, dynamic>>('/deploys');
      state = state.copyWith(
        overview: DeployOverview.fromJson(raw),
        loading: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(err, fallback: 'Erro ao carregar os deploys'),
      );
      return false;
    }
  }

  void refreshIfLoaded() {
    if (state.overview != null) {
      fetchOverview();
    }
  }

  Future<void> fetchAgents() async {
    try {
      final raw = await _api.get<List<dynamic>>('/deploys/agents');
      final parsed = asJsonList(raw, DeployAgentView.fromJson);
      state = state.copyWith(
        agents: List<DeployAgentView>.unmodifiable(parsed),
      );
    } catch (_) {
      state = state.copyWith(agents: const <DeployAgentView>[]);
    }
  }

  // ── SSE Handlers ────────────────────────────────────────────────────────

  void applyRunPayload(Map<String, dynamic> data) {
    final parsed = DeployRunView.tryFromJson(data);
    if (parsed != null) applyRun(parsed);
  }

  void applyRun(DeployRunView run) {
    final current = state.runs[run.id];
    if (current != null &&
        isDeployRunFinished(current.status) &&
        !isDeployRunFinished(run.status)) {
      return;
    }
    final nextRuns = Map<int, DeployRunView>.from(state.runs)..[run.id] = run;
    DeployOverview? nextOverview = state.overview;
    if (nextOverview != null) {
      final patchedApps = nextOverview.applications
          .map((app) {
            final hasTarget = app.targets.any((t) => t.id == run.targetId);
            if (!hasTarget) return app;
            final patchedTargets = app.targets
                .map((target) {
                  if (target.id != run.targetId) return target;
                  var updated = target;
                  if (isDeployRunPending(run.status) ||
                      run.status == 'running') {
                    if (target.pending == null ||
                        target.pending!.id <= run.id) {
                      updated = updated.copyWith(pending: run);
                    }
                  } else if (target.pending?.id == run.id) {
                    updated = updated.copyWith(clearPending: true);
                  }
                  if (run.status == 'succeeded' &&
                      (target.live == null || target.live!.id <= run.id)) {
                    updated = updated.copyWith(live: run);
                  }
                  return updated;
                })
                .toList(growable: false);
            return app.copyWith(
              targets: List<DeployTargetView>.unmodifiable(patchedTargets),
            );
          })
          .toList(growable: false);
      nextOverview = nextOverview.copyWith(
        applications: List<DeployApplicationView>.unmodifiable(patchedApps),
      );
    }
    state = state.copyWith(
      runs: Map<int, DeployRunView>.unmodifiable(nextRuns),
      overview: nextOverview,
    );
  }

  void applyRunLinesPayload(Map<String, dynamic> data) {
    final runId = parseIntOrNull(data['runId']);
    if (runId == null || data['lines'] is! List) return;
    final current = state.runConsoles[runId];
    if (current == null) return;
    final nextLines = appendBounded<String>(
      current,
      asStringList(data['lines']),
      _maxConsoleLines,
    );
    state = state.copyWith(
      runConsoles: Map<int, List<String>>.unmodifiable(<int, List<String>>{
        ...state.runConsoles,
        runId: nextLines,
      }),
    );
  }

  void applyBuildPayload(Map<String, dynamic> data) {
    final parsed = DeployServerBuildView.tryFromJson(data);
    if (parsed != null) applyBuild(parsed);
  }

  void applyBuild(DeployServerBuildView build) {
    final known = state.builds[build.id];
    if (known != null &&
        !deployBuildStatusMeta(known.status).live &&
        deployBuildStatusMeta(build.status).live) {
      return;
    }
    state = state.copyWith(
      builds: Map<int, DeployServerBuildView>.unmodifiable(
        <int, DeployServerBuildView>{...state.builds, build.id: build},
      ),
    );
  }

  void applyBuildLinesPayload(Map<String, dynamic> data) {
    final buildId = parseIntOrNull(data['buildId']);
    if (buildId == null || data['lines'] is! List) return;
    final current = state.buildConsoles[buildId];
    if (current == null) return;
    final nextLines = appendBounded<String>(
      current,
      asStringList(data['lines']),
      _maxConsoleLines,
    );
    state = state.copyWith(
      buildConsoles: Map<int, List<String>>.unmodifiable(<int, List<String>>{
        ...state.buildConsoles,
        buildId: nextLines,
      }),
    );
  }

  // ── Consoles ────────────────────────────────────────────────────────────

  Future<void> openRunConsole(int runId) async {
    if (!state.runConsoles.containsKey(runId)) {
      state = state.copyWith(
        runConsoles: Map<int, List<String>>.unmodifiable(<int, List<String>>{
          ...state.runConsoles,
          runId: const <String>[],
        }),
      );
    }
    final results =
        await Future.wait<Map<String, dynamic>>(<Future<Map<String, dynamic>>>[
          _api.get<Map<String, dynamic>>('/deploys/runs/$runId/log'),
          _api.get<Map<String, dynamic>>('/deploys/runs/$runId'),
        ]);
    final savedLog = (results[0]['log'] as String?) ?? '';
    final savedLines = savedLog.isEmpty
        ? <String>[]
        : savedLog.replaceFirst(RegExp(r'\n$'), '').split('\n');
    final live = state.runConsoles[runId] ?? const <String>[];
    final extra = live.where((l) => !savedLines.contains(l));
    state = state.copyWith(
      runConsoles: Map<int, List<String>>.unmodifiable(<int, List<String>>{
        ...state.runConsoles,
        runId: List<String>.unmodifiable(<String>[...savedLines, ...extra]),
      }),
    );
    applyRun(DeployRunView.fromJson(results[1]));
  }

  void closeRunConsole(int runId) {
    final next = Map<int, List<String>>.from(state.runConsoles)..remove(runId);
    state = state.copyWith(
      runConsoles: Map<int, List<String>>.unmodifiable(next),
    );
  }

  // ── CRUD & Setup (DTOs tipados — ISP) ───────────────────────────────────

  Future<Map<String, dynamic>> setup(DeploySetupInput input) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/setup',
      data: input.toJson(),
    );
    await fetchOverview();
    return raw;
  }

  Future<Map<String, dynamic>> saveApplication(
    int? id,
    DeployApplicationInput input,
  ) async {
    final raw = id == null
        ? await _api.post<Map<String, dynamic>>(
            '/deploys/applications',
            data: input.toJson(),
          )
        : await _api.put<Map<String, dynamic>>(
            '/deploys/applications/$id',
            data: input.toJson(),
          );
    await fetchOverview();
    return raw;
  }

  Future<void> removeApplication(int id) async {
    await _api.delete<dynamic>('/deploys/applications/$id');
    await fetchOverview();
  }

  Future<Map<String, dynamic>> saveTarget(
    int applicationId,
    int? targetId,
    DeployTargetInput input,
  ) async {
    final raw = targetId == null
        ? await _api.post<Map<String, dynamic>>(
            '/deploys/applications/$applicationId/targets',
            data: input.toJson(),
          )
        : await _api.put<Map<String, dynamic>>(
            '/deploys/targets/$targetId',
            data: input.toJson(),
          );
    await fetchOverview();
    return raw;
  }

  Future<void> removeTarget(int targetId) async {
    await _api.delete<dynamic>('/deploys/targets/$targetId');
    await fetchOverview();
  }

  Future<DeployCredentialView> saveCredential(
    int? id,
    DeployCredentialInput input,
  ) async {
    final raw = id == null
        ? await _api.post<Map<String, dynamic>>(
            '/deploys/credentials',
            data: input.toJson(),
          )
        : await _api.put<Map<String, dynamic>>(
            '/deploys/credentials/$id',
            data: input.toJson(),
          );
    await fetchOverview();
    return DeployCredentialView.fromJson(raw);
  }

  Future<void> removeCredential(int id) async {
    await _api.delete<dynamic>('/deploys/credentials/$id');
    await fetchOverview();
  }

  // ── GitHub App & Discovery ──────────────────────────────────────────────

  Future<DeployGithubManifestView> githubManifest({
    required String origin,
    String? organization,
    required bool allowWrite,
  }) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/github/manifest',
      data: <String, dynamic>{
        'origin': origin,
        if (organization != null && organization.isNotEmpty)
          'organization': organization,
        'allowWrite': allowWrite,
      },
    );
    return DeployGithubManifestView.fromJson(raw);
  }

  Future<DeployGithubConnectionView> connectGithub(
    String code,
    String callbackState,
  ) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/github/connections',
      data: <String, dynamic>{'code': code, 'state': callbackState},
    );
    await fetchOverview();
    return DeployGithubConnectionView.fromJson(raw);
  }

  Future<DeployGithubConnectionView> claimGithubInstallation(
    int installationId,
  ) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/github/installations',
      data: <String, dynamic>{'installationId': installationId},
    );
    await fetchOverview();
    return DeployGithubConnectionView.fromJson(raw);
  }

  Future<DeployGithubConnectionView> refreshGithub(int id) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/github/connections/$id/refresh',
      data: const <String, dynamic>{},
    );
    await fetchOverview();
    return DeployGithubConnectionView.fromJson(raw);
  }

  Future<void> removeGithub(int id) async {
    await _api.delete<dynamic>('/deploys/github/connections/$id');
    await fetchOverview();
  }

  Future<List<DeployRepositoryView>> githubRepositories({
    int? connectionId,
    int? credentialId,
  }) async {
    final raw = await _api.get<List<dynamic>>(
      '/deploys/github/repositories',
      queryParameters: <String, dynamic>{
        'connectionId': ?connectionId,
        'credentialId': ?credentialId,
      },
    );
    return List<DeployRepositoryView>.unmodifiable(
      asJsonList(raw, DeployRepositoryView.fromJson),
    );
  }

  Future<DeployReferencesView> githubReferences({
    required String repository,
    int? connectionId,
    int? credentialId,
  }) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/github/references',
      queryParameters: <String, dynamic>{
        'repository': repository,
        'connectionId': ?connectionId,
        'credentialId': ?credentialId,
      },
    );
    return DeployReferencesView.fromJson(raw);
  }

  Future<DeployComposeDiscovery> githubCompose({
    required String repository,
    String? reference,
    int? connectionId,
    int? credentialId,
  }) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/github/compose',
      queryParameters: <String, dynamic>{
        'repository': repository,
        if (reference != null && reference.trim().isNotEmpty)
          'reference': reference.trim(),
        'connectionId': ?connectionId,
        'credentialId': ?credentialId,
      },
    );
    return DeployComposeDiscovery.fromJson(raw);
  }

  Future<DeployWorkflowView> workflow(
    int applicationId,
    String centralUrl,
  ) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/applications/$applicationId/workflow',
      queryParameters: <String, dynamic>{'centralUrl': centralUrl},
    );
    return DeployWorkflowView.fromJson(raw);
  }

  Future<DeployGithubStatusView> githubStatus(
    int applicationId,
    String centralUrl,
  ) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/applications/$applicationId/github',
      queryParameters: <String, dynamic>{'centralUrl': centralUrl},
    );
    return DeployGithubStatusView.fromJson(raw);
  }

  Future<Map<String, dynamic>> openWorkflowPullRequest(
    int applicationId,
    String centralUrl,
  ) {
    return _api.post<Map<String, dynamic>>(
      '/deploys/applications/$applicationId/github/pull-request',
      data: <String, dynamic>{'centralUrl': centralUrl},
    );
  }

  // ── Readiness, Runs & Actions ───────────────────────────────────────────

  Future<DeployReadinessView> readinessFor(int probeId, String project) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/readiness',
      queryParameters: <String, dynamic>{
        'probeId': probeId,
        'project': project,
      },
    );
    return DeployReadinessView.fromJson(raw);
  }

  Future<DeployReadinessView> targetReadiness(int targetId) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/targets/$targetId/readiness',
    );
    return DeployReadinessView.fromJson(raw);
  }

  Future<List<DeployRunView>> targetHistory(int targetId) async {
    final raw = await _api.get<List<dynamic>>(
      '/deploys/targets/$targetId/runs',
    );
    return List<DeployRunView>.unmodifiable(
      asJsonList(raw, DeployRunView.fromJson),
    );
  }

  Future<DeployChangesView> runChanges(int runId) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/runs/$runId/changes',
    );
    return DeployChangesView.fromJson(raw);
  }

  Future<List<DeployBackupView>> runBackups(int runId) async {
    final raw = await _api.get<List<dynamic>>('/deploys/runs/$runId/backups');
    return List<DeployBackupView>.unmodifiable(
      asJsonList(raw, DeployBackupView.fromJson),
    );
  }

  Future<DeployRunView> _runAction(
    String path,
    Map<String, dynamic> body,
  ) async {
    final raw = await _api.post<Map<String, dynamic>>(path, data: body);
    final run = DeployRunView.fromJson(raw);
    applyRun(run);
    return run;
  }

  Future<DeployRunView> triggerDeploy(
    int targetId, {
    String? commitSha,
    String? imageTag,
    required String when,
  }) => _runAction('/deploys/targets/$targetId/deploy', <String, dynamic>{
    if (commitSha != null && commitSha.isNotEmpty) 'commitSha': commitSha,
    if (imageTag != null && imageTag.isNotEmpty) 'imageTag': imageTag,
    'when': when,
  });

  Future<DeployRunView> approveRun(int runId, {required String when}) =>
      _runAction('/deploys/runs/$runId/approve', <String, dynamic>{
        'when': when,
      });

  Future<DeployRunView> rejectRun(int runId, {String? reason}) =>
      _runAction('/deploys/runs/$runId/reject', <String, dynamic>{
        if (reason != null && reason.isNotEmpty) 'reason': reason,
      });

  Future<DeployRunView> cancelRun(int runId) =>
      _runAction('/deploys/runs/$runId/cancel', const <String, dynamic>{});

  Future<DeployRunView> restoreRun(int runId, {required String when}) =>
      _runAction('/deploys/runs/$runId/restore', <String, dynamic>{
        'when': when,
      });

  // ── Encrypted `.env` Management ─────────────────────────────────────────

  Future<DeployEnvContentView> envCurrent(int targetId) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/targets/$targetId/env',
    );
    return DeployEnvContentView.fromJson(raw);
  }

  Future<List<DeployEnvVersionView>> envVersions(int targetId) async {
    final raw = await _api.get<List<dynamic>>(
      '/deploys/targets/$targetId/env/versions',
    );
    return List<DeployEnvVersionView>.unmodifiable(
      asJsonList(raw, DeployEnvVersionView.fromJson),
    );
  }

  Future<DeployEnvContentView> envVersion(int versionId) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/env-versions/$versionId',
    );
    return DeployEnvContentView.fromJson(raw);
  }

  Future<DeployEnvDiff> envPreview(int targetId, String content) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/targets/$targetId/env/preview',
      data: <String, dynamic>{'content': content},
    );
    return DeployEnvDiff.fromJson(raw);
  }

  Future<DeployEnvSaved> envSave(
    int targetId, {
    required String content,
    required bool redeploy,
    String? note,
  }) async {
    final raw = await _api.put<Map<String, dynamic>>(
      '/deploys/targets/$targetId/env',
      data: <String, dynamic>{
        'content': content,
        'redeploy': redeploy,
        if (note != null && note.isNotEmpty) 'note': note,
      },
    );
    final saved = DeployEnvSaved.fromJson(raw);
    if (saved.run != null) applyRun(saved.run!);
    return saved;
  }

  // ── Server Builds (ADR 023 — 9 endpoints) ───────────────────────────────

  Future<void> fetchRecentBuilds() async {
    try {
      final raw = await _api.get<List<dynamic>>('/deploys/builds');
      final next = Map<int, DeployServerBuildView>.from(state.builds);
      for (final b in asJsonList(raw, DeployServerBuildView.fromJson)) {
        next[b.id] = b;
      }
      state = state.copyWith(
        builds: Map<int, DeployServerBuildView>.unmodifiable(next),
        buildsLoaded: true,
      );
    } catch (_) {
      state = state.copyWith(buildsLoaded: true);
    }
  }

  Future<void> openBuildConsole(int buildId) async {
    if (!state.buildConsoles.containsKey(buildId)) {
      state = state.copyWith(
        buildConsoles: Map<int, List<String>>.unmodifiable(<int, List<String>>{
          ...state.buildConsoles,
          buildId: const <String>[],
        }),
      );
    }
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/builds/$buildId/log',
    );
    final log = (raw['log'] as String?) ?? '';
    final fetched = log.isEmpty ? const <String>[] : log.split('\n');
    final existing = state.buildConsoles[buildId] ?? const <String>[];
    final merged = existing.length > fetched.length ? existing : fetched;
    state = state.copyWith(
      buildConsoles: Map<int, List<String>>.unmodifiable(<int, List<String>>{
        ...state.buildConsoles,
        buildId: List<String>.unmodifiable(merged),
      }),
    );
  }

  void closeBuildConsole(int buildId) {
    final next = Map<int, List<String>>.from(state.buildConsoles)
      ..remove(buildId);
    state = state.copyWith(
      buildConsoles: Map<int, List<String>>.unmodifiable(next),
    );
  }

  Future<DeployServerBuildView> startBuild(
    int applicationId, {
    String? reference,
    String? commit,
    bool noCache = false,
  }) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/applications/$applicationId/builds',
      data: <String, dynamic>{
        if (reference != null && reference.isNotEmpty) 'reference': reference,
        if (commit != null && commit.isNotEmpty) 'commit': commit,
        'noCache': noCache,
      },
    );
    final build = DeployServerBuildView.fromJson(raw);
    applyBuild(build);
    return build;
  }

  Future<DeployServerBuildView> rebuild(int buildId) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/deploys/builds/$buildId/rebuild',
      data: const <String, dynamic>{},
    );
    final build = DeployServerBuildView.fromJson(raw);
    applyBuild(build);
    return build;
  }

  Future<void> cancelBuild(int buildId) async {
    await _api.post<dynamic>(
      '/deploys/builds/$buildId/cancel',
      data: const <String, dynamic>{},
    );
  }

  Future<List<DeployBuildSecretView>> buildSecrets(int applicationId) async {
    final raw = await _api.get<List<dynamic>>(
      '/deploys/applications/$applicationId/build-secrets',
    );
    return List<DeployBuildSecretView>.unmodifiable(
      asJsonList(raw, DeployBuildSecretView.fromJson),
    );
  }

  Future<List<DeployBuildSecretView>> saveBuildSecret(
    int applicationId,
    String name,
    String value,
  ) async {
    final raw = await _api.post<List<dynamic>>(
      '/deploys/applications/$applicationId/build-secrets',
      data: <String, dynamic>{'name': name, 'value': value},
    );
    return List<DeployBuildSecretView>.unmodifiable(
      asJsonList(raw, DeployBuildSecretView.fromJson),
    );
  }

  Future<void> removeBuildSecret(int applicationId, String name) async {
    await _api.delete<dynamic>(
      '/deploys/applications/$applicationId/build-secrets/${Uri.encodeComponent(name)}',
    );
  }

  Future<BuilderReadiness> builderReadiness(int probeId) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/deploys/agents/$probeId/builder',
    );
    return BuilderReadiness.fromJson(raw);
  }
}

final deploysProvider = NotifierProvider<DeploysNotifier, DeploysState>(
  DeploysNotifier.new,
);

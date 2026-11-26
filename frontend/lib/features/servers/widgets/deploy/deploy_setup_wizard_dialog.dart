import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy/deploy_env_editor.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy/deploy_repository_picker.dart';
import 'package:netmonitor_app/features/servers/widgets/docker_history_widgets.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Wizard completo de 3 etapas para automatizar uma stack (`DeploySetupDialog`).
Future<void> showDeploySetupDialog(
  BuildContext context, {
  int? presetApplicationId,
  int? presetProbeId,
  String? presetProject,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Automatizar deploy contínuo',
    subtitle:
        'Passo 1: Repositório · Passo 2: Servidor alvo · Passo 3: Variáveis (.env)',
    maxWidth: 780,
    builder: (_) => _DeploySetupBody(
      presetApplicationId: presetApplicationId,
      presetProbeId: presetProbeId,
      presetProject: presetProject,
    ),
  );
}

class _DeploySetupBody extends ConsumerStatefulWidget {
  const _DeploySetupBody({
    this.presetApplicationId,
    this.presetProbeId,
    this.presetProject,
  });
  final int? presetApplicationId;
  final int? presetProbeId;
  final String? presetProject;

  @override
  ConsumerState<_DeploySetupBody> createState() => _DeploySetupBodyState();
}

class _DeploySetupBodyState extends ConsumerState<_DeploySetupBody> {
  int _step = 0;

  // Step 1: Application
  int? _existingAppId;
  String _builder = 'netmonitor';
  int? _githubConnectionId;
  int? _credentialId;
  int? _buildProbeId;
  final TextEditingController _appNameCtrl = TextEditingController();
  final TextEditingController _repoCtrl = TextEditingController();
  final TextEditingController _composeFilesCtrl = TextEditingController(
    text: 'docker-compose.yml',
  );
  final TextEditingController _tagVarCtrl = TextEditingController(
    text: 'IMAGE_TAG',
  );
  final String _rollout = 'sequential';

  // Step 2: Target
  final TextEditingController _targetNameCtrl = TextEditingController(
    text: 'Produção',
  );
  late final TextEditingController _projectCtrl;
  final TextEditingController _refCtrl = TextEditingController(text: 'main');
  int? _probeId;
  bool _requiresApproval = false;
  final TextEditingController _windowStartCtrl = TextEditingController();
  final TextEditingController _windowEndCtrl = TextEditingController();
  int? _backupConnectionId;
  bool _autoRollback = true;
  String _vulnPolicy = 'block_critical';
  final bool _requireProvenance = false;

  // Step 3: Initial .env
  String _envContent = '';
  bool _envTouched = false;
  List<DeployEnvTemplateView> _envTemplates = const <DeployEnvTemplateView>[];
  String? _activeEnvTemplateName;
  String? _discoveredRepoKey;

  DeployReadinessView? _readiness;
  List<DeployBuildImage> _discoveredImages = const <DeployBuildImage>[];
  List<DeployRepositoryView> _repositories = const <DeployRepositoryView>[];
  DeployReferencesView? _references;
  bool _loadingRepos = false;
  String? _reposError;
  bool _showRepoPicker = true;
  int _repoLoadSeq = 0;
  String? _composeSummary;
  String? _autoFilledAppName;
  String? _autoFilledProject;
  bool _refManuallyEdited = false;
  bool _discovering = false;
  bool _saving = false;
  String? _error;

  DeployGithubConnectionView? _preferredConnection(
    List<DeployGithubConnectionView> connections,
  ) {
    if (connections.isEmpty) return null;
    return connections.where((c) => c.installed).firstOrNull ??
        connections.first;
  }

  DeployGithubConnectionView? _connectionById(
    List<DeployGithubConnectionView> connections,
    int? id,
  ) {
    if (id == null) return null;
    for (final c in connections) {
      if (c.id == id) return c;
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    _existingAppId = widget.presetApplicationId;
    _probeId = widget.presetProbeId;
    _projectCtrl = TextEditingController(text: widget.presetProject ?? '');
    final initialDeploys = ref.read(deploysProvider);
    final initialConnection = _preferredConnection(
      initialDeploys.githubConnections,
    );
    _githubConnectionId = initialConnection?.id;
    _probeId ??= initialDeploys.agents
        .where((a) => a.allowsDeploy)
        .firstOrNull
        ?.id;
    _buildProbeId = initialDeploys.agents
        .where((a) => a.allowsBuild)
        .firstOrNull
        ?.id;

    Future<void>.microtask(() async {
      if (initialConnection != null && initialConnection.installed) {
        _loadRepositories(initialConnection);
      }
      await Future.wait<void>(<Future<void>>[
        ref.read(deploysProvider.notifier).fetchAgents(),
        ref.read(databasesProvider.notifier).fetchConnections(),
      ]);
      if (!mounted) return;
      final deploys = ref.read(deploysProvider);
      if (_githubConnectionId == null && deploys.githubConnections.isNotEmpty) {
        final conn = _preferredConnection(deploys.githubConnections);
        if (conn != null) {
          setState(() => _githubConnectionId = conn.id);
          if (conn.installed) _loadRepositories(conn);
        }
      }
      if (_probeId == null) {
        final deployAgent = deploys.agents
            .where((a) => a.allowsDeploy)
            .firstOrNull;
        if (deployAgent != null) setState(() => _probeId = deployAgent.id);
      }
      final buildAgent = deploys.agents.where((a) => a.allowsBuild).firstOrNull;
      if (buildAgent != null) setState(() => _buildProbeId = buildAgent.id);
    });
  }

  Future<void> _loadRepositories(DeployGithubConnectionView connection) async {
    final seq = ++_repoLoadSeq;
    setState(() {
      _loadingRepos = true;
      _reposError = null;
    });
    try {
      final list = await ref
          .read(deploysProvider.notifier)
          .githubRepositories(connectionId: connection.id);
      if (!mounted || seq != _repoLoadSeq) return;
      setState(() {
        _repositories = list;
        _loadingRepos = false;
      });
    } catch (err) {
      if (!mounted || seq != _repoLoadSeq) return;
      setState(() {
        _reposError = describeError(
          err,
          fallback: 'Não foi possível listar os repositórios.',
        );
        _loadingRepos = false;
      });
    }
  }

  Future<void> _loadReferences(String repository) async {
    try {
      final refs = await ref
          .read(deploysProvider.notifier)
          .githubReferences(
            repository: repository,
            connectionId: _githubConnectionId,
            credentialId: _credentialId,
          );
      if (!mounted || _repoCtrl.text.trim() != repository) return;
      setState(() {
        _references = refs;
        if (!_refManuallyEdited && refs.defaultBranch.isNotEmpty) {
          _refCtrl.text = refs.defaultBranch;
        }
      });
    } catch (_) {
      // Branches são auxiliares — o campo manual continua funcionando.
    }
  }

  void _syncSuggestedProject() {
    if (_existingAppId != null) return;
    final current = _projectCtrl.text.trim();
    if (current.isNotEmpty && current != _autoFilledProject) return;
    final suggested = suggestComposeProjectName(
      appName: _appNameCtrl.text,
      repository: _repoCtrl.text,
    );
    if (suggested.isNotEmpty) {
      _projectCtrl.text = suggested;
      _autoFilledProject = suggested;
    }
  }

  void _selectRepository(DeployRepositoryView repo) {
    final repoName = repo.fullName.split('/').last;
    final slugName = suggestComposeProjectName(
      appName: repoName,
      repository: repo.fullName,
    );
    setState(() {
      _repoCtrl.text = repo.fullName;
      _showRepoPicker = false;
      _error = null;
      _composeSummary = null;
      _references = null;
      _envTemplates = const <DeployEnvTemplateView>[];
      _activeEnvTemplateName = null;
      _discoveredRepoKey = null;
      if (!_envTouched) {
        _envContent = '';
      }
      if (!_refManuallyEdited && repo.defaultBranch.isNotEmpty) {
        _refCtrl.text = repo.defaultBranch;
      }
      if (_appNameCtrl.text.trim().isEmpty ||
          _appNameCtrl.text.trim() == _autoFilledAppName) {
        _appNameCtrl.text = repoName;
        _autoFilledAppName = repoName;
      }
      if ((_projectCtrl.text.trim().isEmpty ||
              _projectCtrl.text.trim() == _autoFilledProject) &&
          slugName.isNotEmpty) {
        _projectCtrl.text = slugName;
        _autoFilledProject = slugName;
      }
    });
    _discoverCompose(reference: repo.defaultBranch);
    _loadReferences(repo.fullName);
  }

  void _onGithubConnectionChanged(
    int? nextId,
    List<DeployGithubConnectionView> connections,
  ) {
    final conn = _connectionById(connections, nextId);
    setState(() {
      _githubConnectionId = nextId;
      _repositories = const <DeployRepositoryView>[];
      _reposError = null;
      _loadingRepos = false;
      _showRepoPicker = true;
      _references = null;
      _composeSummary = null;
      _envTemplates = const <DeployEnvTemplateView>[];
      _activeEnvTemplateName = null;
      _discoveredRepoKey = null;
    });
    if (conn != null && conn.installed) {
      _loadRepositories(conn);
    } else {
      _repoLoadSeq++;
    }
  }

  @override
  void dispose() {
    _appNameCtrl.dispose();
    _repoCtrl.dispose();
    _composeFilesCtrl.dispose();
    _tagVarCtrl.dispose();
    _targetNameCtrl.dispose();
    _projectCtrl.dispose();
    _refCtrl.dispose();
    _windowStartCtrl.dispose();
    _windowEndCtrl.dispose();
    super.dispose();
  }

  Future<void> _discoverCompose({String? reference}) async {
    final repo = normalizeGithubRepository(_repoCtrl.text);
    if (repo.isEmpty) return;
    if (repo != _repoCtrl.text.trim()) {
      _repoCtrl.text = repo;
    }
    final effectiveRef = (reference ?? _refCtrl.text).trim();
    final repoKey = '$repo@$effectiveRef';
    setState(() {
      _discovering = true;
      _composeSummary = null;
      _error = null;
    });
    try {
      final disc = await ref
          .read(deploysProvider.notifier)
          .githubCompose(
            repository: repo,
            reference: effectiveRef.isEmpty ? null : effectiveRef,
            connectionId: _githubConnectionId,
            credentialId: _credentialId,
          );
      if (!mounted) return;
      final summaryParts = <String>[
        if (disc.suggestedFiles.isNotEmpty) disc.suggestedFiles.join(', '),
        if (disc.services.isNotEmpty)
          countNoun(disc.services.length, 'serviço'),
        if (disc.images.isNotEmpty) countNoun(disc.images.length, 'imagem'),
        if (disc.envTemplates.isNotEmpty) disc.envTemplates.first.name,
      ];
      setState(() {
        _discoveredImages = disc.images;
        _envTemplates = disc.envTemplates;
        _discoveredRepoKey = repoKey;
        if (disc.suggestedFiles.isNotEmpty) {
          _composeFilesCtrl.text = disc.suggestedFiles.join(', ');
        }
        if (disc.imageTagVariable.isNotEmpty) {
          _tagVarCtrl.text = disc.imageTagVariable;
        }
        if (_appNameCtrl.text.trim().isEmpty) {
          final repoName = repo.split('/').last;
          _appNameCtrl.text = repoName;
          _autoFilledAppName = repoName;
        }
        if (_projectCtrl.text.trim().isEmpty) {
          final slugName = repo.split('/').last.toLowerCase();
          _projectCtrl.text = slugName;
          _autoFilledProject = slugName;
        }
        if (!_envTouched &&
            _envContent.trim().isEmpty &&
            disc.envTemplates.isNotEmpty) {
          _envContent = disc.envTemplates.first.content;
          _activeEnvTemplateName = disc.envTemplates.first.name;
        }
        _composeSummary = summaryParts.isNotEmpty
            ? 'Detectado: ${summaryParts.join(' · ')}'
            : (disc.notes.isNotEmpty ? disc.notes.first : null);
        _discovering = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(
          err,
          fallback: 'Erro ao inspecionar repositório.',
        );
        _discovering = false;
      });
    }
  }

  void _ensureEnvTemplatesForStep() {
    if (_existingAppId != null) return;
    final repo = _repoCtrl.text.trim();
    if (repo.isEmpty || _discovering) return;
    final effectiveRef = _refCtrl.text.trim();
    final repoKey = '$repo@$effectiveRef';
    if (_discoveredRepoKey != repoKey) {
      _discoverCompose(reference: effectiveRef);
    }
  }

  Future<void> _checkReadiness() async {
    if (_probeId == null || _projectCtrl.text.trim().isEmpty) return;
    try {
      final res = await ref
          .read(deploysProvider.notifier)
          .readinessFor(_probeId!, _projectCtrl.text.trim());
      if (mounted) setState(() => _readiness = res);
    } catch (_) {}
  }

  Future<void> _submit() async {
    final normalizedRepo = normalizeGithubRepository(_repoCtrl.text);
    if (_existingAppId == null && normalizedRepo.isEmpty) {
      setState(() {
        _step = 0;
        _error = 'Informe ou selecione o repositório (dono/repo) no passo 1.';
      });
      return;
    }
    if (_existingAppId == null && normalizedRepo != _repoCtrl.text.trim()) {
      _repoCtrl.text = normalizedRepo;
    }
    _syncSuggestedProject();
    final deploys = ref.read(deploysProvider);
    final validProbeId =
        deploys.agents.any((a) => a.id == _probeId && a.allowsDeploy)
        ? _probeId
        : null;
    if (validProbeId == null) {
      setState(() {
        _step = 1;
        _error =
            'Escolha um servidor alvo com permissão "deploy" habilitada no passo 2.';
      });
      return;
    }
    if (_projectCtrl.text.trim().isEmpty) {
      setState(() {
        _step = 1;
        _error =
            'Informe o nome do projeto Docker Compose no passo 2 (ex.: minha-app).';
      });
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final composeFiles = _composeFilesCtrl.text
          .split(',')
          .map((e) => e.trim())
          .where((e) => e.isNotEmpty)
          .toList(growable: false);
      final appName = _appNameCtrl.text.trim().isNotEmpty
          ? _appNameCtrl.text.trim()
          : normalizedRepo.split('/').last;
      final slug = sanitizeFileSlug(appName, fallback: 'app')
          .replaceAll(RegExp(r'[^a-z0-9-]+'), '-')
          .replaceAll(RegExp(r'^[-]+|[-]+$'), '');

      await ref
          .read(deploysProvider.notifier)
          .setup(
            DeploySetupInput(
              applicationId: _existingAppId,
              application: _existingAppId != null
                  ? null
                  : DeployApplicationInput(
                      name: appName,
                      slug: slug.isEmpty ? 'app' : slug,
                      repository: normalizedRepo,
                      credentialId: _credentialId,
                      githubConnectionId: _githubConnectionId,
                      composeFiles: composeFiles.isEmpty
                          ? const <String>['docker-compose.yml']
                          : composeFiles,
                      imageTagVariable: _tagVarCtrl.text.trim(),
                      images: _builder == 'netmonitor'
                          ? _discoveredImages
                          : const <DeployBuildImage>[],
                      rollout: _rollout,
                      builder: _builder,
                      buildProbeId: _builder == 'netmonitor'
                          ? _buildProbeId
                          : null,
                    ),
              target: DeployTargetInput(
                name: _targetNameCtrl.text.trim(),
                probeId: validProbeId,
                project: _projectCtrl.text.trim(),
                reference: _refCtrl.text.trim(),
                requiresApproval: _requiresApproval,
                windowStart: _windowStartCtrl.text.trim(),
                windowEnd: _windowEndCtrl.text.trim(),
                backupConnectionId: _backupConnectionId,
                autoRollback: _autoRollback,
                vulnerabilityPolicy: _vulnPolicy,
                requireProvenance: _requireProvenance,
              ),
              env: _envContent.trim().isEmpty ? null : _envContent,
            ),
          );
      if (mounted) Navigator.of(context).pop();
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao configurar deploy.');
        _saving = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);
    final dbConnections = ref.watch(databasesProvider).connections;
    final safeExistingAppId =
        deploys.applications.any((a) => a.id == _existingAppId)
        ? _existingAppId
        : null;
    final safeGithubConnectionId =
        deploys.githubConnections.any((c) => c.id == _githubConnectionId)
        ? _githubConnectionId
        : null;
    final activeGithubConnection = _connectionById(
      deploys.githubConnections,
      safeGithubConnectionId,
    );
    final safeBuildProbeId = deploys.agents.any((a) => a.id == _buildProbeId)
        ? _buildProbeId
        : null;
    final safeProbeId = deploys.agents.any((a) => a.id == _probeId)
        ? _probeId
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SegmentedButton<int>(
          showSelectedIcon: false,
          style: SegmentedButton.styleFrom(
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 8),
          ),
          segments: const <ButtonSegment<int>>[
            ButtonSegment<int>(
              value: 0,
              label: Text('1. Origem', maxLines: 1, softWrap: false),
            ),
            ButtonSegment<int>(
              value: 1,
              label: Text('2. Alvo', maxLines: 1, softWrap: false),
            ),
            ButtonSegment<int>(
              value: 2,
              label: Text('3. .env', maxLines: 1, softWrap: false),
            ),
          ],
          selected: <int>{_step},
          onSelectionChanged: (set) {
            setState(() {
              _step = set.first;
              _error = null;
              if (_step == 1) _syncSuggestedProject();
            });
            if (_step == 1) _checkReadiness();
            if (_step == 2) _ensureEnvTemplatesForStep();
          },
        ),
        const SizedBox(height: 14),

        if (_step == 0) ...<Widget>[
          if (deploys.applications.isNotEmpty) ...<Widget>[
            AppDropdownFormField<int?>(
              initialValue: safeExistingAppId,
              decoration: const InputDecoration(labelText: 'Aplicação'),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text(
                    'Criar nova aplicação',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ...deploys.applications.map(
                  (a) => DropdownMenuItem<int?>(
                    value: a.id,
                    child: Text(
                      '${a.name} (${a.repository})',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ),
              ],
              onChanged: (v) => setState(() => _existingAppId = v),
            ),
            const SizedBox(height: 12),
          ],
          if (_existingAppId == null) ...<Widget>[
            ChoiceCard(
              title: 'O NetMonitor compila (recomendado)',
              subtitle:
                  'O push chega, o builder isolado compila, escaneia e sobe.',
              icon: Icons.build_outlined,
              selected: _builder == 'netmonitor',
              badge: 'Recomendado',
              onTap: () => setState(() => _builder = 'netmonitor'),
            ),
            const SizedBox(height: 8),
            ChoiceCard(
              title: 'O GitHub Actions compila',
              subtitle:
                  'Workflow publica no ghcr.io e avisa a central via OIDC.',
              icon: Icons.code,
              selected: _builder == 'github_actions',
              onTap: () => setState(() => _builder = 'github_actions'),
            ),
            const SizedBox(height: 12),
            if (deploys.githubConnections.isNotEmpty) ...<Widget>[
              AppDropdownFormField<int?>(
                key: ValueKey<int?>(safeGithubConnectionId),
                initialValue: safeGithubConnectionId,
                decoration: const InputDecoration(
                  labelText: 'Conexão GitHub App',
                ),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text(
                      'Sem GitHub App',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  ...deploys.githubConnections.map(
                    (c) => DropdownMenuItem<int?>(
                      value: c.id,
                      child: Text(
                        '${c.account} (${c.appSlug})',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
                ],
                onChanged: (v) =>
                    _onGithubConnectionChanged(v, deploys.githubConnections),
              ),
              const SizedBox(height: 10),
            ],
            InlineFieldActionRow(
              field: TextField(
                controller: _repoCtrl,
                onChanged: (value) {
                  final normalized = normalizeGithubRepository(value);
                  if (normalized != value.trim() &&
                      value.trim().contains('github.com')) {
                    _repoCtrl.value = TextEditingValue(
                      text: normalized,
                      selection: TextSelection.collapsed(
                        offset: normalized.length,
                      ),
                    );
                  }
                  setState(() {
                    _showRepoPicker = true;
                    _composeSummary = null;
                    _error = null;
                  });
                },
                decoration: InputDecoration(
                  labelText: 'Repositório (dono/repo)',
                  hintText: activeGithubConnection != null
                      ? 'Busque ou escolha um repositório abaixo'
                      : 'minha-org/meu-app',
                  prefixIcon: activeGithubConnection != null
                      ? const Icon(Icons.search, size: 18)
                      : null,
                  suffixIcon: _repoCtrl.text.isNotEmpty
                      ? IconButton(
                          tooltip: 'Limpar',
                          icon: const Icon(Icons.close, size: 18),
                          onPressed: () => setState(() {
                            _repoCtrl.clear();
                            _showRepoPicker = true;
                            _composeSummary = null;
                          }),
                        )
                      : null,
                ),
              ),
              action: OutlinedButton.icon(
                onPressed: _discovering || _repoCtrl.text.trim().isEmpty
                    ? null
                    : () {
                        _discoverCompose();
                        _loadReferences(_repoCtrl.text.trim());
                      },
                icon: _discovering
                    ? const SizedBox(
                        width: 15,
                        height: 15,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.search, size: 16),
                label: const Text('Detectar'),
              ),
            ),
            if (activeGithubConnection != null) ...<Widget>[
              const SizedBox(height: 8),
              DeployRepositoryPicker(
                repositories: _repositories,
                query: _repoCtrl.text,
                selectedRepository: _repoCtrl.text,
                loading: _loadingRepos,
                error: _reposError,
                expanded: _showRepoPicker,
                accountLabel: activeGithubConnection.account,
                connectionInstalled: activeGithubConnection.installed,
                onSelect: _selectRepository,
                onRefresh: () => _loadRepositories(activeGithubConnection),
                onToggleExpanded: () =>
                    setState(() => _showRepoPicker = !_showRepoPicker),
              ),
            ],
            if (_composeSummary != null) ...<Widget>[
              const SizedBox(height: 8),
              NestedPanel(
                tone: SemanticTone.info,
                child: Text(
                  _composeSummary!,
                  style: context.textTheme.bodySmall,
                ),
              ),
            ],
            const SizedBox(height: 10),
            TextField(
              controller: _appNameCtrl,
              onChanged: (_) => _syncSuggestedProject(),
              decoration: const InputDecoration(labelText: 'Nome da aplicação'),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _composeFilesCtrl,
              decoration: const InputDecoration(
                labelText: 'Arquivos Compose (separados por vírgula)',
              ),
            ),
            const SizedBox(height: 10),
            if (_builder == 'netmonitor')
              AppDropdownFormField<int?>(
                key: ValueKey<int?>(safeBuildProbeId),
                initialValue: safeBuildProbeId,
                decoration: const InputDecoration(
                  labelText: 'Agente builder (permissão build)',
                ),
                items: deploys.agents
                    .map(
                      (a) => DropdownMenuItem<int?>(
                        value: a.id,
                        enabled: a.allowsBuild,
                        child: Text(
                          '${a.name}${!a.allowsBuild ? ' (sem permissão build)' : ''}',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    )
                    .toList(growable: false),
                onChanged: (v) => setState(() => _buildProbeId = v),
              ),
          ],
        ] else if (_step == 1) ...<Widget>[
          TextField(
            controller: _targetNameCtrl,
            decoration: const InputDecoration(
              labelText: 'Nome do alvo (ex.: Produção, Homologação)',
            ),
          ),
          const SizedBox(height: 10),
          AppDropdownFormField<int?>(
            key: ValueKey<int?>(safeProbeId),
            initialValue: safeProbeId,
            decoration: const InputDecoration(
              labelText: 'Servidor alvo (permissão deploy)',
            ),
            items: deploys.agents
                .map(
                  (a) => DropdownMenuItem<int?>(
                    value: a.id,
                    enabled: a.allowsDeploy,
                    child: Text(
                      '${a.name}${!a.allowsDeploy ? ' (sem permissão deploy)' : (!a.connected ? ' (offline)' : '')}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) {
              setState(() => _probeId = v);
              _checkReadiness();
            },
          ),
          const SizedBox(height: 10),
          ResponsiveFieldRow(
            breakpoint: 480,
            spacing: 8,
            runSpacing: 10,
            children: <Widget>[
              TextField(
                controller: _projectCtrl,
                onChanged: (_) => setState(() => _error = null),
                decoration: const InputDecoration(
                  labelText: 'Projeto Docker Compose',
                  hintText: 'ex.: minha-app',
                ),
                onEditingComplete: _checkReadiness,
              ),
              TextField(
                controller: _refCtrl,
                onChanged: (_) => _refManuallyEdited = true,
                decoration: const InputDecoration(
                  labelText: 'Branch ou tag',
                  hintText: 'main ou v*',
                ),
              ),
            ],
          ),
          if (_references != null &&
              _references!.branches.isNotEmpty) ...<Widget>[
            const SizedBox(height: 8),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: _references!.branches
                  .take(8)
                  .map(
                    (branch) => ChoiceChip(
                      visualDensity: VisualDensity.compact,
                      selected: _refCtrl.text.trim() == branch,
                      label: Text(branch),
                      onSelected: (_) => setState(() {
                        _refManuallyEdited = true;
                        _refCtrl.text = branch;
                      }),
                    ),
                  )
                  .toList(growable: false),
            ),
          ],
          const SizedBox(height: 10),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _requiresApproval,
            title: const Text('Exigir aprovação antes de publicar'),
            onChanged: (v) => setState(() => _requiresApproval = v),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _autoRollback,
            title: const Text('Rollback automático se falhar saúde'),
            onChanged: (v) => setState(() => _autoRollback = v),
          ),
          ResponsiveFieldRow(
            breakpoint: 480,
            spacing: 8,
            runSpacing: 10,
            children: <Widget>[
              TextField(
                controller: _windowStartCtrl,
                decoration: const InputDecoration(
                  labelText: 'Janela início (HH:MM)',
                  hintText: '23:00',
                ),
              ),
              TextField(
                controller: _windowEndCtrl,
                decoration: const InputDecoration(
                  labelText: 'Janela fim (HH:MM)',
                  hintText: '05:00',
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          AppDropdownFormField<int?>(
            initialValue: dbConnections.any((c) => c.id == _backupConnectionId)
                ? _backupConnectionId
                : null,
            decoration: const InputDecoration(
              labelText: 'Backup de banco pré-deploy (opcional)',
            ),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text('Nenhum', overflow: TextOverflow.ellipsis),
              ),
              ...dbConnections.map(
                (c) => DropdownMenuItem<int?>(
                  value: c.id,
                  enabled: c.storageDestinationId != null,
                  child: Text(
                    '${c.name}${c.storageDestinationId == null ? ' (sem destino de backup)' : ''}',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
            ],
            onChanged: (v) => setState(() => _backupConnectionId = v),
          ),
          if (_builder == 'netmonitor') ...<Widget>[
            const SizedBox(height: 10),
            AppDropdownFormField<String>(
              initialValue: _vulnPolicy,
              decoration: const InputDecoration(
                labelText: 'Política de vulnerabilidades',
              ),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'block_critical',
                  child: Text(
                    'Bloquear crítica',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                DropdownMenuItem(
                  value: 'block_high',
                  child: Text(
                    'Bloquear alta ou crítica',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                DropdownMenuItem(
                  value: 'warn',
                  child: Text('Só avisar', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) {
                if (v != null) setState(() => _vulnPolicy = v);
              },
            ),
          ],
          if (_readiness != null && _readiness!.checks.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            NestedPanel(
              tone: _readiness!.ready
                  ? SemanticTone.success
                  : SemanticTone.warning,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: _readiness!.checks
                    .map((c) {
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Text('• [${c.level.toUpperCase()}] ${c.title}'),
                      );
                    })
                    .toList(growable: false),
              ),
            ),
          ],
        ] else ...<Widget>[
          DeployEnvStepEditor(
            content: _envContent,
            templates: _envTemplates,
            activeTemplateName: _activeEnvTemplateName,
            loadingTemplates: _discovering,
            hasRepository:
                _existingAppId == null && _repoCtrl.text.trim().isNotEmpty,
            onContentChanged: (value) {
              _envTouched = true;
              _envContent = value;
            },
            onSelectTemplate: (tpl) => setState(() {
              _envTouched = false;
              _envContent = tpl.content;
              _activeEnvTemplateName = tpl.name;
            }),
            onClear: () => setState(() {
              _envTouched = true;
              _envContent = '';
              _activeEnvTemplateName = null;
            }),
            onReloadTemplates:
                _existingAppId == null && _repoCtrl.text.trim().isNotEmpty
                ? () {
                    setState(() => _envTouched = false);
                    _discoverCompose(reference: _refCtrl.text.trim());
                  }
                : null,
          ),
        ],

        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: <Widget>[
            if (_step > 0)
              TextButton(
                onPressed: () => setState(() => _step--),
                child: const Text('Voltar'),
              )
            else
              const SizedBox.shrink(),
            if (_step < 2)
              FilledButton(
                onPressed: () {
                  setState(() {
                    _step++;
                    _error = null;
                    if (_step == 1) _syncSuggestedProject();
                  });
                  if (_step == 1) _checkReadiness();
                  if (_step == 2) _ensureEnvTemplatesForStep();
                },
                child: const Text('Próximo'),
              )
            else
              FilledButton.icon(
                onPressed: _saving ? null : _submit,
                icon: const Icon(Icons.rocket_launch),
                label: const Text('Concluir configuração'),
              ),
          ],
        ),
      ],
    );
  }
}

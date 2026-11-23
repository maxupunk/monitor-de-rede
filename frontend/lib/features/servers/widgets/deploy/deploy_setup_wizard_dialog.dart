import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
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
  final TextEditingController _envCtrl = TextEditingController();

  DeployReadinessView? _readiness;
  List<DeployBuildImage> _discoveredImages = const <DeployBuildImage>[];
  bool _discovering = false;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _existingAppId = widget.presetApplicationId;
    _probeId = widget.presetProbeId;
    _projectCtrl = TextEditingController(text: widget.presetProject ?? '');
    Future<void>.microtask(() async {
      await Future.wait<void>(<Future<void>>[
        ref.read(deploysProvider.notifier).fetchAgents(),
        ref.read(databasesProvider.notifier).fetchConnections(),
      ]);
      if (!mounted) return;
      final deploys = ref.read(deploysProvider);
      if (deploys.githubConnections.isNotEmpty) {
        setState(
          () => _githubConnectionId = deploys.githubConnections.first.id,
        );
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
    _envCtrl.dispose();
    super.dispose();
  }

  Future<void> _discoverCompose() async {
    final repo = _repoCtrl.text.trim();
    if (repo.isEmpty) return;
    setState(() {
      _discovering = true;
      _error = null;
    });
    try {
      final disc = await ref
          .read(deploysProvider.notifier)
          .githubCompose(
            repository: repo,
            connectionId: _githubConnectionId,
            credentialId: _credentialId,
          );
      if (!mounted) return;
      setState(() {
        _discoveredImages = disc.images;
        if (disc.suggestedFiles.isNotEmpty) {
          _composeFilesCtrl.text = disc.suggestedFiles.join(', ');
        }
        if (disc.imageTagVariable.isNotEmpty) {
          _tagVarCtrl.text = disc.imageTagVariable;
        }
        if (_appNameCtrl.text.trim().isEmpty) {
          _appNameCtrl.text = repo.split('/').last;
        }
        if (_projectCtrl.text.trim().isEmpty) {
          _projectCtrl.text = repo.split('/').last.toLowerCase();
        }
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
    if (_probeId == null || _projectCtrl.text.trim().isEmpty) {
      setState(
        () => _error = 'Escolha o servidor alvo e o nome do projeto compose.',
      );
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
          : _repoCtrl.text.trim().split('/').last;
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
                      repository: _repoCtrl.text.trim(),
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
                probeId: _probeId!,
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
              env: _envCtrl.text.trim().isEmpty ? null : _envCtrl.text,
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
            setState(() => _step = set.first);
            if (_step == 1) _checkReadiness();
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
                onChanged: (v) => setState(() => _githubConnectionId = v),
              ),
              const SizedBox(height: 10),
            ],
            InlineFieldActionRow(
              field: TextField(
                controller: _repoCtrl,
                decoration: const InputDecoration(
                  labelText: 'Repositório (dono/repo)',
                  hintText: 'minha-org/meu-app',
                ),
              ),
              action: OutlinedButton.icon(
                onPressed: _discovering ? null : _discoverCompose,
                icon: const Icon(Icons.search, size: 16),
                label: const Text('Detectar'),
              ),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _appNameCtrl,
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
                initialValue: deploys.agents.any((a) => a.id == _buildProbeId)
                    ? _buildProbeId
                    : null,
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
            initialValue: deploys.agents.any((a) => a.id == _probeId)
                ? _probeId
                : null,
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
                decoration: const InputDecoration(
                  labelText: 'Projeto Docker Compose',
                ),
                onEditingComplete: _checkReadiness,
              ),
              TextField(
                controller: _refCtrl,
                decoration: const InputDecoration(
                  labelText: 'Branch ou tag',
                  hintText: 'main ou v*',
                ),
              ),
            ],
          ),
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
          TextField(
            controller: _envCtrl,
            maxLines: 10,
            style: const TextStyle(fontFamily: 'monospace', fontSize: 13),
            decoration: const InputDecoration(
              labelText: 'Arquivo .env inicial (opcional — cifrado na central)',
              alignLabelWithHint: true,
            ),
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
                  setState(() => _step++);
                  if (_step == 1) _checkReadiness();
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

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_run_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Diálogo de Conexões GitHub (GitHub App via manifest + tokens PAT — ADR 021).
Future<void> showDeployConnectionsDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Conexões com o GitHub',
    subtitle: 'GitHub App da central (recomendado) ou token clássico PAT',
    maxWidth: 740,
    builder: (_) => const _DeployConnectionsBody(),
  );
}

class _DeployConnectionsBody extends ConsumerStatefulWidget {
  const _DeployConnectionsBody();

  @override
  ConsumerState<_DeployConnectionsBody> createState() =>
      _DeployConnectionsBodyState();
}

class _DeployConnectionsBodyState
    extends ConsumerState<_DeployConnectionsBody> {
  final TextEditingController _orgCtrl = TextEditingController();
  bool _allowWrite = true;
  bool _creatingManifest = false;
  DeployGithubManifestView? _manifestReady;

  // PAT form
  final TextEditingController _credNameCtrl = TextEditingController();
  final TextEditingController _credRegistryCtrl = TextEditingController(
    text: 'ghcr.io',
  );
  final TextEditingController _credUserCtrl = TextEditingController();
  final TextEditingController _credSecretCtrl = TextEditingController();
  bool _savingCred = false;
  String? _error;

  @override
  void dispose() {
    _orgCtrl.dispose();
    _credNameCtrl.dispose();
    _credRegistryCtrl.dispose();
    _credUserCtrl.dispose();
    _credSecretCtrl.dispose();
    super.dispose();
  }

  Future<void> _startGithubManifest() async {
    setState(() {
      _creatingManifest = true;
      _error = null;
    });
    try {
      final origin = kIsWeb
          ? Uri.base.origin
          : ref.read(serverConfigProvider).baseUrl;
      final manifest = await ref
          .read(deploysProvider.notifier)
          .githubManifest(
            origin: origin,
            organization: _orgCtrl.text.trim(),
            allowWrite: _allowWrite,
          );
      if (!mounted) return;
      setState(() {
        _manifestReady = manifest;
        _creatingManifest = false;
      });
      final uri = Uri.tryParse(manifest.actionUrl);
      if (uri != null) {
        await launchUrl(uri, mode: LaunchMode.externalApplication);
      }
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _creatingManifest = false;
      });
    }
  }

  Future<void> _saveCredential() async {
    if (_credNameCtrl.text.trim().isEmpty ||
        _credUserCtrl.text.trim().isEmpty) {
      setState(() => _error = 'Informe nome e usuário da credencial.');
      return;
    }
    setState(() {
      _savingCred = true;
      _error = null;
    });
    try {
      await ref
          .read(deploysProvider.notifier)
          .saveCredential(null, <String, dynamic>{
            'name': _credNameCtrl.text.trim(),
            'registry': _credRegistryCtrl.text.trim(),
            'username': _credUserCtrl.text.trim(),
            'secret': _credSecretCtrl.text,
          });
      if (!mounted) return;
      _credNameCtrl.clear();
      _credUserCtrl.clear();
      _credSecretCtrl.clear();
      setState(() => _savingCred = false);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _savingCred = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          'GitHub App da central',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 6),
        Text(
          'A chave privada do GitHub App nunca sai da central; o agente recebe apenas tokens de 1h somente leitura.',
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 10),
        if (deploys.githubConnections.isNotEmpty) ...<Widget>[
          ...deploys.githubConnections.map(
            (conn) => Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: NestedPanel(
                tone: conn.installed
                    ? SemanticTone.success
                    : SemanticTone.warning,
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            '${conn.account} (${conn.appSlug})',
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          Text(
                            conn.installed
                                ? 'Instalado nos repositórios'
                                : 'Falta instalar nos repositórios',
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    Wrap(
                      spacing: 4,
                      children: <Widget>[
                        if (!conn.installed && conn.installUrl.isNotEmpty)
                          FilledButton.tonal(
                            onPressed: () => launchUrl(
                              Uri.parse(conn.installUrl),
                              mode: LaunchMode.externalApplication,
                            ),
                            child: const Text('Instalar'),
                          ),
                        IconButton(
                          tooltip: 'Atualizar',
                          icon: const Icon(Icons.refresh, size: 18),
                          onPressed: () => ref
                              .read(deploysProvider.notifier)
                              .refreshGithub(conn.id),
                        ),
                        IconButton(
                          tooltip: 'Remover',
                          icon: const Icon(Icons.delete_outline, size: 18),
                          onPressed: () => ref
                              .read(deploysProvider.notifier)
                              .removeGithub(conn.id),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
          const SizedBox(height: 10),
        ],
        NestedPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              TextField(
                controller: _orgCtrl,
                decoration: const InputDecoration(
                  labelText:
                      'Organização GitHub (opcional — deixe em branco para conta pessoal)',
                ),
              ),
              const SizedBox(height: 6),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                value: _allowWrite,
                title: const Text('Permitir abrir PR do workflow'),
                subtitle: const Text(
                  'Sempre em branch própria do NetMonitor, nunca na branch padrão.',
                ),
                onChanged: (v) => setState(() => _allowWrite = v),
              ),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton.icon(
                  onPressed: _creatingManifest ? null : _startGithubManifest,
                  icon: const Icon(Icons.open_in_new, size: 16),
                  label: const Text('Conectar GitHub App'),
                ),
              ),
              if (_manifestReady != null) ...<Widget>[
                const SizedBox(height: 10),
                CodeViewer(
                  title: 'Manifest gerado (${_manifestReady!.actionUrl})',
                  code: _manifestReady!.manifest,
                  maxHeight: 160,
                ),
              ],
            ],
          ),
        ),
        const Divider(height: 28),
        Text(
          'Tokens clássicos (PAT / ghcr.io)',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 8),
        ...deploys.credentials.map(
          (cred) => ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.vpn_key_outlined),
            title: Text(
              cred.name,
              style: const TextStyle(fontWeight: FontWeight.w700),
            ),
            subtitle: Text('${cred.username} · ${cred.registry ?? 'GitHub'}'),
            trailing: IconButton(
              icon: const Icon(Icons.delete_outline),
              onPressed: () =>
                  ref.read(deploysProvider.notifier).removeCredential(cred.id),
            ),
          ),
        ),
        const SizedBox(height: 8),
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            TextField(
              controller: _credNameCtrl,
              decoration: const InputDecoration(
                labelText: 'Nome',
                isDense: true,
              ),
            ),
            TextField(
              controller: _credRegistryCtrl,
              decoration: const InputDecoration(
                labelText: 'Registry',
                isDense: true,
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            TextField(
              controller: _credUserCtrl,
              decoration: const InputDecoration(
                labelText: 'Usuário',
                isDense: true,
              ),
            ),
            TextField(
              controller: _credSecretCtrl,
              obscureText: true,
              decoration: const InputDecoration(
                labelText: 'Token / PAT',
                isDense: true,
              ),
            ),
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 10),
        Align(
          alignment: Alignment.centerRight,
          child: OutlinedButton.icon(
            onPressed: _savingCred ? null : _saveCredential,
            icon: const Icon(Icons.add),
            label: const Text('Adicionar token'),
          ),
        ),
      ],
    );
  }
}

/// Wizard completo de 3 etapas para automatizar uma stack (`DeploySetupDialog`).
Future<void> showDeploySetupDialog(
  BuildContext context, {
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
      presetProbeId: presetProbeId,
      presetProject: presetProject,
    ),
  );
}

class _DeploySetupBody extends ConsumerStatefulWidget {
  const _DeploySetupBody({this.presetProbeId, this.presetProject});
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
  bool _discovering = false;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
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
        _error = err.toString();
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
      final slug = appName.toLowerCase().replaceAll(
        RegExp(r'[^a-z0-9-]+'),
        '-',
      );

      await ref.read(deploysProvider.notifier).setup(<String, dynamic>{
        'applicationId': _existingAppId,
        if (_existingAppId == null)
          'application': <String, dynamic>{
            'name': appName,
            'slug': slug,
            'repository': _repoCtrl.text.trim(),
            'credentialId': _credentialId,
            'githubConnectionId': _githubConnectionId,
            'composeFiles': composeFiles.isEmpty
                ? <String>['docker-compose.yml']
                : composeFiles,
            'imageTagVariable': _tagVarCtrl.text.trim(),
            'images': const <Map<String, dynamic>>[],
            'rollout': _rollout,
            'builder': _builder,
            'buildProbeId': _builder == 'netmonitor' ? _buildProbeId : null,
            'buildArgs': const <Map<String, dynamic>>[],
          },
        'target': <String, dynamic>{
          'name': _targetNameCtrl.text.trim(),
          'probeId': _probeId,
          'project': _projectCtrl.text.trim(),
          'reference': _refCtrl.text.trim(),
          'requiresApproval': _requiresApproval,
          'windowStart': _windowStartCtrl.text.trim().isEmpty
              ? null
              : _windowStartCtrl.text.trim(),
          'windowEnd': _windowEndCtrl.text.trim().isEmpty
              ? null
              : _windowEndCtrl.text.trim(),
          'backupConnectionId': _backupConnectionId,
          'waitTimeoutSecs': 180,
          'autoRollback': _autoRollback,
          'keepImages': 3,
          'enabled': true,
          'position': 0,
          'vulnerabilityPolicy': _vulnPolicy,
          'requireProvenance': _requireProvenance,
        },
        if (_envCtrl.text.trim().isNotEmpty) 'env': _envCtrl.text,
      });
      if (mounted) Navigator.of(context).pop();
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _saving = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);
    final dbConnections = ref.watch(databasesProvider).connections;

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
            ButtonSegment(
              value: 0,
              label: Text('1. Origem', maxLines: 1, softWrap: false),
            ),
            ButtonSegment(
              value: 1,
              label: Text('2. Alvo', maxLines: 1, softWrap: false),
            ),
            ButtonSegment(
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
              initialValue: _existingAppId,
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
                initialValue: _githubConnectionId,
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
            LayoutBuilder(
              builder: (context, constraints) {
                final repoField = TextField(
                  controller: _repoCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Repositório (dono/repo)',
                    hintText: 'minha-org/meu-app',
                  ),
                );
                final detectButton = OutlinedButton.icon(
                  onPressed: _discovering ? null : _discoverCompose,
                  icon: const Icon(Icons.search, size: 16),
                  label: const Text('Detectar'),
                );
                if (constraints.maxWidth < 420) {
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      repoField,
                      const SizedBox(height: 8),
                      Align(
                        alignment: Alignment.centerRight,
                        child: detectButton,
                      ),
                    ],
                  );
                }
                return Row(
                  children: <Widget>[
                    Expanded(child: repoField),
                    const SizedBox(width: 8),
                    detectButton,
                  ],
                );
              },
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

/// Diálogo para iniciar publicação manual em um alvo (`DeployPublishDialog`).
Future<void> showDeployPublishDialog(
  BuildContext context, {
  required DeployTargetView target,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Publicar em ${target.name}',
    subtitle:
        'Projeto ${target.project} · branch ${refShortName(target.reference)}',
    builder: (_) => _DeployPublishBody(target: target),
  );
}

class _DeployPublishBody extends ConsumerStatefulWidget {
  const _DeployPublishBody({required this.target});
  final DeployTargetView target;

  @override
  ConsumerState<_DeployPublishBody> createState() => _DeployPublishBodyState();
}

class _DeployPublishBodyState extends ConsumerState<_DeployPublishBody> {
  final TextEditingController _shaCtrl = TextEditingController();
  String _when = 'now';
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _shaCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final run = await ref
          .read(deploysProvider.notifier)
          .triggerDeploy(
            widget.target.id,
            commitSha: _shaCtrl.text.trim(),
            when: _when,
          );
      if (!mounted) return;
      Navigator.of(context).pop();
      await showDeployRunDialog(context, runId: run.id, canAdmin: true);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _submitting = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final hasWindow =
        widget.target.windowStart != null &&
        widget.target.windowStart!.isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _shaCtrl,
          decoration: InputDecoration(
            labelText:
                'Commit SHA (opcional — vazio usa o mais recente / reaplica .env)',
            hintText: widget.target.live?.shortSha ?? '40 caracteres hex',
          ),
        ),
        const SizedBox(height: 12),
        if (hasWindow)
          SegmentedButton<String>(
            showSelectedIcon: false,
            style: SegmentedButton.styleFrom(
              visualDensity: VisualDensity.compact,
              padding: const EdgeInsets.symmetric(horizontal: 8),
            ),
            segments: const <ButtonSegment<String>>[
              ButtonSegment(
                value: 'now',
                label: Text('Publicar agora', maxLines: 1, softWrap: false),
              ),
              ButtonSegment(
                value: 'window',
                label: Text('Na janela', maxLines: 1, softWrap: false),
              ),
            ],
            selected: <String>{_when},
            onSelectionChanged: (set) => setState(() => _when = set.first),
          ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _submitting ? null : _submit,
            icon: const Icon(Icons.rocket_launch),
            label: const Text('Publicar'),
          ),
        ),
      ],
    );
  }
}

/// Diálogo para disparar compilação no servidor (`DeployBuildStartDialog`).
Future<void> showDeployBuildStartDialog(
  BuildContext context, {
  required DeployApplicationView application,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Compilar ${application.name}',
    subtitle: 'Compila no builder dedicado e envia para o Registry interno',
    builder: (_) => _DeployBuildStartBody(application: application),
  );
}

class _DeployBuildStartBody extends ConsumerStatefulWidget {
  const _DeployBuildStartBody({required this.application});
  final DeployApplicationView application;

  @override
  ConsumerState<_DeployBuildStartBody> createState() =>
      _DeployBuildStartBodyState();
}

class _DeployBuildStartBodyState extends ConsumerState<_DeployBuildStartBody> {
  late final TextEditingController _refCtrl;
  final TextEditingController _commitCtrl = TextEditingController();
  bool _noCache = false;
  BuilderReadiness? _builderStatus;
  bool _submitting = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final firstRef = widget.application.targets.isNotEmpty
        ? refShortName(widget.application.targets.first.reference)
        : 'main';
    _refCtrl = TextEditingController(
      text: firstRef.contains('*') ? 'main' : firstRef,
    );
    final probeId =
        widget.application.buildProbeId ??
        widget.application.targets.firstOrNull?.agent?.id;
    if (probeId != null) {
      Future<void>.microtask(() async {
        try {
          final status = await ref
              .read(deploysProvider.notifier)
              .builderReadiness(probeId);
          if (mounted) setState(() => _builderStatus = status);
        } catch (_) {}
      });
    }
  }

  @override
  void dispose() {
    _refCtrl.dispose();
    _commitCtrl.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final build = await ref
          .read(deploysProvider.notifier)
          .startBuild(
            widget.application.id,
            reference: _refCtrl.text.trim(),
            commit: _commitCtrl.text.trim(),
            noCache: _noCache,
          );
      if (!mounted) return;
      Navigator.of(context).pop();
      await showDeployBuildDialog(context, buildId: build.id, canAdmin: true);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _submitting = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (_builderStatus != null) ...<Widget>[
          NestedPanel(
            tone: _builderStatus!.builderReady
                ? SemanticTone.success
                : SemanticTone.warning,
            child: Text(
              _builderStatus!.builderReady
                  ? 'Builder pronto · Buildx ${_builderStatus!.buildxVersion ?? ''} · '
                        '${formatBytes(_builderStatus!.freeBytes)} livres · ${_builderStatus!.cpus} vCPUs'
                  : 'Builder ainda não está pronto neste agente.',
              style: context.textTheme.bodySmall,
            ),
          ),
          const SizedBox(height: 12),
        ],
        TextField(
          controller: _refCtrl,
          decoration: const InputDecoration(labelText: 'Branch ou tag'),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _commitCtrl,
          decoration: const InputDecoration(
            labelText: 'Commit SHA exato (opcional)',
          ),
        ),
        const SizedBox(height: 8),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _noCache,
          title: const Text('Compilar sem cache (--no-cache)'),
          onChanged: (v) => setState(() => _noCache = v),
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 14),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _submitting ? null : _start,
            icon: const Icon(Icons.build_outlined),
            label: const Text('Iniciar compilação'),
          ),
        ),
      ],
    );
  }
}

/// Diálogo de workflow GitHub Actions + abertura de PR em branch dedicada.
Future<void> showDeployWorkflowDialog(
  BuildContext context, {
  required DeployApplicationView application,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Workflow GitHub Actions · ${application.name}',
    subtitle: 'Notificação assinada por OIDC (sem segredo compartilhado)',
    maxWidth: 780,
    builder: (_) => _DeployWorkflowBody(application: application),
  );
}

class _DeployWorkflowBody extends ConsumerStatefulWidget {
  const _DeployWorkflowBody({required this.application});
  final DeployApplicationView application;

  @override
  ConsumerState<_DeployWorkflowBody> createState() =>
      _DeployWorkflowBodyState();
}

class _DeployWorkflowBodyState extends ConsumerState<_DeployWorkflowBody> {
  late final TextEditingController _urlCtrl;
  DeployWorkflowView? _workflow;
  DeployGithubStatusView? _status;
  bool _loading = true;
  bool _openingPr = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final defaultUrl = kIsWeb
        ? Uri.base.origin
        : ref.read(serverConfigProvider).baseUrl;
    _urlCtrl = TextEditingController(text: defaultUrl);
    Future<void>.microtask(_load);
  }

  @override
  void dispose() {
    _urlCtrl.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final url = _urlCtrl.text.trim();
      final notifier = ref.read(deploysProvider.notifier);
      final wf = await notifier.workflow(widget.application.id, url);
      DeployGithubStatusView? ghStatus;
      try {
        ghStatus = await notifier.githubStatus(widget.application.id, url);
      } catch (_) {}
      if (!mounted) return;
      setState(() {
        _workflow = wf;
        _status = ghStatus;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _loading = false;
      });
    }
  }

  Future<void> _openPr() async {
    setState(() {
      _openingPr = true;
      _error = null;
    });
    try {
      final res = await ref
          .read(deploysProvider.notifier)
          .openWorkflowPullRequest(widget.application.id, _urlCtrl.text.trim());
      if (!mounted) return;
      final url = res['url'] as String?;
      if (url != null) {
        await launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication);
      }
      await _load();
    } catch (err) {
      if (!mounted) return;
      setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _openingPr = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isPrivate = looksPrivateUrl(_urlCtrl.text);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        LayoutBuilder(
          builder: (context, constraints) {
            final urlField = TextField(
              controller: _urlCtrl,
              decoration: const InputDecoration(
                labelText: 'Endereço público HTTPS da central',
              ),
            );
            final generateBtn = OutlinedButton(
              onPressed: _load,
              child: const Text('Gerar'),
            );
            if (constraints.maxWidth < 420) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  urlField,
                  const SizedBox(height: 8),
                  Align(alignment: Alignment.centerRight, child: generateBtn),
                ],
              );
            }
            return Row(
              children: <Widget>[
                Expanded(child: urlField),
                const SizedBox(width: 8),
                generateBtn,
              ],
            );
          },
        ),
        if (isPrivate) ...<Widget>[
          const SizedBox(height: 8),
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Este endereço parece ser interno/privado — o GitHub Actions precisa de um endereço público HTTPS.',
            ),
          ),
        ],
        if (_status != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(
            tone: _status!.workflow == 'current'
                ? SemanticTone.success
                : SemanticTone.info,
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                Text('Status no repositório: ${_status!.workflow}'),
                if (_status!.canWrite && _status!.workflow != 'current')
                  FilledButton.tonalIcon(
                    onPressed: _openingPr ? null : _openPr,
                    icon: const Icon(Icons.merge_type, size: 16),
                    label: const Text('Abrir PR com workflow'),
                  ),
              ],
            ),
          ),
        ],
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 12),
        if (_loading)
          const LoadingStateView(message: 'Gerando YAML do workflow...')
        else if (_workflow != null)
          CodeViewer(
            title: _workflow!.path,
            code: _workflow!.yaml,
            maxHeight: 300,
          ),
      ],
    );
  }
}

/// Histórico de deploys de um alvo específico (`DeployHistoryDialog`).
Future<void> showDeployHistoryDialog(
  BuildContext context, {
  required DeployTargetView target,
  required bool canAdmin,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Histórico · ${target.name}',
    subtitle: 'Projeto ${target.project}',
    maxWidth: 700,
    builder: (_) => _DeployHistoryBody(targetId: target.id, canAdmin: canAdmin),
  );
}

class _DeployHistoryBody extends ConsumerStatefulWidget {
  const _DeployHistoryBody({required this.targetId, required this.canAdmin});
  final int targetId;
  final bool canAdmin;

  @override
  ConsumerState<_DeployHistoryBody> createState() => _DeployHistoryBodyState();
}

class _DeployHistoryBodyState extends ConsumerState<_DeployHistoryBody> {
  List<DeployRunView> _runs = const <DeployRunView>[];
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    try {
      final list = await ref
          .read(deploysProvider.notifier)
          .targetHistory(widget.targetId);
      if (!mounted) return;
      setState(() {
        _runs = list;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Carregando histórico...');
    }
    if (_error != null) return ErrorStateView(message: _error!, onRetry: _load);
    if (_runs.isEmpty) {
      return const EmptyStateView(title: 'Nenhum deploy neste alvo');
    }

    return Column(
      children: _runs
          .map((run) {
            final visual = deployRunStatusMeta(run.status);
            return Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: ChoiceCard(
                title:
                    '#${run.id} · ${run.shortSha} ${commitFirstLine(run.commitMessage)}',
                subtitle:
                    '${formatDateTime(run.createdAt)}'
                    '${run.durationMs != null ? ' · ${formatElapsedMs(run.durationMs)}' : ''}',
                badge: visual.label,
                badgeTone: visual.tone,
                tone: visual.tone,
                icon: visual.icon,
                onTap: () => showDeployRunDialog(
                  context,
                  runId: run.id,
                  canAdmin: widget.canAdmin,
                ),
              ),
            );
          })
          .toList(growable: false),
    );
  }
}

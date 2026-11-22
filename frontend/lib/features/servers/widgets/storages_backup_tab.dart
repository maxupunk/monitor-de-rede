import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/utils/download_helper.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Destinos" (Storages) plugada no `BackupPage` via `BackupTabRegistry`.
class StoragesBackupTab extends ConsumerStatefulWidget {
  const StoragesBackupTab({super.key});

  @override
  ConsumerState<StoragesBackupTab> createState() => _StoragesBackupTabState();
}

class _StoragesBackupTabState extends ConsumerState<StoragesBackupTab> {
  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() async {
      await Future.wait<void>(<Future<void>>[
        ref.read(storagesProvider.notifier).fetchStorages(),
        ref.read(storagesProvider.notifier).fetchMeta(),
        ref.read(databasesProvider.notifier).fetchConnections(),
      ]);
    });
  }

  Future<void> _openForm({int? editId}) async {
    StorageDestinationDetail? detail;
    if (editId != null) {
      try {
        detail = await ref.read(storagesProvider.notifier).fetchDetail(editId);
      } catch (err) {
        if (!mounted) return;
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('Erro ao abrir destino: $err')));
        return;
      }
    }
    if (!mounted) return;
    await showAdaptiveFormSheet<void>(
      context: context,
      title: detail == null
          ? 'Novo destino de armazenamento'
          : 'Editar ${detail.name}',
      subtitle:
          'Local, SFTP (TOFU SSH pinning), S3, R2, MinIO, GCS ou Azure Blob',
      maxWidth: 700,
      builder: (_) => _StorageFormBody(existing: detail),
    );
  }

  Future<void> _testSaved(StorageDestinationResponse storage) async {
    try {
      final result = await ref
          .read(storagesProvider.notifier)
          .testSaved(storage.id);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            result.ok
                ? 'Conexão OK: ${result.message}'
                : 'Falha: ${result.message}',
          ),
        ),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro ao testar destino: $err')));
    }
  }

  Future<void> _openExplorer(StorageDestinationResponse storage) {
    return showAdaptiveFormSheet<void>(
      context: context,
      title: 'Explorar · ${storage.name}',
      subtitle: storage.target ?? storageProviderInfo(storage.provider).label,
      maxWidth: 740,
      builder: (_) => _StorageExplorerBody(storage: storage),
    );
  }

  Future<void> _remove(
    StorageDestinationResponse storage,
    int linkedDbCount,
  ) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Excluir destino ${storage.name}?',
      message: linkedDbCount > 0
          ? '$linkedDbCount conexão(ões) de banco usam este destino e ficarão sem destino configurado. Os arquivos no servidor remoto não são apagados.'
          : 'Os arquivos já armazenados no destino não são apagados.',
      confirmLabel: 'Excluir destino',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref.read(storagesProvider.notifier).remove(storage.id);
    if (ok && mounted) {
      ref.read(databasesProvider.notifier).fetchConnections();
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Destino ${storage.name} excluído.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(storagesProvider);
    final dbConnections = ref.watch(databasesProvider).connections;

    final isCompact = MediaQuery.sizeOf(context).width < 680;

    return RefreshIndicator(
      onRefresh: () => ref.read(storagesProvider.notifier).fetchStorages(),
      child: ListView(
        padding: const EdgeInsets.only(top: 12, bottom: 24),
        children: <Widget>[
          if (isCompact)
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Destinos de armazenamento',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.titleLarge?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
                FilledButton.icon(
                  onPressed: () => _openForm(),
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Novo'),
                ),
                IconButton(
                  tooltip: 'Atualizar',
                  onPressed: state.loading
                      ? null
                      : () =>
                            ref.read(storagesProvider.notifier).fetchStorages(),
                  icon: const Icon(Icons.refresh, size: 20),
                ),
              ],
            )
          else
            SearchFilterHeader(
              title: 'Destinos de armazenamento',
              subtitle:
                  'Onde as cópias de backup do NetMonitor e dos bancos ficam salvas.',
              actions: <Widget>[
                OutlinedButton.icon(
                  onPressed: state.loading
                      ? null
                      : () =>
                            ref.read(storagesProvider.notifier).fetchStorages(),
                  icon: const Icon(Icons.refresh, size: 18),
                  label: const Text('Atualizar'),
                ),
                FilledButton.icon(
                  onPressed: () => _openForm(),
                  icon: const Icon(Icons.add),
                  label: const Text('Novo destino'),
                ),
              ],
            ),
          const SizedBox(height: 12),

          if (state.error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(state.error!)),
            const SizedBox(height: 12),
          ],

          if (state.loading && !state.loaded)
            const LoadingStateView(message: 'Carregando destinos...')
          else if (state.storages.isEmpty)
            EmptyStateView(
              title: 'Nenhum destino configurado',
              description:
                  'Adicione uma pasta local, servidor SFTP, Amazon S3, Cloudflare R2, MinIO, GCS ou Azure Blob.',
              icon: Icons.cloud_upload_outlined,
              action: FilledButton.icon(
                onPressed: () => _openForm(),
                icon: const Icon(Icons.add),
                label: const Text('Criar primeiro destino'),
              ),
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 1,
              md: 2,
              lg: 2,
              xl: 2,
              children: state.storages
                  .map((storage) {
                    final info = storageProviderInfo(storage.provider);
                    final usedByDbs = dbConnections
                        .where((c) => c.storageDestinationId == storage.id)
                        .toList(growable: false);
                    return SectionCard(
                      title: storage.name,
                      subtitle: info.label,
                      icon: info.icon,
                      iconTone: storage.hasUnreadableCredentials
                          ? SemanticTone.error
                          : info.tone,
                      trailing: SemanticChip(
                        label: countNoun(usedByDbs.length, 'banco'),
                        tone: usedByDbs.isNotEmpty
                            ? SemanticTone.info
                            : SemanticTone.secondary,
                        compact: true,
                      ),
                      footer: SymmetricalActionBar(
                        spacing: 8,
                        runSpacing: 8,
                        children: <Widget>[
                          FilledButton.tonalIcon(
                            onPressed: () => _openExplorer(storage),
                            icon: const Icon(Icons.folder_open, size: 16),
                            label: const Text('Explorar'),
                          ),
                          OutlinedButton.icon(
                            onPressed: () => _testSaved(storage),
                            icon: const Icon(Icons.network_check, size: 16),
                            label: const Text('Testar'),
                          ),
                          OutlinedButton.icon(
                            onPressed: () => _openForm(editId: storage.id),
                            icon: const Icon(Icons.edit_outlined, size: 16),
                            label: const Text('Editar'),
                          ),
                          OutlinedButton.icon(
                            onPressed: () => _remove(storage, usedByDbs.length),
                            icon: Icon(
                              Icons.delete_outline,
                              size: 16,
                              color: context.colorScheme.error,
                            ),
                            label: Text(
                              'Excluir',
                              style: TextStyle(
                                color: context.colorScheme.error,
                              ),
                            ),
                          ),
                        ],
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          if (storage.hasUnreadableCredentials)
                            const NestedPanel(
                              tone: SemanticTone.error,
                              child: Text(
                                'Credencial ilegível — edite e informe de novo.',
                              ),
                            )
                          else
                            KeyValueRow(
                              label: 'Localização',
                              value: storage.target ?? '—',
                              monospace: true,
                            ),
                          if (usedByDbs.isNotEmpty)
                            KeyValueRow(
                              label: 'Usado por',
                              value: usedByDbs.map((c) => c.name).join(', '),
                            ),
                        ],
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
        ],
      ),
    );
  }
}

class _StorageFormBody extends ConsumerStatefulWidget {
  const _StorageFormBody({this.existing});
  final StorageDestinationDetail? existing;

  @override
  ConsumerState<_StorageFormBody> createState() => _StorageFormBodyState();
}

class _StorageFormBodyState extends ConsumerState<_StorageFormBody> {
  late String _provider;
  late final TextEditingController _nameCtrl;
  late final TextEditingController _basePathCtrl;
  late final TextEditingController _hostCtrl;
  late final TextEditingController _portCtrl;
  late final TextEditingController _usernameCtrl;
  late final TextEditingController _passwordCtrl;
  late final TextEditingController _privateKeyCtrl;
  late final TextEditingController _passphraseCtrl;
  late final TextEditingController _endpointCtrl;
  late final TextEditingController _bucketCtrl;
  late final TextEditingController _regionCtrl;
  late final TextEditingController _accessKeyCtrl;
  late final TextEditingController _secretKeyCtrl;
  late final TextEditingController _prefixCtrl;
  late final TextEditingController _gcsJsonCtrl;
  late final TextEditingController _azureConnCtrl;
  late final TextEditingController _azureContainerCtrl;

  String _sftpAuthMode = 'password'; // 'password' | 'key'
  String? _hostKeyFingerprint;
  bool _testing = false;
  bool _testFailed = false;
  String? _testMessage;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    final cfg = e?.config ?? const <String, dynamic>{};
    _provider = e?.provider ?? 'local';
    _hostKeyFingerprint = cfg['hostKeyFingerprint'] as String?;
    if (e != null && e.secretsSet.contains('privateKey')) {
      _sftpAuthMode = 'key';
    }

    _nameCtrl = TextEditingController(text: e?.name ?? '');
    _basePathCtrl = TextEditingController(
      text: (cfg['basePath'] as String?) ?? '',
    );
    _hostCtrl = TextEditingController(text: (cfg['host'] as String?) ?? '');
    _portCtrl = TextEditingController(text: (cfg['port'] ?? 22).toString());
    _usernameCtrl = TextEditingController(
      text: (cfg['username'] as String?) ?? '',
    );
    _passwordCtrl = TextEditingController();
    _privateKeyCtrl = TextEditingController();
    _passphraseCtrl = TextEditingController();
    _endpointCtrl = TextEditingController(
      text: (cfg['endpoint'] as String?) ?? '',
    );
    _bucketCtrl = TextEditingController(text: (cfg['bucket'] as String?) ?? '');
    _regionCtrl = TextEditingController(
      text: (cfg['region'] as String?) ?? 'us-east-1',
    );
    _accessKeyCtrl = TextEditingController(
      text: (cfg['accessKeyId'] as String?) ?? '',
    );
    _secretKeyCtrl = TextEditingController();
    _prefixCtrl = TextEditingController(text: (cfg['prefix'] as String?) ?? '');
    _gcsJsonCtrl = TextEditingController();
    _azureConnCtrl = TextEditingController();
    _azureContainerCtrl = TextEditingController(
      text: (cfg['container'] as String?) ?? '',
    );
  }

  @override
  void dispose() {
    for (final c in <TextEditingController>[
      _nameCtrl,
      _basePathCtrl,
      _hostCtrl,
      _portCtrl,
      _usernameCtrl,
      _passwordCtrl,
      _privateKeyCtrl,
      _passphraseCtrl,
      _endpointCtrl,
      _bucketCtrl,
      _regionCtrl,
      _accessKeyCtrl,
      _secretKeyCtrl,
      _prefixCtrl,
      _gcsJsonCtrl,
      _azureConnCtrl,
      _azureContainerCtrl,
    ]) {
      c.dispose();
    }
    super.dispose();
  }

  Map<String, dynamic> _buildConfig() {
    final configType = storageProviderInfo(_provider).configType;
    switch (configType) {
      case 'local':
        return <String, dynamic>{
          'type': 'local',
          'basePath': _basePathCtrl.text.trim(),
        };
      case 'sftp':
        return <String, dynamic>{
          'type': 'sftp',
          'host': _hostCtrl.text.trim(),
          'port': int.tryParse(_portCtrl.text.trim()) ?? 22,
          'username': _usernameCtrl.text.trim(),
          if (_sftpAuthMode == 'password' && _passwordCtrl.text.isNotEmpty)
            'password': _passwordCtrl.text,
          if (_sftpAuthMode == 'key' && _privateKeyCtrl.text.isNotEmpty)
            'privateKey': _privateKeyCtrl.text,
          if (_sftpAuthMode == 'key' && _passphraseCtrl.text.isNotEmpty)
            'passphrase': _passphraseCtrl.text,
          'basePath': _basePathCtrl.text.trim(),
          if (_hostKeyFingerprint != null && _hostKeyFingerprint!.isNotEmpty)
            'hostKeyFingerprint': _hostKeyFingerprint,
        };
      case 's3':
        return <String, dynamic>{
          'type': 's3',
          if (_provider != 'aws_s3' && _endpointCtrl.text.trim().isNotEmpty)
            'endpoint': _endpointCtrl.text.trim(),
          'bucket': _bucketCtrl.text.trim(),
          if (_provider != 'cloudflare_r2' &&
              _regionCtrl.text.trim().isNotEmpty)
            'region': _regionCtrl.text.trim(),
          'accessKeyId': _accessKeyCtrl.text.trim(),
          if (_secretKeyCtrl.text.isNotEmpty)
            'secretAccessKey': _secretKeyCtrl.text,
          if (_prefixCtrl.text.trim().isNotEmpty)
            'prefix': _prefixCtrl.text.trim(),
        };
      case 'gcs':
        return <String, dynamic>{
          'type': 'gcs',
          'bucket': _bucketCtrl.text.trim(),
          if (_gcsJsonCtrl.text.trim().isNotEmpty)
            'credentialsJson': _gcsJsonCtrl.text.trim(),
          if (_prefixCtrl.text.trim().isNotEmpty)
            'prefix': _prefixCtrl.text.trim(),
        };
      case 'azure_blob':
      default:
        return <String, dynamic>{
          'type': 'azure_blob',
          if (_azureConnCtrl.text.trim().isNotEmpty)
            'connectionString': _azureConnCtrl.text.trim(),
          'container': _azureContainerCtrl.text.trim(),
          if (_prefixCtrl.text.trim().isNotEmpty)
            'prefix': _prefixCtrl.text.trim(),
        };
    }
  }

  Future<void> _test() async {
    setState(() {
      _testing = true;
      _testFailed = false;
      _error = null;
      _testMessage = null;
    });
    try {
      final resp = await ref
          .read(storagesProvider.notifier)
          .testDraft(
            id: widget.existing?.id,
            provider: _provider,
            config: _buildConfig(),
          );
      if (!mounted) return;
      setState(() {
        _testing = false;
        _testFailed = !resp.ok;
        _testMessage = resp.message;
        if (resp.hostKeyFingerprint != null &&
            resp.hostKeyFingerprint!.isNotEmpty) {
          _hostKeyFingerprint = resp.hostKeyFingerprint;
        }
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _testing = false;
        _testFailed = true;
        _error = err.toString();
      });
    }
  }

  Future<void> _save() async {
    if (_nameCtrl.text.trim().isEmpty) {
      setState(() => _error = 'Informe um nome para o destino.');
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref
          .read(storagesProvider.notifier)
          .save(
            widget.existing?.id,
            StorageDestinationInput(
              name: _nameCtrl.text.trim(),
              provider: _provider,
              config: _buildConfig(),
            ),
          );
      if (mounted) Navigator.of(context).pop();
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _error = err.toString();
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final localRoot = ref.watch(storagesProvider).meta?.localRoot ?? 'storage/';
    final secretsSet = widget.existing?.secretsSet ?? const <String>[];
    final configType = storageProviderInfo(_provider).configType;

    String? secretHint(String field) => secretsSet.contains(field)
        ? 'definida — deixe em branco para manter'
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (widget.existing == null) ...<Widget>[
          Text(
            'Provedor de armazenamento',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: storageProvidersCatalog
                .map((info) {
                  return ChoiceChip(
                    selected: _provider == info.value,
                    avatar: Icon(info.icon, size: 16),
                    label: Text(info.label),
                    onSelected: (_) => setState(() => _provider = info.value),
                  );
                })
                .toList(growable: false),
          ),
          const SizedBox(height: 14),
        ],

        TextField(
          controller: _nameCtrl,
          decoration: const InputDecoration(labelText: 'Nome do destino'),
        ),
        const SizedBox(height: 12),

        if (configType == 'local') ...<Widget>[
          TextField(
            controller: _basePathCtrl,
            decoration: InputDecoration(
              labelText: 'Subpasta dentro de $localRoot',
              prefixText: localRoot,
            ),
          ),
        ] else if (configType == 'sftp') ...<Widget>[
          ResponsiveFieldRow(
            breakpoint: 480,
            spacing: 10,
            runSpacing: 10,
            children: <Widget>[
              TextField(
                controller: _hostCtrl,
                decoration: const InputDecoration(labelText: 'Host SFTP / SSH'),
              ),
              TextField(
                controller: _portCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Porta'),
              ),
            ],
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _usernameCtrl,
            decoration: const InputDecoration(labelText: 'Usuário SSH'),
          ),
          const SizedBox(height: 10),
          SegmentedButton<String>(
            showSelectedIcon: false,
            style: SegmentedButton.styleFrom(
              visualDensity: VisualDensity.compact,
              padding: const EdgeInsets.symmetric(horizontal: 10),
            ),
            segments: const <ButtonSegment<String>>[
              ButtonSegment(
                value: 'password',
                label: Text('Senha', maxLines: 1, softWrap: false),
              ),
              ButtonSegment(
                value: 'key',
                label: Text('Chave SSH', maxLines: 1, softWrap: false),
              ),
            ],
            selected: <String>{_sftpAuthMode},
            onSelectionChanged: (set) =>
                setState(() => _sftpAuthMode = set.first),
          ),
          const SizedBox(height: 10),
          if (_sftpAuthMode == 'password')
            TextField(
              controller: _passwordCtrl,
              obscureText: true,
              decoration: InputDecoration(
                labelText: 'Senha SSH',
                hintText: secretHint('password'),
              ),
            )
          else ...<Widget>[
            TextField(
              controller: _privateKeyCtrl,
              maxLines: 4,
              decoration: InputDecoration(
                labelText: 'Chave privada PEM',
                hintText: secretHint('privateKey'),
              ),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _passphraseCtrl,
              obscureText: true,
              decoration: InputDecoration(
                labelText: 'Passphrase (opcional)',
                hintText: secretHint('passphrase'),
              ),
            ),
          ],
          const SizedBox(height: 10),
          TextField(
            controller: _basePathCtrl,
            decoration: const InputDecoration(labelText: 'Pasta remota'),
          ),
          if (_hostKeyFingerprint != null &&
              _hostKeyFingerprint!.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            NestedPanel(
              tone: SemanticTone.info,
              child: Wrap(
                spacing: 8,
                runSpacing: 6,
                alignment: WrapAlignment.spaceBetween,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  Text(
                    'Host key SSH fixada (TOFU): $_hostKeyFingerprint',
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 12,
                    ),
                  ),
                  TextButton(
                    onPressed: () => setState(() => _hostKeyFingerprint = null),
                    child: const Text('Aceitar nova'),
                  ),
                ],
              ),
            ),
          ],
        ] else if (configType == 's3') ...<Widget>[
          if (_provider != 'aws_s3') ...<Widget>[
            TextField(
              controller: _endpointCtrl,
              decoration: const InputDecoration(
                labelText: 'Endpoint URL',
                hintText: 'https://s3.exemplo.com',
              ),
            ),
            const SizedBox(height: 10),
          ],
          TextField(
            controller: _bucketCtrl,
            decoration: const InputDecoration(labelText: 'Bucket'),
          ),
          const SizedBox(height: 10),
          if (_provider != 'cloudflare_r2') ...<Widget>[
            TextField(
              controller: _regionCtrl,
              decoration: const InputDecoration(labelText: 'Região'),
            ),
            const SizedBox(height: 10),
          ],
          TextField(
            controller: _accessKeyCtrl,
            decoration: const InputDecoration(labelText: 'Access Key ID'),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _secretKeyCtrl,
            obscureText: true,
            decoration: InputDecoration(
              labelText: 'Secret Access Key',
              hintText: secretHint('secretAccessKey'),
            ),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _prefixCtrl,
            decoration: const InputDecoration(labelText: 'Prefixo (opcional)'),
          ),
        ] else if (configType == 'gcs') ...<Widget>[
          TextField(
            controller: _bucketCtrl,
            decoration: const InputDecoration(labelText: 'Bucket GCS'),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _gcsJsonCtrl,
            maxLines: 5,
            decoration: InputDecoration(
              labelText: 'JSON da conta de serviço',
              hintText: secretHint('credentialsJson'),
            ),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _prefixCtrl,
            decoration: const InputDecoration(labelText: 'Prefixo (opcional)'),
          ),
        ] else ...<Widget>[
          TextField(
            controller: _azureContainerCtrl,
            decoration: const InputDecoration(labelText: 'Container Azure'),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _azureConnCtrl,
            obscureText: true,
            decoration: InputDecoration(
              labelText: 'Connection String',
              hintText: secretHint('connectionString'),
            ),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _prefixCtrl,
            decoration: const InputDecoration(labelText: 'Prefixo (opcional)'),
          ),
        ],

        if (_testMessage != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: _testFailed ? SemanticTone.warning : SemanticTone.success,
            child: Text(_testMessage!),
          ),
        ],
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          alignment: WrapAlignment.end,
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: _testing ? null : _test,
              icon: _testing
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.network_check),
              label: const Text('Testar conexão'),
            ),
            FilledButton.icon(
              onPressed: _saving ? null : _save,
              icon: Icon(
                _testFailed ? Icons.warning_amber_rounded : Icons.save,
              ),
              label: Text(
                _testFailed ? 'Salvar mesmo assim' : 'Salvar destino',
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _StorageExplorerBody extends ConsumerStatefulWidget {
  const _StorageExplorerBody({required this.storage});
  final StorageDestinationResponse storage;

  @override
  ConsumerState<_StorageExplorerBody> createState() =>
      _StorageExplorerBodyState();
}

class _StorageExplorerBodyState extends ConsumerState<_StorageExplorerBody> {
  String _path = '';
  List<StorageObject> _objects = const <StorageObject>[];
  String? _nextCursor;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() => _browse(''));
  }

  Future<void> _browse(
    String path, {
    String? cursor,
    bool append = false,
  }) async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final resp = await ref
          .read(storagesProvider.notifier)
          .browse(widget.storage.id, path, cursor: cursor);
      if (!mounted) return;
      setState(() {
        _path = resp.path;
        _objects = append
            ? <StorageObject>[..._objects, ...resp.objects]
            : resp.objects;
        _nextCursor = resp.nextCursor;
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

  Future<void> _download(StorageObject obj) async {
    try {
      final res = await ref
          .read(storagesProvider.notifier)
          .downloadObject(widget.storage.id, obj.key);
      if (!mounted) return;
      await shareDownloadedBytes(
        context,
        bytes: res.bytes,
        filename: res.filename ?? obj.name,
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro ao baixar arquivo: $err')));
    }
  }

  Future<void> _delete(StorageObject obj) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Apagar ${obj.isDirectory ? 'pasta' : 'arquivo'} ${obj.name}?',
      message: obj.isDirectory
          ? 'Todos os arquivos dentro desta pasta serão removidos recursivamente.'
          : 'Este arquivo será apagado permanentemente do destino.',
      confirmLabel: 'Apagar',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    await ref
        .read(storagesProvider.notifier)
        .deleteObject(widget.storage.id, obj.key, directory: obj.isDirectory);
    if (mounted) await _browse(_path);
  }

  @override
  Widget build(BuildContext context) {
    final segments = _path
        .split('/')
        .where((s) => s.isNotEmpty)
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 4,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            ActionChip(label: const Text('Raiz'), onPressed: () => _browse('')),
            for (var i = 0; i < segments.length; i++) ...<Widget>[
              const Icon(Icons.chevron_right, size: 16),
              ActionChip(
                label: Text(segments[i]),
                onPressed: () => _browse(segments.take(i + 1).join('/')),
              ),
            ],
          ],
        ),
        const SizedBox(height: 12),
        if (_loading && _objects.isEmpty)
          const LoadingStateView(message: 'Listando objetos...')
        else if (_error != null)
          ErrorStateView(message: _error!, onRetry: () => _browse(_path))
        else if (_objects.isEmpty)
          const EmptyStateView(title: 'Pasta vazia', icon: Icons.folder_open)
        else ...<Widget>[
          ..._objects.map(
            (obj) => ListTile(
              contentPadding: EdgeInsets.zero,
              leading: Icon(
                obj.isDirectory
                    ? Icons.folder_outlined
                    : Icons.insert_drive_file_outlined,
              ),
              title: Text(
                obj.name,
                style: const TextStyle(fontWeight: FontWeight.w600),
              ),
              subtitle: Text(
                obj.isDirectory
                    ? 'Pasta'
                    : '${formatBytes(obj.size)}${obj.lastModified != null ? ' · ${formatDateTime(obj.lastModified)}' : ''}',
              ),
              onTap: obj.isDirectory ? () => _browse(obj.key) : null,
              trailing: Wrap(
                spacing: 4,
                children: <Widget>[
                  if (!obj.isDirectory)
                    IconButton(
                      tooltip: 'Baixar',
                      icon: const Icon(Icons.download_outlined),
                      onPressed: () => _download(obj),
                    ),
                  IconButton(
                    tooltip: 'Apagar',
                    icon: const Icon(Icons.delete_outline),
                    onPressed: () => _delete(obj),
                  ),
                ],
              ),
            ),
          ),
          if (_nextCursor != null)
            Padding(
              padding: const EdgeInsets.only(top: 10),
              child: OutlinedButton(
                onPressed: _loading
                    ? null
                    : () => _browse(_path, cursor: _nextCursor, append: true),
                child: const Text('Carregar mais'),
              ),
            ),
        ],
      ],
    );
  }
}

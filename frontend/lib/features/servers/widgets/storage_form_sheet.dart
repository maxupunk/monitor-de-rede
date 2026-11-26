import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o formulário modal de criação/edição de destino de armazenamento.
Future<void> showStorageFormSheet(
  BuildContext context, {
  StorageDestinationDetail? existing,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: existing == null
        ? 'Novo destino de armazenamento'
        : 'Editar ${existing.name}',
    subtitle:
        'Local, SFTP (TOFU SSH pinning), S3, R2, MinIO, GCS ou Azure Blob',
    maxWidth: 700,
    builder: (_) => StorageFormBody(existing: existing),
  );
}

class StorageFormBody extends ConsumerStatefulWidget {
  const StorageFormBody({super.key, this.existing});
  final StorageDestinationDetail? existing;

  @override
  ConsumerState<StorageFormBody> createState() => _StorageFormBodyState();
}

class _StorageFormBodyState extends ConsumerState<StorageFormBody> {
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

  String? _localPathValidationError() {
    final configType = storageProviderInfo(_provider).configType;
    if (configType != 'local') return null;
    final rawRoot = ref.read(storagesProvider).meta?.localRoot ?? 'storage';
    final root = formatLocalStorageRoot(rawRoot);
    return validateLocalBasePath(_basePathCtrl.text, rootLabel: root.label);
  }

  Future<void> _test() async {
    final localError = _localPathValidationError();
    if (localError != null) {
      setState(() {
        _error = null;
        _testMessage = null;
      });
      return;
    }
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
        _error = describeError(err, fallback: 'Falha ao testar destino.');
      });
    }
  }

  Future<void> _save() async {
    if (_nameCtrl.text.trim().isEmpty) {
      setState(() => _error = 'Informe um nome para o destino.');
      return;
    }
    final localError = _localPathValidationError();
    if (localError != null) {
      setState(() => _error = null);
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
        _error = describeError(err, fallback: 'Erro ao salvar destino.');
      });
    }
  }

  String? _secretHint(String field) {
    final secretsSet = widget.existing?.secretsSet ?? const <String>[];
    return secretsSet.contains(field)
        ? 'definida — deixe em branco para manter'
        : null;
  }

  List<Widget> _buildProviderFields(String configType, String localRoot) {
    final root = formatLocalStorageRoot(localRoot);
    final localPathError = configType == 'local'
        ? validateLocalBasePath(_basePathCtrl.text, rootLabel: root.label)
        : null;

    return switch (configType) {
      'local' => <Widget>[
        TextField(
          controller: _basePathCtrl,
          onChanged: (_) => setState(() => _error = null),
          decoration: InputDecoration(
            labelText: 'Subpasta dentro de ${root.label}',
            floatingLabelBehavior: FloatingLabelBehavior.always,
            prefixText: root.prefix,
            hintText: 'ex.: databases-local',
            helperText:
                'Opcional — deixe em branco para salvar direto em ${root.label}.',
            errorText: localPathError,
          ),
        ),
      ],
      'sftp' => _buildSftpFields(),
      's3' => _buildS3Fields(),
      'gcs' => <Widget>[
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
            hintText: _secretHint('credentialsJson'),
          ),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _prefixCtrl,
          decoration: const InputDecoration(labelText: 'Prefixo (opcional)'),
        ),
      ],
      _ => <Widget>[
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
            hintText: _secretHint('connectionString'),
          ),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _prefixCtrl,
          decoration: const InputDecoration(labelText: 'Prefixo (opcional)'),
        ),
      ],
    };
  }

  List<Widget> _buildSftpFields() {
    return <Widget>[
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
          ButtonSegment<String>(
            value: 'password',
            label: Text('Senha', maxLines: 1, softWrap: false),
          ),
          ButtonSegment<String>(
            value: 'key',
            label: Text('Chave SSH', maxLines: 1, softWrap: false),
          ),
        ],
        selected: <String>{_sftpAuthMode},
        onSelectionChanged: (set) => setState(() => _sftpAuthMode = set.first),
      ),
      const SizedBox(height: 10),
      if (_sftpAuthMode == 'password')
        TextField(
          controller: _passwordCtrl,
          obscureText: true,
          decoration: InputDecoration(
            labelText: 'Senha SSH',
            hintText: _secretHint('password'),
          ),
        )
      else ...<Widget>[
        TextField(
          controller: _privateKeyCtrl,
          maxLines: 4,
          decoration: InputDecoration(
            labelText: 'Chave privada PEM',
            hintText: _secretHint('privateKey'),
          ),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _passphraseCtrl,
          obscureText: true,
          decoration: InputDecoration(
            labelText: 'Passphrase (opcional)',
            hintText: _secretHint('passphrase'),
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
                style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
              ),
              TextButton(
                onPressed: () => setState(() => _hostKeyFingerprint = null),
                child: const Text('Aceitar nova'),
              ),
            ],
          ),
        ),
      ],
    ];
  }

  List<Widget> _buildS3Fields() {
    return <Widget>[
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
          hintText: _secretHint('secretAccessKey'),
        ),
      ),
      const SizedBox(height: 10),
      TextField(
        controller: _prefixCtrl,
        decoration: const InputDecoration(labelText: 'Prefixo (opcional)'),
      ),
    ];
  }

  @override
  Widget build(BuildContext context) {
    final localRoot = ref.watch(storagesProvider).meta?.localRoot ?? 'storage/';
    final configType = storageProviderInfo(_provider).configType;

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

        ..._buildProviderFields(configType, localRoot),

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

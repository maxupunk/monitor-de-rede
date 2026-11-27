import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o formulário modal de criação/edição de conexão de banco de dados (ADR 013).
Future<void> showDatabaseConnectionFormSheet(
  BuildContext context, {
  DatabaseConnectionResponse? existing,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: existing == null ? 'Novo banco de dados' : 'Editar ${existing.name}',
    subtitle:
        'Backup consistente via central ou ponte segura do agente (ADR 013)',
    maxWidth: 740,
    builder: (_) => DatabaseConnectionFormBody(existing: existing),
  );
}

class DatabaseConnectionFormBody extends ConsumerStatefulWidget {
  const DatabaseConnectionFormBody({super.key, this.existing});
  final DatabaseConnectionResponse? existing;

  @override
  ConsumerState<DatabaseConnectionFormBody> createState() =>
      _DatabaseConnectionFormBodyState();
}

class _DatabaseConnectionFormBodyState
    extends ConsumerState<DatabaseConnectionFormBody> {
  late final TextEditingController _nameCtrl;
  late final TextEditingController _hostCtrl;
  late final TextEditingController _portCtrl;
  late final TextEditingController _userCtrl;
  late final TextEditingController _passCtrl;
  late final TextEditingController _intervalCtrl;
  late final TextEditingController _retentionCtrl;

  String _engine = 'postgres';
  String _sslMode = 'prefer';
  int? _viaProbeId;

  /// O contêiner que o endereço acompanha; nulo é endereço fixo.
  DatabaseContainerLink? _container;
  int? _storageDestinationId;
  bool _storageTouched = false;
  bool _backupEnabled = true;
  bool _backupEnabledTouched = false;
  bool _allDatabases = true;
  List<String> _discoveredDatabases = const <String>[];
  Set<String> _selectedDatabases = <String>{};

  bool _probing = false;
  bool _probeFailed = false;
  String? _probeInfo;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    final initialStorages = ref.read(storagesProvider).storages;
    _engine = e?.engine ?? 'postgres';
    _sslMode = e?.sslMode ?? 'prefer';
    _viaProbeId = e?.viaProbeId;
    _container = e?.container;
    _storageDestinationId = e != null
        ? e.storageDestinationId
        : (initialStorages.isNotEmpty ? initialStorages.first.id : null);
    _backupEnabled = e?.backupEnabled ?? true;
    _allDatabases = e == null || e.databases.isEmpty;
    _selectedDatabases = Set<String>.from(e?.databases ?? const <String>[]);
    _discoveredDatabases = List<String>.from(e?.databases ?? const <String>[]);

    _nameCtrl = TextEditingController(text: e?.name ?? '');
    _hostCtrl = TextEditingController(text: e?.host ?? '');
    _portCtrl = TextEditingController(
      text: (e?.port ?? (_engine == 'postgres' ? 5432 : 3306)).toString(),
    );
    _userCtrl = TextEditingController(text: e?.username ?? '');
    _passCtrl = TextEditingController();
    _intervalCtrl = TextEditingController(
      text: (e?.backupIntervalHours ?? 24).toString(),
    );
    _retentionCtrl = TextEditingController(
      text: (e?.backupRetention ?? 7).toString(),
    );

    Future<void>.microtask(() async {
      await Future.wait<void>(<Future<void>>[
        ref.read(databasesProvider.notifier).fetchAgents(),
        if (widget.existing == null)
          ref.read(databasesProvider.notifier).fetchContainers(),
        ref.read(storagesProvider.notifier).fetchStorages(),
      ]);
      if (!mounted) return;
      if (widget.existing == null &&
          !_storageTouched &&
          _storageDestinationId == null) {
        final loadedStorages = ref.read(storagesProvider).storages;
        if (loadedStorages.isNotEmpty) {
          setState(() => _storageDestinationId = loadedStorages.first.id);
        }
      }
    });
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _hostCtrl.dispose();
    _portCtrl.dispose();
    _userCtrl.dispose();
    _passCtrl.dispose();
    _intervalCtrl.dispose();
    _retentionCtrl.dispose();
    super.dispose();
  }

  Future<void> _pickContainer(DatabaseContainer container) async {
    setState(() {
      _probing = true;
      _error = null;
    });
    try {
      final res = await ref
          .read(databasesProvider.notifier)
          .resolveContainer(container.hostKey, container.containerId);
      if (!mounted) return;
      final chosen =
          res.chosen ?? (res.addresses.isNotEmpty ? res.addresses.first : null);
      setState(() {
        _engine = container.engine;
        _container = DatabaseContainerLink.fromContainer(res.container);
        _nameCtrl.text = container.containerName;
        if (chosen != null) {
          _hostCtrl.text = chosen.host;
          _portCtrl.text = chosen.port.toString();
          _viaProbeId = chosen.viaProbeId ?? container.probeId;
        }
        if (res.username != null && res.username!.isNotEmpty) {
          _userCtrl.text = res.username!;
        }
        if (res.database != null && res.database!.isNotEmpty) {
          _allDatabases = false;
          _selectedDatabases = <String>{res.database!};
          _discoveredDatabases = <String>[res.database!];
        }
        _probeInfo = chosen != null
            ? 'Endereço resolvido (${chosen.kind}): ${chosen.host}:${chosen.port}. Informe a senha para testar.'
            : 'Contêiner selecionado. Confira host/porta e informe a senha.';
        _probing = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao resolver container.');
        _probing = false;
      });
    }
  }

  Future<bool> _runProbe() async {
    setState(() {
      _probing = true;
      _probeFailed = false;
      _error = null;
    });
    try {
      final resp = await ref
          .read(databasesProvider.notifier)
          .probe(
            id: widget.existing?.id,
            engine: _engine,
            host: _hostCtrl.text.trim(),
            port:
                int.tryParse(_portCtrl.text.trim()) ??
                (_engine == 'postgres' ? 5432 : 3306),
            username: _userCtrl.text.trim(),
            password: _passCtrl.text,
            sslMode: _sslMode,
            viaProbeId: _viaProbeId,
          );
      if (!mounted) return false;
      setState(() {
        _discoveredDatabases = resp.databases;
        _probeInfo =
            'Conectado (${formatLatency(resp.latencyMs)}) · ${resp.version} · ${countNoun(resp.databases.length, 'banco')}';
        _probing = false;
      });
      return true;
    } catch (err) {
      if (!mounted) return false;
      setState(() {
        _probeFailed = true;
        _error = describeError(err, fallback: 'Falha ao testar conexão.');
        _probing = false;
      });
      return false;
    }
  }

  Future<void> _save() async {
    if (_nameCtrl.text.trim().isEmpty || _hostCtrl.text.trim().isEmpty) {
      setState(() => _error = 'Preencha nome e host do banco.');
      return;
    }
    final storages = ref.read(storagesProvider).storages;
    final resolvedStorageId = storages.any((s) => s.id == _storageDestinationId)
        ? _storageDestinationId
        : null;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref
          .read(databasesProvider.notifier)
          .save(
            widget.existing?.id,
            DatabaseConnectionInput(
              name: _nameCtrl.text.trim(),
              engine: _engine,
              host: _hostCtrl.text.trim(),
              port:
                  int.tryParse(_portCtrl.text.trim()) ??
                  (_engine == 'postgres' ? 5432 : 3306),
              username: _userCtrl.text.trim(),
              password: _passCtrl.text.isEmpty ? null : _passCtrl.text,
              sslMode: _sslMode,
              databases: _allDatabases
                  ? const <String>[]
                  : _selectedDatabases.toList(),
              storageDestinationId: resolvedStorageId,
              viaProbeId: _viaProbeId,
              backupEnabled: resolvedStorageId != null && _backupEnabled,
              backupIntervalHours:
                  int.tryParse(_intervalCtrl.text.trim()) ?? 24,
              backupRetention: int.tryParse(_retentionCtrl.text.trim()) ?? 7,
              container: _container,
            ),
          );
      if (mounted) Navigator.of(context).pop();
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _error = describeError(err, fallback: 'Erro ao salvar conexão.');
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final dbState = ref.watch(databasesProvider);
    final storages = ref.watch(storagesProvider).storages;
    final discoveredContainers = dbState.containers.containers;
    final resolvedStorageId = storages.any((s) => s.id == _storageDestinationId)
        ? _storageDestinationId
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (widget.existing == null &&
            discoveredContainers.isNotEmpty) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 4,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        const Icon(Icons.auto_fix_high, size: 18),
                        const SizedBox(width: 8),
                        Flexible(
                          child: Text(
                            'Bancos detectados (${discoveredContainers.length})',
                            style: context.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                      ],
                    ),
                    TextButton(
                      onPressed: dbState.containersLoading
                          ? null
                          : () => ref
                                .read(databasesProvider.notifier)
                                .fetchContainers(),
                      child: const Text('Procurar de novo'),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: discoveredContainers
                      .take(10)
                      .map((c) {
                        return ActionChip(
                          avatar: const Icon(Icons.storage_outlined, size: 16),
                          label: Text('${c.containerName} (${c.hostName})'),
                          onPressed: () => _pickContainer(c),
                        );
                      })
                      .toList(growable: false),
                ),
              ],
            ),
          ),
          const SizedBox(height: 14),
        ],

        SegmentedButton<String>(
          showSelectedIcon: false,
          style: SegmentedButton.styleFrom(
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 8),
          ),
          segments: const <ButtonSegment<String>>[
            ButtonSegment<String>(
              value: 'postgres',
              label: Text('PostgreSQL', maxLines: 1, softWrap: false),
            ),
            ButtonSegment<String>(
              value: 'mysql',
              label: Text('MySQL', maxLines: 1, softWrap: false),
            ),
            ButtonSegment<String>(
              value: 'mariadb',
              label: Text('MariaDB', maxLines: 1, softWrap: false),
            ),
          ],
          selected: <String>{_engine},
          onSelectionChanged: (set) {
            final next = set.first;
            setState(() {
              _engine = next;
              if (_portCtrl.text == '5432' || _portCtrl.text == '3306') {
                _portCtrl.text = next == 'postgres' ? '5432' : '3306';
              }
            });
          },
        ),
        const SizedBox(height: 12),

        TextField(
          controller: _nameCtrl,
          decoration: const InputDecoration(labelText: 'Nome da conexão'),
        ),
        const SizedBox(height: 12),

        AppDropdownFormField<int?>(
          initialValue: dbState.agents.any((a) => a.id == _viaProbeId)
              ? _viaProbeId
              : null,
          decoration: const InputDecoration(
            labelText: 'Rota de acesso (ADR 013)',
            helperText:
                'Agentes precisam da permissão "database" em AGENT_ALLOW.',
          ),
          items: <DropdownMenuItem<int?>>[
            const DropdownMenuItem<int?>(
              value: null,
              child: Text(
                'Direto desta central',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            ...dbState.agents.map(
              (a) => DropdownMenuItem<int?>(
                value: a.id,
                enabled: a.allowed,
                child: Text(
                  'Agente ${a.name}${!a.allowed ? ' (sem permissão database)' : (!a.connected ? ' (offline)' : '')}',
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
          ],
          onChanged: (v) => setState(() => _viaProbeId = v),
        ),
        if (_viaProbeId != null) ...<Widget>[
          const SizedBox(height: 10),
          CodeViewer(
            title: 'Permissão obrigatória no agente remoto (ADR 013)',
            code:
                'AGENT_ALLOW=read,database\n'
                'AGENT_DATABASE_TARGETS=${_hostCtrl.text.trim().isEmpty ? '172.16.0.0/12' : _hostCtrl.text.trim()}:${_portCtrl.text.trim()}',
          ),
        ],
        const SizedBox(height: 12),

        if (_container != null) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Row(
              children: <Widget>[
                const Icon(Icons.link, size: 18),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'O endereço acompanha o contêiner ${_container!.name}: '
                    'se ele reiniciar ou for recriado com outro IP, o backup o reencontra sozinho.',
                  ),
                ),
                const SizedBox(width: 8),
                TextButton(
                  onPressed: () => setState(() => _container = null),
                  child: const Text('Fixar endereço'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 10,
          runSpacing: 12,
          children: <Widget>[
            TextField(
              controller: _hostCtrl,
              decoration: InputDecoration(
                labelText: 'Host / IP',
                helperText: _container == null
                    ? null
                    : 'Atualizado sozinho se o contêiner mudar de IP',
                helperMaxLines: 2,
              ),
            ),
            TextField(
              controller: _portCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Porta'),
            ),
          ],
        ),
        const SizedBox(height: 12),

        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 10,
          runSpacing: 12,
          children: <Widget>[
            TextField(
              controller: _userCtrl,
              decoration: const InputDecoration(labelText: 'Usuário'),
            ),
            TextField(
              controller: _passCtrl,
              obscureText: true,
              decoration: InputDecoration(
                labelText: 'Senha',
                hintText: widget.existing?.passwordSet == true
                    ? 'definida — deixe em branco para manter'
                    : null,
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),

        AppDropdownFormField<String>(
          initialValue: _sslMode,
          decoration: const InputDecoration(labelText: 'Modo SSL'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem<String>(
              value: 'prefer',
              child: Text(
                'Preferir SSL (prefer)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem<String>(
              value: 'require',
              child: Text(
                'Exigir SSL (require)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem<String>(
              value: 'disable',
              child: Text(
                'Desabilitado (disable)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
          onChanged: (v) {
            if (v != null) setState(() => _sslMode = v);
          },
        ),
        const SizedBox(height: 12),

        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: _probing ? null : _runProbe,
              icon: _probing
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.network_check),
              label: const Text('Testar e listar bancos'),
            ),
          ],
        ),
        if (_probeInfo != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.success, child: Text(_probeInfo!)),
        ],
        const SizedBox(height: 12),

        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _allDatabases,
          title: const Text('Fazer backup de todos os bancos'),
          onChanged: (v) => setState(() => _allDatabases = v),
        ),
        if (!_allDatabases && _discoveredDatabases.isNotEmpty)
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: _discoveredDatabases
                .map((db) {
                  final selected = _selectedDatabases.contains(db);
                  return FilterChip(
                    selected: selected,
                    label: Text(db),
                    onSelected: (checked) => setState(() {
                      if (checked) {
                        _selectedDatabases.add(db);
                      } else {
                        _selectedDatabases.remove(db);
                      }
                    }),
                  );
                })
                .toList(growable: false),
          ),

        const Divider(height: 24),
        AppDropdownFormField<int?>(
          key: ValueKey<int?>(resolvedStorageId),
          initialValue: resolvedStorageId,
          decoration: InputDecoration(
            labelText: 'Destino de armazenamento',
            helperText: storages.isEmpty
                ? 'Nenhum destino cadastrado — crie um na aba Destinos para habilitar backups.'
                : null,
          ),
          items: <DropdownMenuItem<int?>>[
            const DropdownMenuItem<int?>(
              value: null,
              child: Text('Nenhum destino', overflow: TextOverflow.ellipsis),
            ),
            ...storages.map(
              (s) => DropdownMenuItem<int?>(
                value: s.id,
                child: Text(
                  '${s.name} (${s.provider})',
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
          ],
          onChanged: (v) => setState(() {
            final wasNull = _storageDestinationId == null;
            _storageTouched = true;
            _storageDestinationId = v;
            if (v != null && wasNull && !_backupEnabledTouched) {
              _backupEnabled = true;
            }
          }),
        ),
        const SizedBox(height: 10),

        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: resolvedStorageId != null && _backupEnabled,
          title: const Text('Backup automático agendado'),
          subtitle: Text(
            resolvedStorageId == null
                ? (storages.isEmpty
                      ? 'Cadastre um destino na aba Destinos para ativar o agendamento.'
                      : 'Selecione um destino de armazenamento acima para ativar o agendamento.')
                : 'Snapshot consistente sem travar escrita no banco.',
          ),
          onChanged: resolvedStorageId == null
              ? null
              : (v) => setState(() {
                  _backupEnabledTouched = true;
                  _backupEnabled = v;
                }),
        ),
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 10,
          runSpacing: 12,
          children: <Widget>[
            TextField(
              controller: _intervalCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Intervalo (horas)'),
            ),
            TextField(
              controller: _retentionCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Retenção (cópias)'),
            ),
          ],
        ),

        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _saving ? null : _save,
            icon: Icon(_probeFailed ? Icons.warning_amber_rounded : Icons.save),
            label: Text(_probeFailed ? 'Salvar mesmo assim' : 'Salvar conexão'),
          ),
        ),
      ],
    );
  }
}

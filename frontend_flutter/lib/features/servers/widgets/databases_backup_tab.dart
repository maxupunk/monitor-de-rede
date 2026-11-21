import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/utils/download_helper.dart';
import 'package:netmonitor_app/features/servers/widgets/database_restore_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Bancos de dados" plugada no `BackupPage` via `BackupTabRegistry`.
class DatabasesBackupTab extends ConsumerStatefulWidget {
  const DatabasesBackupTab({super.key});

  @override
  ConsumerState<DatabasesBackupTab> createState() => _DatabasesBackupTabState();
}

class _DatabasesBackupTabState extends ConsumerState<DatabasesBackupTab> {
  final TextEditingController _searchController = TextEditingController();
  String _search = '';
  String _originFilter = 'all';
  String _healthFilter = 'all'; // 'all' | 'failed' | 'no_storage'
  bool _groupByOrigin = true;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() async {
      await Future.wait<void>(<Future<void>>[
        ref.read(databasesProvider.notifier).fetchConnections(),
        ref.read(storagesProvider.notifier).fetchStorages(),
      ]);
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _openForm({DatabaseConnectionResponse? existing}) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing == null
          ? 'Novo banco de dados'
          : 'Editar ${existing.name}',
      subtitle:
          'Backup consistente via central ou ponte segura do agente (ADR 013)',
      maxWidth: 740,
      builder: (_) => _DatabaseFormBody(existing: existing),
    );
  }

  Future<void> _openHistory(DatabaseConnectionResponse conn) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Histórico de cópias · ${conn.name}',
      subtitle: '${conn.engineLabel} · ${conn.host}:${conn.port}',
      maxWidth: 740,
      builder: (_) => _DatabaseHistoryBody(connection: conn),
    );
  }

  Future<void> _runBackupNow(DatabaseConnectionResponse conn) async {
    if (conn.storageDestinationId == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Configure um destino de armazenamento antes de fazer backup.',
          ),
        ),
      );
      return;
    }
    try {
      await ref.read(databasesProvider.notifier).runBackup(conn.id);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Backup de ${conn.name} iniciado.')),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Falha ao iniciar backup: $err')));
    }
  }

  Future<void> _testConnection(DatabaseConnectionResponse conn) async {
    try {
      final result = await ref
          .read(databasesProvider.notifier)
          .probe(
            id: conn.id,
            engine: conn.engine,
            host: conn.host,
            port: conn.port,
            username: conn.username,
            sslMode: conn.sslMode,
            viaProbeId: conn.viaProbeId,
          );
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Conexão OK (${formatLatency(result.latencyMs)}) · ${countNoun(result.databases.length, 'banco')} encontrado(s).',
          ),
        ),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Falha no teste de conexão: $err')),
      );
    }
  }

  Future<void> _deleteConnection(DatabaseConnectionResponse conn) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Excluir conexão ${conn.name}?',
      message: 'Os arquivos de backup já gravados no destino de armazenamento serão mantidos.',
      confirmLabel: 'Excluir conexão',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref.read(databasesProvider.notifier).remove(conn.id);
    if (ok && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Conexão ${conn.name} excluída.')));
    }
  }

  String _originKey(DatabaseConnectionResponse conn) =>
      conn.viaProbeId == null ? 'central' : 'agent-${conn.viaProbeId}';

  String _originLabel(DatabaseConnectionResponse conn) =>
      conn.viaProbeId == null
      ? 'Esta central (direto)'
      : 'Agente ${conn.viaProbeName ?? conn.viaProbeId}';

  @override
  Widget build(BuildContext context) {
    final dbState = ref.watch(databasesProvider);
    final connections = dbState.connections;
    final origins = <String, String>{};
    for (final c in connections) {
      origins[_originKey(c)] = _originLabel(c);
    }

    final term = _search.trim().toLowerCase();
    final filtered = connections
        .where((c) {
          if (_originFilter != 'all' && _originKey(c) != _originFilter) {
            return false;
          }
          if (_healthFilter == 'failed' && c.lastBackupStatus != 'failed') {
            return false;
          }
          if (_healthFilter == 'no_storage' && c.storageDestinationId != null) {
            return false;
          }
          if (term.isEmpty) return true;
          final hay =
              '${c.name} ${c.host} ${c.port} ${c.username} ${c.engine} '
                      '${c.storageDestinationName ?? ''} ${_originLabel(c)} ${c.databases.join(' ')}'
                  .toLowerCase();
          return hay.contains(term);
        })
        .toList(growable: false);

    final failedCount = connections
        .where((c) => c.lastBackupStatus == 'failed')
        .length;
    final noStorageCount = connections
        .where((c) => c.storageDestinationId == null)
        .length;

    return RefreshIndicator(
      onRefresh: () => ref.read(databasesProvider.notifier).fetchConnections(),
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          SearchFilterHeader(
            title: 'Bancos de dados',
            subtitle: 'Backups agendados e sob demanda de PostgreSQL, MySQL e MariaDB.',
            searchController: _searchController,
            searchHint: 'Buscar banco, host, agente ou destino...',
            onSearchChanged: (v) => setState(() => _search = v),
            actions: <Widget>[
              OutlinedButton.icon(
                onPressed: dbState.loading
                    ? null
                    : () => ref
                          .read(databasesProvider.notifier)
                          .fetchConnections(),
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('Atualizar'),
              ),
              FilledButton.icon(
                onPressed: () => _openForm(),
                icon: const Icon(Icons.add),
                label: const Text('Adicionar banco'),
              ),
            ],
            filters: <Widget>[
              ChoiceChip(
                selected: _healthFilter == 'all',
                label: Text('Todos (${connections.length})'),
                onSelected: (_) => setState(() => _healthFilter = 'all'),
              ),
              ChoiceChip(
                selected: _healthFilter == 'failed',
                label: Text('Com falha ($failedCount)'),
                onSelected: (_) => setState(() => _healthFilter = 'failed'),
              ),
              ChoiceChip(
                selected: _healthFilter == 'no_storage',
                label: Text('Sem destino ($noStorageCount)'),
                onSelected: (_) => setState(() => _healthFilter = 'no_storage'),
              ),
              if (origins.length > 1)
                FilterChip(
                  selected: _groupByOrigin,
                  label: const Text('Agrupar por origem'),
                  onSelected: (v) => setState(() => _groupByOrigin = v),
                ),
            ],
          ),
          if (origins.length > 1) ...<Widget>[
            const SizedBox(height: 10),
            DropdownButtonFormField<String>(
              initialValue: origins.containsKey(_originFilter)
                  ? _originFilter
                  : 'all',
              decoration: const InputDecoration(
                labelText: 'Filtrar por origem',
                isDense: true,
              ),
              items: <DropdownMenuItem<String>>[
                const DropdownMenuItem(
                  value: 'all',
                  child: Text('Todas as origens'),
                ),
                ...origins.entries.map(
                  (e) => DropdownMenuItem(value: e.key, child: Text(e.value)),
                ),
              ],
              onChanged: (v) {
                if (v != null) setState(() => _originFilter = v);
              },
            ),
          ],
          const SizedBox(height: 14),

          if (dbState.error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(dbState.error!)),
            const SizedBox(height: 12),
          ],

          if (dbState.loading && !dbState.loaded)
            const LoadingStateView(message: 'Carregando bancos de dados...')
          else if (filtered.isEmpty)
            EmptyStateView(
              title: connections.isEmpty
                  ? 'Nenhum banco de dados cadastrado'
                  : 'Nenhum banco encontrado',
              description: connections.isEmpty
                  ? 'Cadastre bancos PostgreSQL, MySQL ou MariaDB diretamente ou descobrindo containers Docker.'
                  : 'Ajuste os filtros aplicados.',
              icon: Icons.storage_outlined,
              action: connections.isEmpty
                  ? FilledButton.icon(
                      onPressed: () => _openForm(),
                      icon: const Icon(Icons.add),
                      label: const Text('Cadastrar primeiro banco'),
                    )
                  : null,
            )
          else if (_groupByOrigin && origins.length > 1)
            ..._buildGrouped(filtered, dbState)
          else
            ResponsiveGrid(
              xs: 1,
              sm: 1,
              md: 2,
              lg: 2,
              xl: 2,
              children: filtered
                  .map((c) => _buildCard(c, dbState))
                  .toList(growable: false),
            ),
        ],
      ),
    );
  }

  List<Widget> _buildGrouped(
    List<DatabaseConnectionResponse> list,
    DatabasesState dbState,
  ) {
    final grouped = <String, List<DatabaseConnectionResponse>>{};
    final labels = <String, String>{};
    for (final c in list) {
      final k = _originKey(c);
      grouped.putIfAbsent(k, () => <DatabaseConnectionResponse>[]).add(c);
      labels[k] = _originLabel(c);
    }
    final keys = grouped.keys.toList()
      ..sort((a, b) {
        if (a == 'central') return -1;
        if (b == 'central') return 1;
        return labels[a]!.compareTo(labels[b]!);
      });

    return keys
        .expand((k) {
          return <Widget>[
            Padding(
              padding: const EdgeInsets.only(top: 8, bottom: 8),
              child: Text(
                '${labels[k]} (${grouped[k]!.length})',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
            ResponsiveGrid(
              xs: 1,
              sm: 1,
              md: 2,
              lg: 2,
              xl: 2,
              children: grouped[k]!
                  .map((c) => _buildCard(c, dbState))
                  .toList(growable: false),
            ),
            const SizedBox(height: 12),
          ];
        })
        .toList(growable: false);
  }

  Widget _buildCard(DatabaseConnectionResponse conn, DatabasesState dbState) {
    final activeBackupJob = dbState.jobFor('backup', conn.id);
    final activeRestoreJob = dbState.jobFor('restore', conn.id);
    final statusTone = switch (conn.lastBackupStatus) {
      'success' => SemanticTone.success,
      'failed' => SemanticTone.error,
      'running' => SemanticTone.info,
      _ =>
        conn.storageDestinationId == null
            ? SemanticTone.warning
            : SemanticTone.secondary,
    };

    return SectionCard(
      title: conn.name,
      subtitle:
          '${conn.engineLabel} · ${conn.host}:${conn.port} (${_originLabel(conn)})',
      icon: Icons.storage_outlined,
      iconTone: statusTone,
      trailing: Wrap(
        spacing: 6,
        children: <Widget>[
          SemanticChip(
            label: conn.backupEnabled
                ? 'A cada ${conn.backupIntervalHours}h'
                : 'Manual',
            tone: conn.backupEnabled
                ? SemanticTone.success
                : SemanticTone.secondary,
            compact: true,
          ),
          if (conn.lastBackupStatus == 'failed')
            const SemanticChip(
              label: 'Último falhou',
              tone: SemanticTone.error,
              compact: true,
            ),
        ],
      ),
      footer: Wrap(
        spacing: 8,
        runSpacing: 8,
        children: <Widget>[
          FilledButton.tonalIcon(
            onPressed: activeBackupJob?.isRunning == true
                ? null
                : () => _runBackupNow(conn),
            icon: const Icon(Icons.backup_outlined, size: 16),
            label: const Text('Backup agora'),
          ),
          OutlinedButton.icon(
            onPressed: () => _openHistory(conn),
            icon: const Icon(Icons.history, size: 16),
            label: const Text('Histórico'),
          ),
          PopupMenuButton<String>(
            onSelected: (action) {
              switch (action) {
                case 'edit':
                  _openForm(existing: conn);
                case 'probe':
                  _testConnection(conn);
                case 'delete':
                  _deleteConnection(conn);
              }
            },
            itemBuilder: (_) => const <PopupMenuEntry<String>>[
              PopupMenuItem(
                value: 'edit',
                child: ListTile(
                  dense: true,
                  leading: Icon(Icons.edit_outlined),
                  title: Text('Configurar backup'),
                ),
              ),
              PopupMenuItem(
                value: 'probe',
                child: ListTile(
                  dense: true,
                  leading: Icon(Icons.network_check),
                  title: Text('Testar conexão agora'),
                ),
              ),
              PopupMenuDivider(),
              PopupMenuItem(
                value: 'delete',
                child: ListTile(
                  dense: true,
                  leading: Icon(Icons.delete_outline),
                  title: Text('Excluir conexão'),
                ),
              ),
            ],
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          KeyValueRow(
            label: 'Destino',
            value: conn.storageDestinationName ?? 'Nenhum destino configurado',
          ),
          KeyValueRow(
            label: 'Escopo',
            value: conn.databases.isEmpty
                ? 'Todos os bancos'
                : conn.databases.join(', '),
          ),
          KeyValueRow(
            label: 'Retenção',
            value:
                '${countNoun(conn.backupRetention, 'cópia')} · Último: ${formatRelativeTime(conn.lastBackupAt)}',
          ),
          if (conn.lastBackupError != null &&
              conn.lastBackupError!.isNotEmpty) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(
              tone: SemanticTone.error,
              child: Text(
                conn.lastBackupError!,
                style: context.textTheme.bodySmall,
              ),
            ),
          ],
          if (activeBackupJob != null) ...<Widget>[
            const SizedBox(height: 10),
            DatabaseJobProgressBanner(job: activeBackupJob),
          ],
          if (activeRestoreJob != null) ...<Widget>[
            const SizedBox(height: 10),
            DatabaseJobProgressBanner(job: activeRestoreJob),
          ],
        ],
      ),
    );
  }
}

class _DatabaseFormBody extends ConsumerStatefulWidget {
  const _DatabaseFormBody({this.existing});
  final DatabaseConnectionResponse? existing;

  @override
  ConsumerState<_DatabaseFormBody> createState() => _DatabaseFormBodyState();
}

class _DatabaseFormBodyState extends ConsumerState<_DatabaseFormBody> {
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
  int? _storageDestinationId;
  bool _backupEnabled = true;
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
    _engine = e?.engine ?? 'postgres';
    _sslMode = e?.sslMode ?? 'prefer';
    _viaProbeId = e?.viaProbeId;
    _storageDestinationId = e?.storageDestinationId;
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
        _error = err.toString();
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
        _error = err.toString();
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
              storageDestinationId: _storageDestinationId,
              viaProbeId: _viaProbeId,
              backupEnabled: _storageDestinationId != null && _backupEnabled,
              backupIntervalHours:
                  int.tryParse(_intervalCtrl.text.trim()) ?? 24,
              backupRetention: int.tryParse(_retentionCtrl.text.trim()) ?? 7,
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
    final dbState = ref.watch(databasesProvider);
    final storages = ref.watch(storagesProvider).storages;
    final discoveredContainers = dbState.containers.containers;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // Descoberta automática de bancos em containers Docker
        if (widget.existing == null &&
            discoveredContainers.isNotEmpty) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    const Icon(Icons.auto_fix_high, size: 18),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Bancos detectados em containers Docker (${discoveredContainers.length})',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
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
          segments: const <ButtonSegment<String>>[
            ButtonSegment(value: 'postgres', label: Text('PostgreSQL')),
            ButtonSegment(value: 'mysql', label: Text('MySQL')),
            ButtonSegment(value: 'mariadb', label: Text('MariaDB')),
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

        // Rota / Ponte de banco via agente (ADR 013)
        DropdownButtonFormField<int?>(
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
              child: Text('Direto desta central'),
            ),
            ...dbState.agents.map(
              (a) => DropdownMenuItem<int?>(
                value: a.id,
                enabled: a.allowed,
                child: Text(
                  'Agente ${a.name}${!a.allowed ? ' (sem permissão database)' : (!a.connected ? ' (offline)' : '')}',
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

        Row(
          children: <Widget>[
            Expanded(
              flex: 3,
              child: TextField(
                controller: _hostCtrl,
                decoration: const InputDecoration(labelText: 'Host / IP'),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              flex: 1,
              child: TextField(
                controller: _portCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Porta'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),

        Row(
          children: <Widget>[
            Expanded(
              child: TextField(
                controller: _userCtrl,
                decoration: const InputDecoration(labelText: 'Usuário'),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: TextField(
                controller: _passCtrl,
                obscureText: true,
                decoration: InputDecoration(
                  labelText: 'Senha',
                  hintText: widget.existing?.passwordSet == true
                      ? 'definida — deixe em branco para manter'
                      : null,
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),

        DropdownButtonFormField<String>(
          initialValue: _sslMode,
          decoration: const InputDecoration(labelText: 'Modo SSL'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem(
              value: 'prefer',
              child: Text('Preferir SSL (prefer)'),
            ),
            DropdownMenuItem(
              value: 'require',
              child: Text('Exigir SSL (require)'),
            ),
            DropdownMenuItem(
              value: 'disable',
              child: Text('Desabilitado (disable)'),
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
        DropdownButtonFormField<int?>(
          initialValue: storages.any((s) => s.id == _storageDestinationId)
              ? _storageDestinationId
              : null,
          decoration: const InputDecoration(
            labelText: 'Destino de armazenamento',
          ),
          items: <DropdownMenuItem<int?>>[
            const DropdownMenuItem<int?>(
              value: null,
              child: Text('Nenhum destino'),
            ),
            ...storages.map(
              (s) => DropdownMenuItem<int?>(
                value: s.id,
                child: Text('${s.name} (${s.provider})'),
              ),
            ),
          ],
          onChanged: (v) => setState(() => _storageDestinationId = v),
        ),
        const SizedBox(height: 10),

        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _storageDestinationId != null && _backupEnabled,
          title: const Text('Backup automático agendado'),
          subtitle: const Text(
            'Snapshot consistente sem travar escrita no banco.',
          ),
          onChanged: _storageDestinationId == null
              ? null
              : (v) => setState(() => _backupEnabled = v),
        ),
        Row(
          children: <Widget>[
            Expanded(
              child: TextField(
                controller: _intervalCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Intervalo (horas)',
                ),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: TextField(
                controller: _retentionCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Retenção (cópias)',
                ),
              ),
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

class _DatabaseHistoryBody extends ConsumerStatefulWidget {
  const _DatabaseHistoryBody({required this.connection});
  final DatabaseConnectionResponse connection;

  @override
  ConsumerState<_DatabaseHistoryBody> createState() =>
      _DatabaseHistoryBodyState();
}

class _DatabaseHistoryBodyState extends ConsumerState<_DatabaseHistoryBody> {
  List<DatabaseBackupResponse> _items = const <DatabaseBackupResponse>[];
  bool _loading = true;
  String? _error;
  String _statusFilter = 'all';

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(databasesProvider.notifier)
          .history(widget.connection.id);
      if (!mounted) return;
      setState(() {
        _items = list;
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

  Future<void> _download(DatabaseBackupResponse item) async {
    if (item.storageDestinationId == null || item.objectKey == null) return;
    try {
      final res = await ref
          .read(storagesProvider.notifier)
          .downloadObject(item.storageDestinationId!, item.objectKey!);
      if (!mounted) return;
      await shareDownloadedBytes(
        context,
        bytes: res.bytes,
        filename: res.filename ?? '${item.databaseName}-${item.id}.sql.gz',
        mimeType: 'application/gzip',
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text('Erro ao baixar backup: $err')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final filtered = _items
        .where((i) {
          if (_statusFilter == 'success') return i.status == 'success';
          if (_statusFilter == 'failed') return i.status == 'failed';
          return true;
        })
        .toList(growable: false);

    final okCount = _items.where((i) => i.status == 'success').length;
    final failCount = _items.where((i) => i.status == 'failed').length;
    final totalBytes = _items.fold<int>(0, (acc, i) => acc + i.sizeBytes);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            SemanticChip(
              label: '$okCount concluídos',
              tone: SemanticTone.success,
              compact: true,
            ),
            SemanticChip(
              label: formatBytes(totalBytes),
              tone: SemanticTone.info,
              compact: true,
            ),
            if (failCount > 0)
              SemanticChip(
                label: '$failCount falhas',
                tone: SemanticTone.error,
                compact: true,
              ),
            OutlinedButton.icon(
              onPressed: _loading ? null : _load,
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('Atualizar'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        SegmentedButton<String>(
          segments: const <ButtonSegment<String>>[
            ButtonSegment(value: 'all', label: Text('Todas')),
            ButtonSegment(value: 'success', label: Text('Concluídas')),
            ButtonSegment(value: 'failed', label: Text('Falhas')),
          ],
          selected: <String>{_statusFilter},
          onSelectionChanged: (set) =>
              setState(() => _statusFilter = set.first),
        ),
        const SizedBox(height: 12),
        if (_loading)
          const LoadingStateView(message: 'Carregando histórico...')
        else if (_error != null)
          ErrorStateView(message: _error!, onRetry: _load)
        else if (filtered.isEmpty)
          const EmptyStateView(title: 'Nenhuma cópia registrada')
        else
          ...filtered.map(
            (item) => Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: NestedPanel(
                tone: item.status == 'failed' ? SemanticTone.error : null,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            item.databaseName,
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                        ),
                        SemanticChip(
                          label: item.status == 'success'
                              ? 'Concluído'
                              : (item.status == 'failed'
                                    ? 'Falhou'
                                    : 'Em andamento'),
                          tone: item.status == 'success'
                              ? SemanticTone.success
                              : (item.status == 'failed'
                                    ? SemanticTone.error
                                    : SemanticTone.info),
                          compact: true,
                        ),
                      ],
                    ),
                    const SizedBox(height: 4),
                    Text(
                      '${formatDateTime(item.startedAt)} · ${formatBytes(item.sizeBytes)} · '
                      '${formatInteger(item.tables)} tabelas · ${formatElapsedMs(item.durationMs)}',
                      style: context.textTheme.bodySmall,
                    ),
                    if (item.error != null &&
                        item.error!.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 4),
                      Text(
                        item.error!,
                        style: context.textTheme.bodySmall?.copyWith(
                          color: context.semanticColors.error,
                        ),
                      ),
                    ],
                    if (item.status == 'success') ...<Widget>[
                      const SizedBox(height: 8),
                      Wrap(
                        spacing: 8,
                        children: <Widget>[
                          if (item.storageDestinationId != null &&
                              item.objectKey != null)
                            OutlinedButton.icon(
                              onPressed: () => _download(item),
                              icon: const Icon(Icons.download, size: 16),
                              label: const Text('Baixar .sql.gz'),
                            ),
                          FilledButton.tonalIcon(
                            onPressed: () => showDatabaseRestoreDialog(
                              context,
                              backupId: item.id,
                              sourceDatabaseName: item.databaseName,
                              sourceConnectionId: widget.connection.id,
                              sourceEngine: widget.connection.engine,
                            ),
                            icon: const Icon(
                              Icons.settings_backup_restore,
                              size: 16,
                            ),
                            label: const Text('Restaurar'),
                          ),
                        ],
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

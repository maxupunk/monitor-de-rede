import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/utils/database_presentation.dart';
import 'package:netmonitor_app/features/servers/widgets/database_backup_history_sheet.dart';
import 'package:netmonitor_app/features/servers/widgets/database_connection_form_sheet.dart';
import 'package:netmonitor_app/features/servers/widgets/database_container_link_sheet.dart';
import 'package:netmonitor_app/features/servers/widgets/database_restore_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'database_backup_history_sheet.dart';
export 'database_connection_form_sheet.dart';

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

  Future<void> _openForm({DatabaseConnectionResponse? existing}) =>
      showDatabaseConnectionFormSheet(context, existing: existing);

  Future<void> _openHistory(DatabaseConnectionResponse conn) =>
      showDatabaseBackupHistorySheet(context, connection: conn);

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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Falha ao iniciar backup: ${describeError(err)}'),
        ),
      );
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
        SnackBar(
          content: Text('Falha no teste de conexão: ${describeError(err)}'),
        ),
      );
    }
  }

  void _notify(String message) {
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }

  Future<void> _linkContainer(DatabaseConnectionResponse conn) async {
    final saved = await showDatabaseContainerLinkSheet(
      context,
      connection: conn,
    );
    if (!mounted || saved == null) return;
    _notify(
      '${saved.name} agora acompanha o contêiner ${saved.container?.name} '
      '(${saved.host}:${saved.port}).',
    );
  }

  Future<void> _followContainer(DatabaseConnectionResponse conn) async {
    try {
      final saved = await ref
          .read(databasesProvider.notifier)
          .followContainer(conn.id);
      if (!mounted) return;
      final moved = saved.host != conn.host || saved.port != conn.port;
      _notify(
        moved
            ? 'Endereço atualizado: ${conn.host}:${conn.port} → ${saved.host}:${saved.port}.'
            : 'O endereço ${saved.host}:${saved.port} ainda é o do contêiner.',
      );
    } catch (err) {
      if (!mounted) return;
      _notify(
        'Não foi possível reencontrar o contêiner: ${describeError(err)}',
      );
    }
  }

  Future<void> _unlinkContainer(DatabaseConnectionResponse conn) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Fixar o endereço de ${conn.name}?',
      message:
          'A conexão deixa de acompanhar o contêiner ${conn.container?.name} e passa a usar sempre ${conn.host}:${conn.port}. '
          'Se o contêiner voltar com outro IP, o backup vai falhar até você editar o endereço.',
      confirmLabel: 'Fixar endereço',
    );
    if (confirmed != true || !mounted) return;
    try {
      await ref.read(databasesProvider.notifier).unlinkContainer(conn.id);
    } catch (err) {
      if (!mounted) return;
      _notify('Não foi possível desvincular: ${describeError(err)}');
    }
  }

  Future<void> _deleteConnection(DatabaseConnectionResponse conn) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Excluir conexão ${conn.name}?',
      message:
          'Os arquivos de backup já gravados no destino de armazenamento serão mantidos.',
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
          if (_healthFilter == 'failed') {
            final pres = resolveDatabaseBackupCardPresentation(
              c,
              dbState.jobFor('backup', c.id),
            );
            final retryingFailure =
                c.lastBackupStatus == 'failed' && pres.isRunning;
            if (!pres.isFailed && !retryingFailure) {
              return false;
            }
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
        .where(
          (c) => resolveDatabaseBackupCardPresentation(
            c,
            dbState.jobFor('backup', c.id),
          ).isFailed,
        )
        .length;
    final noStorageCount = connections
        .where((c) => c.storageDestinationId == null)
        .length;
    final isCompact = MediaQuery.sizeOf(context).width < 680;
    final activeSecondaryCount = _originFilter != 'all' ? 1 : 0;

    return RefreshIndicator(
      onRefresh: () => ref.read(databasesProvider.notifier).fetchConnections(),
      child: ListView(
        padding: const EdgeInsets.only(top: 12, bottom: 24),
        children: <Widget>[
          if (isCompact)
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Bancos de dados',
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
                  label: const Text('Adicionar'),
                ),
                IconButton(
                  tooltip: 'Atualizar',
                  onPressed: dbState.loading
                      ? null
                      : () => ref
                            .read(databasesProvider.notifier)
                            .fetchConnections(),
                  icon: const Icon(Icons.refresh, size: 20),
                ),
              ],
            )
          else
            SearchFilterHeader(
              title: 'Bancos de dados',
              subtitle:
                  'Backups agendados e sob demanda de PostgreSQL, MySQL e MariaDB.',
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
            ),
          const SizedBox(height: 10),
          UnifiedFilterBar(
            searchController: _searchController,
            searchHint: 'Buscar banco, host, agente ou destino...',
            onSearchChanged: (v) => setState(() => _search = v),
            activeFilterCount: activeSecondaryCount,
            filterSheetTitle: 'Filtros de bancos',
            filterSheetBuilder: origins.length > 1
                ? (sheetCtx, setSheetState) => Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      AppDropdownFormField<String>(
                        initialValue: origins.containsKey(_originFilter)
                            ? _originFilter
                            : 'all',
                        decoration: const InputDecoration(
                          labelText: 'Filtrar por origem',
                          isDense: true,
                        ),
                        items: <DropdownMenuItem<String>>[
                          const DropdownMenuItem<String>(
                            value: 'all',
                            child: Text(
                              'Todas as origens',
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          ...origins.entries.map(
                            (e) => DropdownMenuItem<String>(
                              value: e.key,
                              child: Text(
                                e.value,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ),
                        ],
                        onChanged: (v) {
                          if (v != null) {
                            setState(() => _originFilter = v);
                            setSheetState(() {});
                          }
                        },
                      ),
                      const SizedBox(height: 8),
                      SwitchListTile(
                        contentPadding: EdgeInsets.zero,
                        value: _groupByOrigin,
                        title: const Text('Agrupar por origem'),
                        onChanged: (v) {
                          setState(() => _groupByOrigin = v);
                          setSheetState(() {});
                        },
                      ),
                    ],
                  )
                : null,
            onClearFilters: () => setState(() {
              _originFilter = 'all';
              _healthFilter = 'all';
            }),
            quickChips: <Widget>[
              ChoiceChip(
                selected: _healthFilter == 'all',
                visualDensity: VisualDensity.compact,
                label: Text('Todos (${connections.length})'),
                onSelected: (_) => setState(() => _healthFilter = 'all'),
              ),
              ChoiceChip(
                selected: _healthFilter == 'failed',
                visualDensity: VisualDensity.compact,
                label: Text('Com falha ($failedCount)'),
                onSelected: (_) => setState(() => _healthFilter = 'failed'),
              ),
              ChoiceChip(
                selected: _healthFilter == 'no_storage',
                visualDensity: VisualDensity.compact,
                label: Text('Sem destino ($noStorageCount)'),
                onSelected: (_) => setState(() => _healthFilter = 'no_storage'),
              ),
            ],
            activeChips: <ActiveFilterChipData>[
              if (_originFilter != 'all' && origins.containsKey(_originFilter))
                ActiveFilterChipData(
                  label: origins[_originFilter]!,
                  icon: Icons.dns_outlined,
                  onDeleted: () => setState(() => _originFilter = 'all'),
                ),
            ],
          ),
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
    final presentation = resolveDatabaseBackupCardPresentation(
      conn,
      activeBackupJob,
    );
    final advice = resolveDatabaseAddressAdvice(conn, presentation);
    final link = conn.container;

    return SectionCard(
      title: conn.name,
      subtitle:
          '${conn.engineLabel} · ${conn.host}:${conn.port} (${_originLabel(conn)})',
      icon: Icons.storage_outlined,
      iconTone: presentation.iconTone,
      trailing: SemanticChip(
        label: presentation.badgeLabel,
        tone: presentation.badgeTone,
        compact: true,
      ),
      footer: Row(
        children: <Widget>[
          Expanded(
            child: SymmetricalActionBar(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                FilledButton.tonalIcon(
                  style: FilledButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 10,
                    ),
                  ),
                  onPressed: activeBackupJob?.isRunning == true
                      ? null
                      : () => _runBackupNow(conn),
                  icon: const Icon(Icons.backup_outlined, size: 16),
                  label: const Text(
                    'Backup agora',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 10,
                    ),
                  ),
                  onPressed: () => _openHistory(conn),
                  icon: const Icon(Icons.history, size: 16),
                  label: const Text(
                    'Histórico',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 4),
          PopupMenuButton<String>(
            tooltip: 'Mais ações',
            onSelected: (action) {
              switch (action) {
                case 'edit':
                  _openForm(existing: conn);
                case 'probe':
                  _testConnection(conn);
                case 'link':
                  _linkContainer(conn);
                case 'follow':
                  _followContainer(conn);
                case 'unlink':
                  _unlinkContainer(conn);
                case 'delete':
                  _deleteConnection(conn);
              }
            },
            itemBuilder: (_) => <PopupMenuEntry<String>>[
              const PopupMenuItem<String>(
                value: 'edit',
                child: ListTile(
                  dense: true,
                  leading: Icon(Icons.edit_outlined),
                  title: Text('Configurar backup'),
                ),
              ),
              const PopupMenuItem<String>(
                value: 'probe',
                child: ListTile(
                  dense: true,
                  leading: Icon(Icons.network_check),
                  title: Text('Testar conexão agora'),
                ),
              ),
              if (link == null)
                const PopupMenuItem<String>(
                  value: 'link',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.link),
                    title: Text('Vincular a um contêiner'),
                  ),
                )
              else ...<PopupMenuEntry<String>>[
                const PopupMenuItem<String>(
                  value: 'follow',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.travel_explore),
                    title: Text('Reencontrar endereço agora'),
                  ),
                ),
                const PopupMenuItem<String>(
                  value: 'unlink',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.link_off),
                    title: Text('Fixar endereço (desvincular)'),
                  ),
                ),
              ],
              const PopupMenuDivider(),
              const PopupMenuItem<String>(
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
            label: 'Endereço',
            value: link == null
                ? 'Fixo · ${conn.host}:${conn.port}'
                : 'Acompanha o contêiner ${link.name}'
                      '${conn.containerHostName != null ? ' (${conn.containerHostName})' : ''}',
          ),
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
          if (presentation.showLastBackupError) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(
              tone: SemanticTone.error,
              child: Text(
                conn.lastBackupError!,
                style: context.textTheme.bodySmall,
              ),
            ),
          ],
          if (advice != DatabaseAddressAdvice.none) ...<Widget>[
            const SizedBox(height: 8),
            _AddressAdvicePanel(
              advice: advice,
              onLink: () => _linkContainer(conn),
              onFollow: () => _followContainer(conn),
            ),
          ],
          if (presentation.showBackupJobBanner &&
              activeBackupJob != null) ...<Widget>[
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

/// Sugestão do card quando o backup não alcançou o banco: vincular ao
/// contêiner (o endereço passa a acompanhá-lo) ou reencontrá-lo agora.
class _AddressAdvicePanel extends StatelessWidget {
  const _AddressAdvicePanel({
    required this.advice,
    required this.onLink,
    required this.onFollow,
  });

  final DatabaseAddressAdvice advice;
  final VoidCallback onLink;
  final VoidCallback onFollow;

  @override
  Widget build(BuildContext context) {
    final linking = advice == DatabaseAddressAdvice.linkContainer;
    return NestedPanel(
      tone: SemanticTone.info,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            linking
                ? 'O banco roda num contêiner? Ao reiniciar, ele pode ter voltado com outro IP. '
                      'Vincule a conexão ao contêiner e o endereço passa a acompanhá-lo.'
                : 'O contêiner não respondeu no último backup. '
                      'Reencontre o endereço agora para não esperar o próximo.',
            style: context.textTheme.bodySmall,
          ),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerLeft,
            child: FilledButton.tonalIcon(
              onPressed: linking ? onLink : onFollow,
              icon: Icon(linking ? Icons.link : Icons.travel_explore, size: 16),
              label: Text(
                linking ? 'Vincular a um contêiner' : 'Reencontrar endereço',
              ),
            ),
          ),
        ],
      ),
    );
  }
}

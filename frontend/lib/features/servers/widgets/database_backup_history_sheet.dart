import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/file_download.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/widgets/database_restore_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o histórico de cópias de uma conexão de banco de dados.
Future<void> showDatabaseBackupHistorySheet(
  BuildContext context, {
  required DatabaseConnectionResponse connection,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Histórico de cópias · ${connection.name}',
    subtitle:
        '${connection.engineLabel} · ${connection.host}:${connection.port}',
    maxWidth: 740,
    builder: (_) => DatabaseBackupHistoryBody(connection: connection),
  );
}

class DatabaseBackupHistoryBody extends ConsumerStatefulWidget {
  const DatabaseBackupHistoryBody({super.key, required this.connection});
  final DatabaseConnectionResponse connection;

  @override
  ConsumerState<DatabaseBackupHistoryBody> createState() =>
      _DatabaseBackupHistoryBodyState();
}

class _DatabaseBackupHistoryBodyState
    extends ConsumerState<DatabaseBackupHistoryBody> {
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
        _error = describeError(
          err,
          fallback: 'Erro ao carregar histórico de cópias.',
        );
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
      final safeDb = sanitizeFileSlug(item.databaseName, fallback: 'backup');
      await saveOrShareBytes(
        context: context,
        bytes: res.bytes,
        filename: res.filename ?? '$safeDb-${item.id}.sql.gz',
        mimeType: 'application/gzip',
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao baixar backup: ${describeError(err)}')),
      );
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
          showSelectedIcon: false,
          style: SegmentedButton.styleFrom(
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 8),
          ),
          segments: const <ButtonSegment<String>>[
            ButtonSegment<String>(
              value: 'all',
              label: Text('Todas', maxLines: 1, softWrap: false),
            ),
            ButtonSegment<String>(
              value: 'success',
              label: Text('Concluídas', maxLines: 1, softWrap: false),
            ),
            ButtonSegment<String>(
              value: 'failed',
              label: Text('Falhas', maxLines: 1, softWrap: false),
            ),
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

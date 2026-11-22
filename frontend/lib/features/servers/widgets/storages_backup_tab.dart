import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/storage_explorer_sheet.dart';
import 'package:netmonitor_app/features/servers/widgets/storage_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'storage_explorer_sheet.dart';
export 'storage_form_sheet.dart';

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
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Erro ao abrir destino: ${describeError(err)}'),
          ),
        );
        return;
      }
    }
    if (!mounted) return;
    await showStorageFormSheet(context, existing: detail);
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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Erro ao testar destino: ${describeError(err)}'),
        ),
      );
    }
  }

  Future<void> _openExplorer(StorageDestinationResponse storage) =>
      showStorageExplorerSheet(context, storage: storage);

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

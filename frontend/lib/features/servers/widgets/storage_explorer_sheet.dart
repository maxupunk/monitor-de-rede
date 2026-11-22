import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/file_download.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o explorador modal de arquivos/pastas de um destino de armazenamento.
Future<void> showStorageExplorerSheet(
  BuildContext context, {
  required StorageDestinationResponse storage,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Explorar · ${storage.name}',
    subtitle: storage.target ?? storageProviderInfo(storage.provider).label,
    maxWidth: 740,
    builder: (_) => StorageExplorerBody(storage: storage),
  );
}

class StorageExplorerBody extends ConsumerStatefulWidget {
  const StorageExplorerBody({super.key, required this.storage});
  final StorageDestinationResponse storage;

  @override
  ConsumerState<StorageExplorerBody> createState() =>
      _StorageExplorerBodyState();
}

class _StorageExplorerBodyState extends ConsumerState<StorageExplorerBody> {
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
        _error = describeError(err, fallback: 'Erro ao explorar destino.');
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
      await saveOrShareBytes(
        context: context,
        bytes: res.bytes,
        filename: res.filename ?? obj.name,
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Erro ao baixar arquivo: ${describeError(err)}'),
        ),
      );
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

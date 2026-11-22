import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/file_download.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Volumes" do Docker (`/docker/volumes`).
class DockerVolumesPage extends ConsumerStatefulWidget {
  const DockerVolumesPage({super.key});

  @override
  ConsumerState<DockerVolumesPage> createState() => _DockerVolumesPageState();
}

class _DockerVolumesPageState extends ConsumerState<DockerVolumesPage> {
  final TextEditingController _searchController = TextEditingController();
  String _search = '';

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _exportVolume(String name) async {
    try {
      final result = await ref.read(dockerProvider.notifier).exportVolume(name);
      if (!mounted) return;
      final safe = sanitizeFileSlug(name, fallback: 'volume');
      final stamp = formatFileTimestamp();
      await saveOrShareBytes(
        context: context,
        bytes: result.bytes,
        filename: result.filename ?? 'volume-$safe-$stamp.tar.gz',
        mimeType: 'application/gzip',
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Erro ao exportar volume: ${describeError(err)}'),
        ),
      );
    }
  }

  Future<void> _removeVolume(DockerVolumeSummary volume) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Remover volume ${volume.name}?',
      message:
          'Todos os arquivos gravados neste volume serão apagados permanentemente.',
      confirmLabel: 'Remover volume',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref
        .read(dockerProvider.notifier)
        .removeVolume(volume.name);
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Volume ${volume.name} removido.')),
      );
    }
  }

  Future<void> _inspectVolume(
    DockerVolumeSummary volume,
    bool canExport,
  ) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: volume.name,
      subtitle: 'Driver ${volume.driver} · Escopo ${volume.scope}',
      builder: (_) => _VolumeDetailBody(
        volumeName: volume.name,
        canExport: canExport,
        onExport: () => _exportVolume(volume.name),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final isAdmin = ref.watch(authProvider).isAdmin;
    final canExport = isAdmin && docker.allows('volume_export');

    final term = _search.trim().toLowerCase();
    final filtered = docker.current.volumes
        .where((v) {
          if (term.isEmpty) return true;
          return '${v.name} ${v.driver} ${v.composeProject ?? ''}'
              .toLowerCase()
              .contains(term);
        })
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        UnifiedFilterBar(
          searchController: _searchController,
          searchHint: 'Buscar volume por nome, driver ou projeto...',
          onSearchChanged: (v) => setState(() => _search = v),
        ),
        const SizedBox(height: 14),
        if (filtered.isEmpty)
          const EmptyStateView(
            title: 'Nenhum volume encontrado',
            icon: Icons.storage_outlined,
          )
        else
          ...filtered.map(
            (vol) => Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Card(
                elevation: 2,
                shape: RoundedRectangleBorder(
                  borderRadius: AppTheme.borderRadius,
                ),
                child: ListTile(
                  onTap: () => _inspectVolume(vol, canExport),
                  leading: const Icon(Icons.storage_outlined),
                  title: Text(
                    vol.name,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  subtitle: Text(
                    'Driver: ${vol.driver} · Escopo: ${vol.scope}'
                    '${vol.composeProject != null ? ' · Projeto: ${vol.composeProject}' : ''}'
                    '${vol.createdAt != null ? ' · ${formatDateTime(vol.createdAt)}' : ''}',
                  ),
                  trailing: Wrap(
                    spacing: 4,
                    children: <Widget>[
                      if (canExport)
                        IconButton(
                          tooltip: 'Exportar .tar.gz',
                          icon: const Icon(Icons.download_outlined),
                          onPressed: () => _exportVolume(vol.name),
                        ),
                      if (isAdmin)
                        IconButton(
                          tooltip: 'Remover volume',
                          icon: const Icon(Icons.delete_outline),
                          onPressed: () => _removeVolume(vol),
                        ),
                    ],
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }
}

class _VolumeDetailBody extends ConsumerStatefulWidget {
  const _VolumeDetailBody({
    required this.volumeName,
    required this.canExport,
    required this.onExport,
  });

  final String volumeName;
  final bool canExport;
  final VoidCallback onExport;

  @override
  ConsumerState<_VolumeDetailBody> createState() => _VolumeDetailBodyState();
}

class _VolumeDetailBodyState extends ConsumerState<_VolumeDetailBody> {
  DockerVolumeDetail? _detail;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    try {
      final d = await ref
          .read(dockerProvider.notifier)
          .fetchVolumeDetail(widget.volumeName);
      if (!mounted) return;
      setState(() {
        _detail = d;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'Erro ao inspecionar volume.');
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Inspecionando volume...');
    }
    if (_error != null) return ErrorStateView(message: _error!, onRetry: _load);
    final d = _detail!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        KeyValueRow(label: 'Driver', value: d.driver),
        KeyValueRow(label: 'Escopo', value: d.scope),
        KeyValueRow(label: 'Criado em', value: formatDateTime(d.createdAt)),
        KeyValueRow(
          label: 'Mountpoint',
          value: d.mountpoint,
          monospace: true,
          copyable: true,
        ),
        if (d.options.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          CodeViewer(
            title: 'Opções do volume',
            code: d.options.entries
                .map((e) => '${e.key}=${e.value}')
                .join('\n'),
          ),
        ],
        if (widget.canExport) ...<Widget>[
          const SizedBox(height: 14),
          FilledButton.icon(
            onPressed: widget.onExport,
            icon: const Icon(Icons.download_outlined),
            label: const Text('Exportar volume (.tar.gz)'),
          ),
        ],
      ],
    );
  }
}

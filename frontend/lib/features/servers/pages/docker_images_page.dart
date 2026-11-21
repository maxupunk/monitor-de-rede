import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Imagens" do Docker (`/docker/images`).
class DockerImagesPage extends ConsumerStatefulWidget {
  const DockerImagesPage({super.key});

  @override
  ConsumerState<DockerImagesPage> createState() => _DockerImagesPageState();
}

class _DockerImagesPageState extends ConsumerState<DockerImagesPage> {
  final TextEditingController _searchController = TextEditingController();
  String _search = '';

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _openPullDialog() async {
    final controller = TextEditingController();
    final pulled = await showAdaptiveFormSheet<bool>(
      context: context,
      title: 'Baixar imagem (docker pull)',
      builder: (ctx) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextField(
            controller: controller,
            autofocus: true,
            decoration: const InputDecoration(
              labelText: 'Referência da imagem',
              hintText: 'ex.: nginx:alpine ou postgres:16',
            ),
          ),
          const SizedBox(height: 16),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: () async {
                final image = controller.text.trim();
                if (image.isEmpty) return;
                final ok = await ref
                    .read(dockerProvider.notifier)
                    .runAction(
                      () => ref
                          .read(dockerProvider.notifier)
                          .apiFor()
                          .pullImage(image),
                    );
                if (ctx.mounted) Navigator.of(ctx).pop(ok);
              },
              icon: const Icon(Icons.download_outlined),
              label: const Text('Baixar imagem'),
            ),
          ),
        ],
      ),
    );
    controller.dispose();
    if (pulled == true && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Pull iniciado! Acompanhe o andamento no painel de operações.',
          ),
        ),
      );
    }
  }

  Future<void> _pruneImages() async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Limpar imagens sem uso?',
      message:
          'Remove imagens órfãs que não estão em uso por nenhum container.',
      confirmLabel: 'Limpar sem uso',
      tone: SemanticTone.warning,
    );
    if (!confirmed || !mounted) return;
    try {
      final result = await ref
          .read(dockerProvider.notifier)
          .apiFor()
          .pruneImages();
      await ref.read(dockerProvider.notifier).refreshAll();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            '${countNoun(result.imagesDeleted, 'imagem removida', 'imagens removidas')} · '
            '${formatDecimalBytes(result.spaceReclaimed)} liberados.',
          ),
        ),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro ao limpar imagens: $err')));
    }
  }

  Future<void> _removeImage(DockerImageSummary image) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Remover imagem ${image.primaryTag}?',
      message: 'Se a imagem estiver em uso por algum container, a Engine recusará a remoção.',
      confirmLabel: 'Remover imagem',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref
        .read(dockerProvider.notifier)
        .runAction(
          () =>
              ref.read(dockerProvider.notifier).apiFor().removeImage(image.id),
          refresh: ref.read(dockerProvider.notifier).refreshAll,
        );
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Imagem ${image.primaryTag} removida.')),
      );
    }
  }

  Future<void> _inspectImage(DockerImageSummary image) {
    return showAdaptiveFormSheet<void>(
      context: context,
      title: image.primaryTag,
      subtitle: shortDockerId(image.id),
      maxWidth: 680,
      builder: (_) => _ImageDetailBody(imageId: image.id),
    );
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final isAdmin = ref.watch(authProvider).isAdmin;
    final canUpdate = isAdmin && docker.allows('update');
    final term = _search.trim().toLowerCase();
    final filtered = docker.current.images
        .where((img) {
          if (term.isEmpty) return true;
          return '${img.repoTags.join(' ')} ${img.repoDigests.join(' ')} ${img.id}'
              .toLowerCase()
              .contains(term);
        })
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          searchController: _searchController,
          searchHint: 'Buscar imagem por tag, digest ou ID...',
          onSearchChanged: (v) => setState(() => _search = v),
          actions: <Widget>[
            if (canUpdate)
              OutlinedButton.icon(
                onPressed: _openPullDialog,
                icon: const Icon(Icons.download_outlined, size: 18),
                label: const Text('Baixar imagem'),
              ),
            if (isAdmin)
              FilledButton.tonalIcon(
                onPressed: _pruneImages,
                icon: const Icon(Icons.cleaning_services_outlined, size: 18),
                label: const Text('Limpar sem uso'),
              ),
          ],
        ),
        const SizedBox(height: 14),
        if (filtered.isEmpty)
          const EmptyStateView(
            title: 'Nenhuma imagem encontrada',
            icon: Icons.layers_outlined,
          )
        else
          ...filtered.map((img) {
            final createdAt = img.created > 0
                ? DateTime.fromMillisecondsSinceEpoch(img.created * 1000)
                : null;
            return Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Card(
                elevation: 2,
                shape: RoundedRectangleBorder(
                  borderRadius: AppTheme.borderRadius,
                ),
                child: ListTile(
                  onTap: () => _inspectImage(img),
                  leading: const Icon(Icons.layers_outlined),
                  title: Text(
                    img.primaryTag,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  subtitle: Text(
                    '${shortDockerId(img.id)} · ${formatDecimalBytes(img.size)}'
                    '${createdAt != null ? ' · Criada em ${formatDate(createdAt)}' : ''}'
                    ' · ${countNoun(img.containers, 'container')}',
                  ),
                  trailing: isAdmin
                      ? IconButton(
                          tooltip: 'Remover imagem',
                          icon: const Icon(Icons.delete_outline),
                          onPressed: () => _removeImage(img),
                        )
                      : null,
                ),
              ),
            );
          }),
      ],
    );
  }
}

class _ImageDetailBody extends ConsumerStatefulWidget {
  const _ImageDetailBody({required this.imageId});
  final String imageId;

  @override
  ConsumerState<_ImageDetailBody> createState() => _ImageDetailBodyState();
}

class _ImageDetailBodyState extends ConsumerState<_ImageDetailBody> {
  DockerImageDetail? _detail;
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
          .apiFor()
          .imageDetail(widget.imageId);
      if (!mounted) return;
      setState(() {
        _detail = d;
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
      return const LoadingStateView(message: 'Inspecionando imagem...');
    }
    if (_error != null) return ErrorStateView(message: _error!, onRetry: _load);
    final d = _detail!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        KeyValueRow(
          label: 'ID',
          value: shortDockerId(d.id),
          monospace: true,
          copyable: true,
        ),
        KeyValueRow(label: 'Tamanho', value: formatDecimalBytes(d.size)),
        KeyValueRow(label: 'Criada em', value: formatDateTime(d.created)),
        KeyValueRow(
          label: 'Usuário',
          value: d.user?.isNotEmpty == true ? d.user! : 'root',
        ),
        KeyValueRow(
          label: 'WorkingDir',
          value: d.workingDir?.isNotEmpty == true ? d.workingDir! : '/',
        ),
        KeyValueRow(label: 'Camadas', value: formatInteger(d.layers.length)),
        if (d.command.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          CodeViewer(title: 'CMD', code: d.command.join(' ')),
        ],
        if (d.environment.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          CodeViewer(
            title: 'Variáveis de ambiente (segredos mascarados)',
            code: d.environment.join('\n'),
            maxHeight: 180,
          ),
        ],
      ],
    );
  }
}

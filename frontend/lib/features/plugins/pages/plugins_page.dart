import 'dart:convert';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/data/plugins_repository.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_editor_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_review_dialog.dart';
import 'package:netmonitor_app/features/vpn/utils/file_export_helper.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const Map<String, int> _statusRank = <String, int>{
  'quarantine': 0,
  'tested': 1,
  'draft': 2,
  'active': 3,
  'disabled': 4,
};

/// Página `/plugins` (`area: ProductArea.network`, `requiresAdmin: true`).
class PluginsPage extends ConsumerStatefulWidget {
  const PluginsPage({super.key});

  @override
  ConsumerState<PluginsPage> createState() => _PluginsPageState();
}

class _PluginsPageState extends ConsumerState<PluginsPage> {
  final _searchCtrl = TextEditingController();
  String _filter = 'all'; // 'all' | 'pending' | 'active' | 'disabled'
  String _query = '';

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(pluginsProvider.notifier).fetchLibrary();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _importPluginFile() async {
    final picked = await FilePicker.platform.pickFiles(
      type: FileType.custom,
      allowedExtensions: const <String>['json'],
      withData: true,
    );
    if (picked == null || picked.files.isEmpty) return;
    final bytes = picked.files.first.bytes;
    if (bytes == null) return;
    try {
      final raw = jsonDecode(utf8.decode(bytes)) as Map<String, dynamic>;
      final pkg = PluginPackage.fromJson(raw);
      // Importa SEMPRE em quarentena e abre revisão de segurança imediatamente (ADR 012).
      final detail = await ref.read(pluginsProvider.notifier).importPlugin(pkg);
      if (mounted) {
        await showPluginReviewDialog(context: context, detail: detail);
      }
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro ao importar plugin: $err')));
    }
  }

  Future<void> _exportPlugin(PluginSummary summary) async {
    try {
      final res = await ref
          .read(pluginsRepositoryProvider)
          .exportPlugin(summary.id);
      await exportFileBytes(
        bytes: res.bytes,
        fileName:
            res.filename ?? '${summary.slug}-${summary.version}.nmplugin.json',
        mimeType: 'application/json',
      );
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Pacote ${summary.slug} exportado.')),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro ao exportar: $err')));
    }
  }

  Future<void> _openReview(PluginSummary summary) async {
    final detail = await ref
        .read(pluginsProvider.notifier)
        .fetchDetail(summary.id);
    if (mounted) {
      await showPluginReviewDialog(context: context, detail: detail);
    }
  }

  Future<void> _openEditor(PluginSummary? summary) async {
    PluginDetail? existing;
    if (summary != null) {
      existing = await ref
          .read(pluginsProvider.notifier)
          .fetchDetail(summary.id);
    }
    if (mounted) {
      await showPluginEditorDialog(context: context, existing: existing);
    }
  }

  Future<void> _deletePlugin(PluginSummary summary) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Excluir "${summary.name}"?',
      message:
          'O plugin será removido da biblioteca e desinstalado de todos os equipamentos.',
      confirmLabel: 'Excluir',
      tone: SemanticTone.error,
    );
    if (!ok) return;
    await ref.read(pluginsProvider.notifier).deletePlugin(summary.id);
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(pluginsProvider);
    final attentionCount = state.library
        .where((p) => p.status == 'quarantine' || p.status == 'tested')
        .length;

    final filtered =
        state.library.where((p) {
          if (_filter == 'pending' &&
              p.status != 'quarantine' &&
              p.status != 'tested' &&
              p.status != 'draft') {
            return false;
          }
          if (_filter == 'active' && p.status != 'active') return false;
          if (_filter == 'disabled' && p.status != 'disabled') return false;
          if (_query.isNotEmpty) {
            final q = _query.toLowerCase();
            return p.name.toLowerCase().contains(q) ||
                p.slug.toLowerCase().contains(q) ||
                (p.description?.toLowerCase().contains(q) ?? false);
          }
          return true;
        }).toList()..sort((a, b) {
          final ra = _statusRank[a.status] ?? 9;
          final rb = _statusRank[b.status] ?? 9;
          if (ra != rb) return ra.compareTo(rb);
          return a.name.toLowerCase().compareTo(b.name.toLowerCase());
        });

    final isCompact = MediaQuery.sizeOf(context).width < 680;
    const filterEntries = <(String, String)>[
      ('all', 'Todos'),
      ('pending', 'Em preparo'),
      ('active', 'Ativos'),
      ('disabled', 'Desativados'),
    ];

    return ListView(
      padding: EdgeInsets.zero,
      children: <Widget>[
        if (isCompact) ...<Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  'Plugins',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              FilledButton.icon(
                style: FilledButton.styleFrom(
                  backgroundColor: context.semanticColors.plugin,
                  foregroundColor: context.semanticColors.onPlugin,
                ),
                onPressed: () => _openEditor(null),
                icon: const Icon(Icons.add, size: 18),
                label: const Text('Novo'),
              ),
              PopupMenuButton<String>(
                tooltip: 'Mais ações',
                onSelected: (cmd) {
                  if (cmd == 'import') _importPluginFile();
                },
                itemBuilder: (_) => const <PopupMenuEntry<String>>[
                  PopupMenuItem<String>(
                    value: 'import',
                    child: ListTile(
                      dense: true,
                      leading: Icon(Icons.upload_file_outlined),
                      title: Text('Importar pacote (.json)'),
                    ),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 10),
          UnifiedFilterBar(
            searchController: _searchCtrl,
            searchHint: 'Buscar por nome, slug ou descrição...',
            onSearchChanged: (v) => setState(() => _query = v.trim()),
            quickChips: <Widget>[
              for (final entry in filterEntries)
                ChoiceChip(
                  label: Text(entry.$2),
                  visualDensity: VisualDensity.compact,
                  selected: _filter == entry.$1,
                  onSelected: (_) => setState(() => _filter = entry.$1),
                ),
            ],
          ),
        ] else
          SearchFilterHeader(
            title: 'Plugins de dispositivo',
            subtitle:
                'Biblioteca de scripts Rhai em sandbox com revisão de quarentena e aplicativos de frota',
            searchController: _searchCtrl,
            searchHint: 'Buscar por nome, slug ou descrição...',
            onSearchChanged: (v) => setState(() => _query = v.trim()),
            filters: <Widget>[
              for (final entry in filterEntries)
                ChoiceChip(
                  label: Text(entry.$2),
                  selected: _filter == entry.$1,
                  onSelected: (_) => setState(() => _filter = entry.$1),
                ),
            ],
            actions: <Widget>[
              OutlinedButton.icon(
                onPressed: _importPluginFile,
                icon: const Icon(Icons.upload_file_outlined, size: 18),
                label: const Text('Importar'),
              ),
              FilledButton.icon(
                style: FilledButton.styleFrom(
                  backgroundColor: context.semanticColors.plugin,
                  foregroundColor: context.semanticColors.onPlugin,
                ),
                onPressed: () => _openEditor(null),
                icon: const Icon(Icons.add, size: 18),
                label: const Text('Novo plugin'),
              ),
            ],
          ),
        if (attentionCount > 0) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.warning,
            child: Wrap(
              spacing: 8,
              runSpacing: 6,
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                Text(
                  '${countNoun(attentionCount, 'plugin precisa', 'plugins precisam')} de revisão de segurança ou ativação.',
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
                TextButton(
                  onPressed: () => setState(() => _filter = 'pending'),
                  child: const Text('Ver pendentes'),
                ),
              ],
            ),
          ),
        ],
        if (state.error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(state.error!)),
        ],
        const SizedBox(height: 14),
        if (state.libraryLoading && state.library.isEmpty)
          const LoadingStateView(message: 'Carregando biblioteca...')
        else if (filtered.isEmpty)
          const EmptyStateView(
            title: 'Nenhum plugin encontrado',
            description:
                'Crie um novo plugin ou importe um arquivo .nmplugin.json para revisar na quarentena.',
            icon: Icons.extension_outlined,
            tone: SemanticTone.plugin,
          )
        else
          ResponsiveGrid(
            xs: 1,
            sm: 2,
            md: 2,
            lg: 3,
            xl: 3,
            children: filtered
                .map((summary) => _buildPluginCard(context, summary))
                .toList(growable: false),
          ),
      ],
    );
  }

  Widget _buildPluginCard(BuildContext context, PluginSummary summary) {
    final statusPres = pluginStatusPresentation(summary.status);
    final sourcePres = pluginSourcePresentation(summary.source);
    final showRisk =
        summary.risk == 'medium' ||
        summary.risk == 'high' ||
        summary.risk == 'critical';

    return SectionCard(
      title: summary.name,
      subtitle: '${summary.slug} · v${summary.version}',
      icon: Icons.extension_outlined,
      iconTone: SemanticTone.plugin,
      trailing: SemanticChip(
        label: statusPres.label,
        tone: statusPres.tone,
        icon: statusPres.icon,
        compact: true,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (summary.description != null &&
              summary.description!.isNotEmpty) ...<Widget>[
            Text(
              summary.description!,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 10),
          ],
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: <Widget>[
              SemanticChip(
                label: sourcePres.label,
                tone: sourcePres.tone,
                compact: true,
              ),
              for (final t in summary.transports)
                SemanticChip(
                  label: t.toUpperCase(),
                  tone: SemanticTone.plugin,
                  compact: true,
                ),
              if (showRisk)
                SemanticChip(
                  label: 'Risco ${severityPresentation(summary.risk!).label}',
                  tone: severityPresentation(summary.risk!).tone,
                  compact: true,
                ),
              if (summary.validatedCount > 0)
                SemanticChip(
                  label: '${summary.validatedCount} validado(s)',
                  tone: SemanticTone.success,
                  compact: true,
                ),
            ],
          ),
          const SizedBox(height: 12),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              _buildPrimaryCta(summary),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (summary.opensApp)
                    TextButton.icon(
                      onPressed: () => context.go('/apps/${summary.id}'),
                      icon: const Icon(Icons.apps, size: 17),
                      label: const Text('Aplicativo'),
                    ),
                  TextButton(
                    onPressed: () => _openEditor(summary),
                    child: Text(
                      summary.source == 'builtin' ? 'Ver código' : 'Editar',
                    ),
                  ),
                  PopupMenuButton<String>(
                    iconSize: 18,
                    onSelected: (cmd) async {
                      if (cmd == 'export') {
                        await _exportPlugin(summary);
                      } else if (cmd == 'disable') {
                        await ref
                            .read(pluginsProvider.notifier)
                            .lifecycle(summary.id, 'disable');
                      } else if (cmd == 'delete') {
                        await _deletePlugin(summary);
                      }
                    },
                    itemBuilder: (_) => <PopupMenuEntry<String>>[
                      const PopupMenuItem<String>(
                        value: 'export',
                        child: Text('Exportar pacote'),
                      ),
                      if (summary.status == 'active')
                        const PopupMenuItem<String>(
                          value: 'disable',
                          child: Text('Desativar'),
                        ),
                      if (summary.source != 'builtin')
                        const PopupMenuItem<String>(
                          value: 'delete',
                          child: Text('Excluir'),
                        ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildPrimaryCta(PluginSummary summary) {
    switch (summary.status) {
      case 'quarantine':
        return FilledButton.icon(
          onPressed: () => _openReview(summary),
          icon: const Icon(Icons.security_outlined, size: 17),
          label: const Text('Revisar segurança'),
        );
      case 'tested':
        return FilledButton.icon(
          onPressed: () => ref
              .read(pluginsProvider.notifier)
              .lifecycle(summary.id, 'promote'),
          icon: const Icon(Icons.check_circle_outline, size: 17),
          label: const Text('Ativar'),
        );
      case 'draft':
        return FilledButton.tonalIcon(
          onPressed: () => _openEditor(summary),
          icon: const Icon(Icons.science_outlined, size: 17),
          label: const Text('Abrir e testar'),
        );
      case 'disabled':
        return FilledButton.tonalIcon(
          onPressed: () => ref
              .read(pluginsProvider.notifier)
              .lifecycle(summary.id, 'enable'),
          icon: const Icon(Icons.power_settings_new, size: 17),
          label: const Text('Reativar'),
        );
      default:
        return SemanticChip(
          label: 'Ativo',
          tone: SemanticTone.plugin,
          icon: Icons.check,
          compact: true,
        );
    }
  }
}

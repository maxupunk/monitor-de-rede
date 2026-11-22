import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_chat_thread.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Página `/ai-chat` (`area: ProductArea.core`, `fullBleed: true`).
class AiChatPage extends ConsumerStatefulWidget {
  const AiChatPage({super.key});

  @override
  ConsumerState<AiChatPage> createState() => _AiChatPageState();
}

class _AiChatPageState extends ConsumerState<AiChatPage> {
  final _searchCtrl = TextEditingController();
  String _search = '';

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(aiSettingsProvider.notifier).load();
      ref.read(aiConversationsProvider.notifier).fetchAll();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _renameConversation(AiConversationSummary conv) async {
    final next = await showDialog<String>(
      context: context,
      builder: (_) => _RenameConversationDialog(initialTitle: conv.title),
    );
    if (next == null || next.isEmpty || next == conv.title) return;
    await ref.read(aiConversationsProvider.notifier).rename(conv.id, next);
  }

  Future<void> _deleteConversation(AiConversationSummary conv) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Excluir conversa?',
      message: 'A conversa "${conv.title}" será removida permanentemente.',
      confirmLabel: 'Excluir',
      tone: SemanticTone.error,
    );
    if (!ok) return;
    final active = ref.read(aiConversationsProvider).activeId == conv.id;
    await ref.read(aiConversationsProvider.notifier).remove(conv.id);
    if (active) {
      ref.read(aiChatProvider.notifier).newConversation();
    }
  }

  void _showCapabilitiesSheet(AiSettings? settings) {
    showAdaptiveFormSheet<void>(
      context: context,
      title: 'O que o Assistente IA pode fazer',
      subtitle: settings != null
          ? 'Provedor: ${settings.activeDriver} · Modelo: ${settings.currentModelLabel}'
          : null,
      maxWidth: 480,
      builder: (_) {
        const capabilities = <String>[
          'Consultar dispositivos, interfaces, monitores e métricas de latência/tráfego',
          'Analisar alertas ativos, correlação e causa raiz de incidentes',
          'Pesquisar eventos recentes e logs syslog da rede',
          'Inspecionar topologia, túneis WireGuard e servidores remotos',
          'Executar testes ativos (ping, traceroute, portas) conforme política configurada',
          'Acessar equipamentos via plugins com confirmação explícita ou modo automático por termo',
          'Gerar gráficos de série temporal diretamente no chat',
        ];
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            if (settings != null) ...<Widget>[
              Wrap(
                spacing: 8,
                runSpacing: 6,
                children: <Widget>[
                  SemanticChip(
                    label: 'Ações: ${settings.actionMode.label}',
                    tone: SemanticTone.ai,
                    compact: true,
                  ),
                  SemanticChip(
                    label: 'Estilo: ${settings.responseStyle.label}',
                    tone: SemanticTone.info,
                    compact: true,
                  ),
                ],
              ),
              const SizedBox(height: 12),
            ],
            for (final bullet in capabilities)
              Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Icon(
                      Icons.check_circle_outline,
                      size: 16,
                      color: context.semanticColors.ai,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(bullet, style: context.textTheme.bodySmall),
                    ),
                  ],
                ),
              ),
          ],
        );
      },
    );
  }

  void _openMobileConversationsSheet() {
    showAdaptiveFormSheet<void>(
      context: context,
      title: 'Conversas',
      maxWidth: 480,
      builder: (sheetCtx) => SizedBox(
        height: math.min(440.0, MediaQuery.sizeOf(sheetCtx).height * 0.62),
        child: _buildSidebarContent(
          onSelect: () => Navigator.of(sheetCtx).pop(),
        ),
      ),
    );
  }

  Widget _buildSidebarContent({VoidCallback? onSelect}) {
    final convs = ref.watch(aiConversationsProvider);
    final settings = ref.watch(aiSettingsProvider).settings;
    final scheme = context.colorScheme;

    final filtered = _search.isEmpty
        ? convs.summaries
        : convs.summaries
              .where(
                (c) => c.title.toLowerCase().contains(_search.toLowerCase()),
              )
              .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              FilledButton.icon(
                onPressed: () {
                  ref.read(aiChatProvider.notifier).newConversation();
                  onSelect?.call();
                },
                icon: const Icon(Icons.add_comment_outlined, size: 18),
                label: const Text('Nova conversa'),
              ),
              const SizedBox(height: 10),
              TextField(
                controller: _searchCtrl,
                onChanged: (v) => setState(() => _search = v.trim()),
                decoration: const InputDecoration(
                  hintText: 'Filtrar conversas...',
                  prefixIcon: Icon(Icons.search, size: 18),
                  isDense: true,
                ),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: convs.loading && convs.summaries.isEmpty
              ? const LoadingStateView(message: 'Carregando conversas...')
              : filtered.isEmpty
              ? const EmptyStateView(
                  title: 'Nenhuma conversa',
                  icon: Icons.chat_bubble_outline,
                )
              : ListView.separated(
                  padding: const EdgeInsets.all(8),
                  itemCount: filtered.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 2),
                  itemBuilder: (context, index) {
                    final item = filtered[index];
                    final selected = convs.activeId == item.id;
                    return ListTile(
                      dense: true,
                      selected: selected,
                      selectedTileColor: scheme.primaryContainer.withValues(
                        alpha: 0.45,
                      ),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(10),
                      ),
                      title: Text(
                        item.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontWeight: selected
                              ? FontWeight.w700
                              : FontWeight.w500,
                        ),
                      ),
                      subtitle: Text(
                        '${countNoun(item.messageCount, 'msg', 'msgs')} · ${formatRelativeTime(item.updatedAt)}',
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                      onTap: () async {
                        await ref
                            .read(aiChatProvider.notifier)
                            .openConversation(item.id);
                        onSelect?.call();
                      },
                      trailing: PopupMenuButton<String>(
                        iconSize: 18,
                        onSelected: (action) {
                          if (action == 'rename') {
                            _renameConversation(item);
                          } else if (action == 'delete') {
                            _deleteConversation(item);
                          }
                        },
                        itemBuilder: (_) => const <PopupMenuEntry<String>>[
                          PopupMenuItem<String>(
                            value: 'rename',
                            child: Text('Renomear'),
                          ),
                          PopupMenuItem<String>(
                            value: 'delete',
                            child: Text('Excluir'),
                          ),
                        ],
                      ),
                    );
                  },
                ),
        ),
        if (settings != null) ...<Widget>[
          const Divider(height: 1),
          Padding(
            padding: const EdgeInsets.all(12),
            child: Row(
              children: <Widget>[
                Icon(
                  Icons.memory_outlined,
                  size: 16,
                  color: context.semanticColors.ai,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    '${settings.activeDriver} · ${settings.currentModelLabel}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.labelSmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(aiSettingsProvider).settings;
    final scheme = context.colorScheme;
    final screenWidth = MediaQuery.sizeOf(context).width;
    final isWide = screenWidth >= 960;
    final isCompact = screenWidth < 600;

    return Row(
      children: <Widget>[
        if (isWide)
          Container(
            width: 300,
            decoration: BoxDecoration(
              color: scheme.surface,
              border: Border(right: BorderSide(color: scheme.outlineVariant)),
            ),
            child: _buildSidebarContent(),
          ),
        Expanded(
          child: Column(
            children: <Widget>[
              Container(
                padding: EdgeInsets.symmetric(
                  horizontal: isCompact ? 8 : 14,
                  vertical: isCompact ? 6 : 10,
                ),
                decoration: BoxDecoration(
                  color: scheme.surface,
                  border: Border(
                    bottom: BorderSide(color: scheme.outlineVariant),
                  ),
                ),
                child: Row(
                  children: <Widget>[
                    if (!isWide)
                      IconButton(
                        tooltip: 'Conversas',
                        icon: const Icon(Icons.forum_outlined),
                        onPressed: _openMobileConversationsSheet,
                      ),
                    Container(
                      width: 34,
                      height: 34,
                      decoration: BoxDecoration(
                        color: context.semanticColors.aiContainer,
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: Icon(
                        Icons.smart_toy_outlined,
                        size: 20,
                        color: context.semanticColors.onAiContainer,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'Assistente IA',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          if (settings != null)
                            Text(
                              settings.enabled
                                  ? settings.currentModelLabel
                                  : 'Desativado nas configurações',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: context.textTheme.labelSmall?.copyWith(
                                color: scheme.onSurfaceVariant,
                              ),
                            ),
                        ],
                      ),
                    ),
                    if (!isWide)
                      IconButton(
                        tooltip: 'Nova conversa',
                        icon: const Icon(Icons.add_comment_outlined),
                        onPressed: () =>
                            ref.read(aiChatProvider.notifier).newConversation(),
                      ),
                    if (isCompact)
                      PopupMenuButton<String>(
                        tooltip: 'Mais opções',
                        onSelected: (action) {
                          if (action == 'capabilities') {
                            _showCapabilitiesSheet(settings);
                          } else if (action == 'benchmark') {
                            context.go('/ai-benchmark');
                          } else if (action == 'settings') {
                            context.go('/settings?tab=ia');
                          }
                        },
                        itemBuilder: (_) => const <PopupMenuEntry<String>>[
                          PopupMenuItem<String>(
                            value: 'capabilities',
                            child: ListTile(
                              dense: true,
                              leading: Icon(Icons.info_outline),
                              title: Text('Capacidades da IA'),
                            ),
                          ),
                          PopupMenuItem<String>(
                            value: 'benchmark',
                            child: ListTile(
                              dense: true,
                              leading: Icon(Icons.speed_outlined),
                              title: Text('Benchmark IA'),
                            ),
                          ),
                          PopupMenuItem<String>(
                            value: 'settings',
                            child: ListTile(
                              dense: true,
                              leading: Icon(Icons.settings_outlined),
                              title: Text('Configurações de IA'),
                            ),
                          ),
                        ],
                      )
                    else ...<Widget>[
                      IconButton(
                        tooltip: 'Capacidades da IA',
                        icon: const Icon(Icons.info_outline),
                        onPressed: () => _showCapabilitiesSheet(settings),
                      ),
                      IconButton(
                        tooltip: 'Benchmark IA',
                        icon: const Icon(Icons.speed_outlined),
                        onPressed: () => context.go('/ai-benchmark'),
                      ),
                      IconButton(
                        tooltip: 'Configurações de IA',
                        icon: const Icon(Icons.settings_outlined),
                        onPressed: () => context.go('/settings?tab=ia'),
                      ),
                    ],
                  ],
                ),
              ),
              if (settings != null && !settings.enabled)
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 10, 16, 0),
                  child: NestedPanel(
                    tone: SemanticTone.warning,
                    child: Wrap(
                      spacing: 8,
                      runSpacing: 6,
                      alignment: WrapAlignment.spaceBetween,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        const Text(
                          'O Assistente IA está desativado. Ative-o nas configurações para conversar.',
                        ),
                        TextButton(
                          onPressed: () => context.go('/settings?tab=ia'),
                          child: const Text('Configurar'),
                        ),
                      ],
                    ),
                  ),
                ),
              const Expanded(child: AiChatThread()),
            ],
          ),
        ),
      ],
    );
  }
}

class _RenameConversationDialog extends StatefulWidget {
  const _RenameConversationDialog({required this.initialTitle});

  final String initialTitle;

  @override
  State<_RenameConversationDialog> createState() =>
      _RenameConversationDialogState();
}

class _RenameConversationDialogState extends State<_RenameConversationDialog> {
  late final TextEditingController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = TextEditingController(text: widget.initialTitle);
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Renomear conversa'),
      content: TextField(
        controller: _ctrl,
        autofocus: true,
        decoration: const InputDecoration(labelText: 'Título'),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancelar'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(_ctrl.text.trim()),
          child: const Text('Salvar'),
        ),
      ],
    );
  }
}

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_chat_thread.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Overlay invisível montado no shell que registra o `AiChatDrawer` no
/// `aiDrawerBuilderProvider` e inicializa a lista de Aplicativos de plugin no menu.
class AiAndAppsShellBootstrapOverlay extends ConsumerStatefulWidget {
  const AiAndAppsShellBootstrapOverlay({super.key});

  @override
  ConsumerState<AiAndAppsShellBootstrapOverlay> createState() =>
      _AiAndAppsShellBootstrapOverlayState();
}

class _AiAndAppsShellBootstrapOverlayState
    extends ConsumerState<AiAndAppsShellBootstrapOverlay> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ref.read(aiDrawerBuilderProvider.notifier).state = (_) =>
          const AiChatDrawer();
      ref.read(aiSettingsProvider.notifier).load();
    });
  }

  @override
  Widget build(BuildContext context) => const SizedBox.shrink();
}

/// Drawer global do Assistente IA (`AiChatDrawer.vue`), alternando entre Chat e Histórico.
class AiChatDrawer extends ConsumerStatefulWidget {
  const AiChatDrawer({super.key});

  @override
  ConsumerState<AiChatDrawer> createState() => _AiChatDrawerState();
}

class _AiChatDrawerState extends ConsumerState<AiChatDrawer> {
  bool _showHistory = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ref.read(aiConversationsProvider.notifier).fetchAll();
    });
  }

  void _closeDrawer({bool force = false}) {
    if (!force && _showHistory) {
      setState(() => _showHistory = false);
      return;
    }
    if (_showHistory) {
      setState(() => _showHistory = false);
    }
    ref.read(aiDrawerOpenProvider.notifier).state = false;
    final scaffold = Scaffold.maybeOf(context);
    if (scaffold?.isEndDrawerOpen == true) {
      Navigator.of(context).pop();
    }
  }

  @override
  Widget build(BuildContext context) {
    final convs = ref.watch(aiConversationsProvider);
    final scheme = context.colorScheme;

    return SafeArea(
      child: Column(
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 8, 8, 8),
            child: Row(
              children: <Widget>[
                Container(
                  width: 32,
                  height: 32,
                  decoration: BoxDecoration(
                    color: context.semanticColors.aiContainer,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Icon(
                    Icons.smart_toy_outlined,
                    size: 18,
                    color: context.semanticColors.onAiContainer,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    _showHistory ? 'Conversas' : 'Assistente IA',
                    style: context.textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
                IconButton(
                  tooltip: _showHistory ? 'Voltar ao chat' : 'Histórico',
                  icon: Icon(
                    _showHistory ? Icons.chat_outlined : Icons.history,
                    size: 20,
                  ),
                  onPressed: () {
                    setState(() => _showHistory = !_showHistory);
                    if (_showHistory) {
                      ref.read(aiConversationsProvider.notifier).fetchAll();
                    }
                  },
                ),
                IconButton(
                  tooltip: 'Nova conversa',
                  icon: const Icon(Icons.add_comment_outlined, size: 20),
                  onPressed: () {
                    ref.read(aiChatProvider.notifier).newConversation();
                    setState(() => _showHistory = false);
                  },
                ),
                IconButton(
                  tooltip: 'Abrir em tela cheia',
                  icon: const Icon(Icons.open_in_full, size: 18),
                  onPressed: () {
                    _closeDrawer(force: true);
                    context.go('/ai-chat');
                  },
                ),
                IconButton(
                  tooltip: 'Fechar',
                  icon: const Icon(Icons.close, size: 20),
                  onPressed: _closeDrawer,
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          Expanded(
            child: _showHistory
                ? convs.loading && convs.summaries.isEmpty
                      ? const LoadingStateView(
                          message: 'Carregando histórico...',
                        )
                      : convs.summaries.isEmpty
                      ? const EmptyStateView(
                          title: 'Sem conversas salvas',
                          icon: Icons.chat_bubble_outline,
                        )
                      : ListView.separated(
                          padding: const EdgeInsets.all(8),
                          itemCount: convs.summaries.length,
                          separatorBuilder: (_, _) => const Divider(height: 1),
                          itemBuilder: (context, index) {
                            final item = convs.summaries[index];
                            final selected = convs.activeId == item.id;
                            return ListTile(
                              selected: selected,
                              shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(10),
                              ),
                              title: Text(
                                item.title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                              subtitle: Text(
                                '${countNoun(item.messageCount, 'mensagem', 'mensagens')} · ${formatRelativeTime(item.updatedAt)}',
                                style: context.textTheme.bodySmall?.copyWith(
                                  color: scheme.onSurfaceVariant,
                                ),
                              ),
                              onTap: () async {
                                await ref
                                    .read(aiChatProvider.notifier)
                                    .openConversation(item.id);
                                if (mounted) {
                                  setState(() => _showHistory = false);
                                }
                              },
                              trailing: IconButton(
                                tooltip: 'Excluir',
                                icon: Icon(
                                  Icons.delete_outline,
                                  size: 18,
                                  color: scheme.error,
                                ),
                                onPressed: () async {
                                  final isActive =
                                      ref
                                          .read(aiConversationsProvider)
                                          .activeId ==
                                      item.id;
                                  await ref
                                      .read(aiConversationsProvider.notifier)
                                      .remove(item.id);
                                  if (isActive && mounted) {
                                    ref
                                        .read(aiChatProvider.notifier)
                                        .newConversation();
                                  }
                                },
                              ),
                            );
                          },
                        )
                : const AiChatThread(compact: true),
          ),
        ],
      ),
    );
  }
}

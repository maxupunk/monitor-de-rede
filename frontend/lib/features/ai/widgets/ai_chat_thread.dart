import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_auto_accept_banner.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_chat_composer.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_message_bubble.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Thread compartilhada entre `AiChatPage` (tela cheia) e `AiChatDrawer` (painel lateral).
class AiChatThread extends ConsumerStatefulWidget {
  const AiChatThread({
    super.key,
    this.compact = false,
    this.screenSuggestions = const <AiSuggestion>[],
  });

  final bool compact;
  final List<AiSuggestion> screenSuggestions;

  @override
  ConsumerState<AiChatThread> createState() => _AiChatThreadState();
}

class _AiChatThreadState extends ConsumerState<AiChatThread> {
  final _scrollCtrl = ScrollController();
  bool _pinnedToBottom = true;

  @override
  void initState() {
    super.initState();
    _scrollCtrl.addListener(_onScroll);
  }

  @override
  void dispose() {
    _scrollCtrl.removeListener(_onScroll);
    _scrollCtrl.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (!_scrollCtrl.hasClients) return;
    final max = _scrollCtrl.position.maxScrollExtent;
    final current = _scrollCtrl.position.pixels;
    final atBottom = (max - current) < 64;
    if (atBottom != _pinnedToBottom) {
      setState(() => _pinnedToBottom = atBottom);
    }
  }

  void _scrollToBottom() {
    if (!_scrollCtrl.hasClients) return;
    _scrollCtrl.animateTo(
      _scrollCtrl.position.maxScrollExtent,
      duration: const Duration(milliseconds: 220),
      curve: Curves.easeOut,
    );
  }

  @override
  Widget build(BuildContext context) {
    final chat = ref.watch(aiChatProvider);
    final suggestions = mergeSuggestions(
      widget.screenSuggestions,
      generalAiSuggestions,
      limit: widget.compact ? 4 : 6,
    );

    // Auto-scroll enquanto recebe tokens se o usuário estiver no fim.
    ref.listen<AiChatState>(aiChatProvider, (prev, next) {
      if (_pinnedToBottom &&
          (next.messages.length != prev?.messages.length || next.isStreaming)) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && _scrollCtrl.hasClients) {
            _scrollCtrl.jumpTo(_scrollCtrl.position.maxScrollExtent);
          }
        });
      }
    });

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(
          child: Stack(
            children: <Widget>[
              if (chat.messages.isEmpty)
                ListView(
                  padding: const EdgeInsets.all(16),
                  children: <Widget>[
                    const AiAutoAcceptBanner(),
                    Center(
                      child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 680),
                        child: Column(
                          children: <Widget>[
                            Container(
                              width: 56,
                              height: 56,
                              decoration: BoxDecoration(
                                color: context.semanticColors.aiContainer,
                                borderRadius: BorderRadius.circular(16),
                              ),
                              child: Icon(
                                Icons.smart_toy_outlined,
                                size: 30,
                                color: context.semanticColors.onAiContainer,
                              ),
                            ),
                            const SizedBox(height: 12),
                            Text(
                              'Como posso ajudar com a sua rede?',
                              textAlign: TextAlign.center,
                              style: context.textTheme.titleMedium?.copyWith(
                                fontWeight: FontWeight.w800,
                              ),
                            ),
                            const SizedBox(height: 4),
                            Text(
                              'Consulto alertas, métricas, logs, topologia, VPN e plugins com dados reais da sua central.',
                              textAlign: TextAlign.center,
                              style: context.textTheme.bodySmall?.copyWith(
                                color: context.colorScheme.onSurfaceVariant,
                              ),
                            ),
                            const SizedBox(height: 16),
                            ResponsiveGrid(
                              xs: 1,
                              sm: widget.compact ? 1 : 2,
                              md: widget.compact ? 1 : 2,
                              lg: widget.compact ? 1 : 2,
                              xl: widget.compact ? 1 : 2,
                              children: suggestions
                                  .map((item) {
                                    return ChoiceCard(
                                      title: item.title,
                                      subtitle: item.prompt,
                                      icon: item.icon,
                                      tone: item.tone,
                                      onTap: () => ref
                                          .read(aiChatProvider.notifier)
                                          .sendMessage(item.prompt),
                                    );
                                  })
                                  .toList(growable: false),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                )
              else
                ListView.separated(
                  controller: _scrollCtrl,
                  padding: const EdgeInsets.all(16),
                  itemCount: chat.messages.length + 1,
                  separatorBuilder: (_, _) => const SizedBox(height: 12),
                  itemBuilder: (context, index) {
                    if (index == 0) {
                      return const AiAutoAcceptBanner();
                    }
                    final msg = chat.messages[index - 1];
                    return AiMessageBubble(
                      message: msg,
                      isLatest: index == chat.messages.length,
                      isStreaming: chat.isStreaming,
                      onAnswerQuestion: (opt) =>
                          ref.read(aiChatProvider.notifier).sendMessage(opt),
                    );
                  },
                ),
              if (!_pinnedToBottom && chat.messages.isNotEmpty)
                Positioned(
                  right: 16,
                  bottom: 12,
                  child: FloatingActionButton.small(
                    heroTag: 'ai-scroll-bottom-${widget.compact}',
                    tooltip: 'Ir para a mensagem mais recente',
                    onPressed: _scrollToBottom,
                    child: const Icon(Icons.arrow_downward),
                  ),
                ),
            ],
          ),
        ),
        AiChatComposer(compact: widget.compact),
      ],
    );
  }
}

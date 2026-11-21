import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_tool_card.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Bolha de mensagem do chat da IA com Markdown, interceptação de links internos,
/// métricas de geração, avaliação (`AiRating`), compactação e botão Desfazer.
class AiMessageBubble extends ConsumerStatefulWidget {
  const AiMessageBubble({
    super.key,
    required this.message,
    required this.isLatest,
    required this.isStreaming,
    required this.onAnswerQuestion,
  });

  final AiDisplayMessage message;
  final bool isLatest;
  final bool isStreaming;
  final ValueChanged<String> onAnswerQuestion;

  @override
  ConsumerState<AiMessageBubble> createState() => _AiMessageBubbleState();
}

class _AiMessageBubbleState extends ConsumerState<AiMessageBubble> {
  bool _showCompactionSummary = false;

  Future<void> _handleLinkTap(String? href) async {
    if (href == null || href.trim().isEmpty) return;
    final trimmed = href.trim();
    if (trimmed.startsWith('/')) {
      if (mounted) context.go(trimmed);
      return;
    }
    final uri = Uri.tryParse(trimmed);
    if (uri != null && (uri.scheme == 'http' || uri.scheme == 'https')) {
      await launchUrl(uri, mode: LaunchMode.externalApplication);
    }
  }

  Future<void> _openDownRatingSheet() async {
    final msg = widget.message;
    String? selectedReason = msg.rating?.reason;
    final customCtrl = TextEditingController(
      text: selectedReason != null && !aiRatingReasons.contains(selectedReason)
          ? selectedReason
          : '',
    );

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'O que faltou na resposta?',
      subtitle: 'Sua avaliação ajuda a melhorar os modelos e ferramentas.',
      maxWidth: 460,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: aiRatingReasons
                      .map((reason) {
                        return ChoiceChip(
                          label: Text(reason),
                          selected: selectedReason == reason,
                          onSelected: (selected) {
                            setModalState(() {
                              selectedReason = selected ? reason : null;
                              if (selected) customCtrl.clear();
                            });
                          },
                        );
                      })
                      .toList(growable: false),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: customCtrl,
                  maxLength: 200,
                  maxLines: 2,
                  decoration: const InputDecoration(
                    labelText: 'Outro motivo (opcional)',
                    hintText: 'Descreva brevemente o erro...',
                  ),
                  onChanged: (value) {
                    if (value.trim().isNotEmpty && selectedReason != null) {
                      setModalState(() => selectedReason = null);
                    }
                  },
                ),
                const SizedBox(height: 12),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: <Widget>[
                    if (msg.rating?.value == 'down')
                      TextButton(
                        onPressed: () {
                          ref
                              .read(aiChatProvider.notifier)
                              .rateMessage(msg.id, 'down', msg.rating?.reason);
                          Navigator.of(sheetCtx).pop();
                        },
                        child: const Text('Desfazer'),
                      ),
                    const Spacer(),
                    TextButton(
                      onPressed: () => Navigator.of(sheetCtx).pop(),
                      child: const Text('Cancelar'),
                    ),
                    const SizedBox(width: 8),
                    FilledButton(
                      onPressed: () {
                        final reason = customCtrl.text.trim().isNotEmpty
                            ? customCtrl.text.trim()
                            : selectedReason;
                        ref
                            .read(aiChatProvider.notifier)
                            .rateMessage(msg.id, 'down', reason);
                        Navigator.of(sheetCtx).pop();
                      },
                      child: const Text('Enviar avaliação'),
                    ),
                  ],
                ),
              ],
            );
          },
        );
      },
    );
    customCtrl.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final msg = widget.message;
    final scheme = context.colorScheme;
    final isUser = msg.role == 'user';

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Align(
          alignment: isUser ? Alignment.centerRight : Alignment.centerLeft,
          child: ConstrainedBox(
            constraints: BoxConstraints(
              maxWidth: isUser ? 620 : double.infinity,
            ),
            child: isUser
                ? _buildUserBubble(context, msg)
                : _buildAssistantBubble(context, msg),
          ),
        ),
        if (msg.compaction != null) ...<Widget>[
          const SizedBox(height: 12),
          _buildCompactionDivider(context, msg.compaction!, scheme),
        ],
      ],
    );
  }

  Widget _buildUserBubble(BuildContext context, AiDisplayMessage msg) {
    final scheme = context.colorScheme;
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 10, 10, 10),
      decoration: BoxDecoration(
        color: scheme.primaryContainer,
        borderRadius: AppTheme.borderRadius,
        border: Border.all(color: scheme.primary.withValues(alpha: 0.3)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          if (msg.screen != null || msg.mentions.isNotEmpty) ...<Widget>[
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: <Widget>[
                if (msg.screen != null)
                  SemanticChip(
                    label: 'Tela: ${msg.screen!.label}',
                    tone: SemanticTone.ai,
                    compact: true,
                  ),
                for (final m in msg.mentions)
                  SemanticChip(
                    label: '@${m.label}',
                    tone: m.kind.tone,
                    icon: m.kind.icon,
                    compact: true,
                  ),
              ],
            ),
            const SizedBox(height: 6),
          ],
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Flexible(
                child: SelectableText(
                  msg.content,
                  style: context.textTheme.bodyMedium?.copyWith(
                    color: scheme.onPrimaryContainer,
                    height: 1.4,
                  ),
                ),
              ),
              IconButton(
                tooltip: 'Desfazer até esta pergunta',
                visualDensity: VisualDensity.compact,
                icon: Icon(
                  Icons.undo,
                  size: 16,
                  color: scheme.onPrimaryContainer,
                ),
                onPressed: () =>
                    ref.read(aiChatProvider.notifier).rewind(msg.id),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildAssistantBubble(BuildContext context, AiDisplayMessage msg) {
    final scheme = context.colorScheme;
    final aiColors = context.semanticColors.resolve(SemanticTone.ai, scheme);
    final tokRate = tokensPerSecond(msg.usage);
    final modelShort = shortModelName(msg.usage?.model);

    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: scheme.surface,
        borderRadius: AppTheme.borderRadius,
        border: Border.all(color: scheme.outlineVariant),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Container(
                width: 26,
                height: 26,
                decoration: BoxDecoration(
                  color: aiColors.container,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(
                  Icons.smart_toy_outlined,
                  size: 16,
                  color: aiColors.onContainer,
                ),
              ),
              const SizedBox(width: 8),
              Text(
                'Assistente IA',
                style: context.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (modelShort != null) ...<Widget>[
                const SizedBox(width: 8),
                SemanticChip(
                  label: modelShort,
                  tone: SemanticTone.ai,
                  compact: true,
                ),
              ],
              if (msg.isStreaming) ...<Widget>[
                const SizedBox(width: 8),
                const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
              ],
            ],
          ),
          if (msg.notices.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            for (final notice in msg.notices) ...<Widget>[
              NestedPanel(
                tone: SemanticTone.info,
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 8,
                ),
                child: Row(
                  children: <Widget>[
                    const Expanded(child: SizedBox()),
                    Expanded(
                      flex: 20,
                      child: Text(
                        notice.message,
                        style: context.textTheme.bodySmall,
                      ),
                    ),
                    if (notice.settings)
                      TextButton(
                        onPressed: () => context.go('/settings?tab=ia'),
                        child: const Text('Configurar IA'),
                      ),
                  ],
                ),
              ),
              const SizedBox(height: 6),
            ],
          ],
          if (msg.toolCalls.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            AiToolActivityList(
              messageId: msg.id,
              toolCalls: msg.toolCalls,
              isLatestMessage: widget.isLatest,
              isStreaming: widget.isStreaming,
              onAnswerQuestion: widget.onAnswerQuestion,
            ),
          ],
          if (msg.content.isNotEmpty) ...<Widget>[
            const SizedBox(height: 8),
            MarkdownBody(
              data: msg.content,
              selectable: true,
              onTapLink: (_, href, _) => _handleLinkTap(href),
              styleSheet: MarkdownStyleSheet.fromTheme(Theme.of(context))
                  .copyWith(
                    p: context.textTheme.bodyMedium?.copyWith(
                      height: 1.48,
                      color: scheme.onSurface,
                    ),
                    code: TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 12.5,
                      color: scheme.onSurface,
                      backgroundColor: scheme.surfaceContainerHigh,
                    ),
                    codeblockDecoration: BoxDecoration(
                      color: scheme.surfaceContainerLowest,
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(color: scheme.outlineVariant),
                    ),
                  ),
            ),
          ],
          if (msg.error != null && msg.error!.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            NestedPanel(
              tone: SemanticTone.error,
              child: Text(
                msg.error!,
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.semanticColors
                      .resolve(SemanticTone.error, scheme)
                      .onContainer,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ],
          if (!msg.isStreaming &&
              (msg.content.isNotEmpty || msg.usage != null)) ...<Widget>[
            const SizedBox(height: 10),
            const Divider(height: 1),
            const SizedBox(height: 6),
            Wrap(
              spacing: 10,
              runSpacing: 4,
              crossAxisAlignment: WrapCrossAlignment.center,
              alignment: WrapAlignment.spaceBetween,
              children: <Widget>[
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    IconButton(
                      tooltip: 'Boa resposta',
                      visualDensity: VisualDensity.compact,
                      icon: Icon(
                        msg.rating?.value == 'up'
                            ? Icons.thumb_up
                            : Icons.thumb_up_outlined,
                        size: 16,
                        color: msg.rating?.value == 'up'
                            ? context.semanticColors.success
                            : scheme.onSurfaceVariant,
                      ),
                      onPressed: () => ref
                          .read(aiChatProvider.notifier)
                          .rateMessage(msg.id, 'up'),
                    ),
                    IconButton(
                      tooltip: 'Resposta ruim',
                      visualDensity: VisualDensity.compact,
                      icon: Icon(
                        msg.rating?.value == 'down'
                            ? Icons.thumb_down
                            : Icons.thumb_down_outlined,
                        size: 16,
                        color: msg.rating?.value == 'down'
                            ? context.semanticColors.error
                            : scheme.onSurfaceVariant,
                      ),
                      onPressed: _openDownRatingSheet,
                    ),
                    if (msg.rating?.reason != null &&
                        msg.rating!.reason!.isNotEmpty)
                      Text(
                        msg.rating!.reason!,
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                  ],
                ),
                if (msg.usage != null)
                  Wrap(
                    spacing: 10,
                    children: <Widget>[
                      if (tokRate != null)
                        Text(
                          formatTokenRate(tokRate),
                          style: context.textTheme.labelSmall?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      if (msg.usage!.durationMs != null)
                        Text(
                          formatElapsedMs(msg.usage!.durationMs),
                          style: context.textTheme.labelSmall?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      Text(
                        '${formatInteger(msg.usage!.promptTokens + msg.usage!.completionTokens)} tok',
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildCompactionDivider(
    BuildContext context,
    AiCompaction compaction,
    ColorScheme scheme,
  ) {
    return NestedPanel(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          InkWell(
            onTap: () => setState(
              () => _showCompactionSummary = !_showCompactionSummary,
            ),
            child: Row(
              children: <Widget>[
                Icon(Icons.compress, size: 16, color: scheme.primary),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Contexto compactado: ${formatInteger(compaction.tokensBefore)} → ${formatInteger(compaction.tokensAfter)} tokens',
                    style: context.textTheme.labelMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
                Icon(
                  _showCompactionSummary
                      ? Icons.expand_less
                      : Icons.expand_more,
                  size: 18,
                ),
              ],
            ),
          ),
          if (_showCompactionSummary) ...<Widget>[
            const Divider(height: 12),
            Text(
              compaction.summary,
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

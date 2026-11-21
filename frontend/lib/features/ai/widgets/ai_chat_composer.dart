import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Compositor mobile-first do chat IA:
/// - Autocomplete `@mentions` com debounce de 150ms (`GET /api/ai/mentions?q=...`).
/// - Chip de contexto de tela (`Tela: <label>`) dispensável/restaurável.
/// - Desambiguação de tela ("Sobre qual tela é a pergunta?").
/// - Medidor da janela de contexto + opção "Compactar na próxima pergunta".
class AiChatComposer extends ConsumerStatefulWidget {
  const AiChatComposer({super.key, this.compact = false});

  final bool compact;

  @override
  ConsumerState<AiChatComposer> createState() => _AiChatComposerState();
}

class _AiChatComposerState extends ConsumerState<AiChatComposer> {
  final _textCtrl = TextEditingController();
  final _focusNode = FocusNode();
  List<AiMention> _mentions = <AiMention>[];
  List<AiMention> _suggestions = const <AiMention>[];
  MentionQuery? _activeQuery;
  Timer? _debounce;
  int _appliedDraftRevision = 0;

  // Estado da desambiguação de tela ambígua.
  String? _pendingDisambiguationText;
  List<AskedScreen>? _disambiguationOptions;

  @override
  void dispose() {
    _debounce?.cancel();
    _textCtrl.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  void _syncMentionsAndQuery() {
    final text = _textCtrl.text;
    final kept = mentionsInText(text, _mentions);
    if (kept.length != _mentions.length) {
      setState(() => _mentions = kept);
    }
    final caret = _textCtrl.selection.baseOffset;
    final query = findMentionQuery(text, caret < 0 ? text.length : caret);
    if (query == null) {
      if (_activeQuery != null || _suggestions.isNotEmpty) {
        setState(() {
          _activeQuery = null;
          _suggestions = const <AiMention>[];
        });
      }
      return;
    }
    setState(() => _activeQuery = query);
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 150), () async {
      try {
        final list = await ref
            .read(aiRepositoryProvider)
            .fetchMentions(query.query);
        if (mounted && _activeQuery?.query == query.query) {
          setState(() => _suggestions = list);
        }
      } catch (_) {
        if (mounted) setState(() => _suggestions = const <AiMention>[]);
      }
    });
  }

  void _pickMention(AiMention mention) {
    final query = _activeQuery;
    if (query == null) return;
    final caret = _textCtrl.selection.baseOffset;
    final result = insertMention(
      _textCtrl.text,
      query,
      caret < 0 ? _textCtrl.text.length : caret,
      mention.label,
    );
    _textCtrl.value = TextEditingValue(
      text: result.text,
      selection: TextSelection.collapsed(offset: result.caret),
    );
    setState(() {
      _mentions = addMention(_mentions, mention);
      _activeQuery = null;
      _suggestions = const <AiMention>[];
    });
    _focusNode.requestFocus();
  }

  void _insertAtSymbol() {
    final text = _textCtrl.text;
    final caret = _textCtrl.selection.baseOffset.clamp(0, text.length);
    final needsSpace = caret > 0 && !RegExp(r'\s').hasMatch(text[caret - 1]);
    final insertion = needsSpace ? ' @' : '@';
    final next = text.substring(0, caret) + insertion + text.substring(caret);
    _textCtrl.value = TextEditingValue(
      text: next,
      selection: TextSelection.collapsed(offset: caret + insertion.length),
    );
    _focusNode.requestFocus();
    _syncMentionsAndQuery();
  }

  Future<void> _submit({AskedScreen? explicitScreen}) async {
    final text = (_pendingDisambiguationText ?? _textCtrl.text).trim();
    if (text.isEmpty) return;

    final chat = ref.read(aiChatProvider);
    final screenState = ref.read(aiScreenProvider);

    if (explicitScreen == null &&
        screenState.isAttached &&
        screenState.activeScreen != null) {
      final options = screenToConfirm(
        currentScreen: screenState.activeScreen!,
        messages: chat.messages,
        text: text,
        hasMentions: _mentions.isNotEmpty,
      );
      if (options != null) {
        setState(() {
          _pendingDisambiguationText = text;
          _disambiguationOptions = options;
        });
        return;
      }
    }

    final mentionsToSend = List<AiMention>.from(_mentions);
    _textCtrl.clear();
    setState(() {
      _mentions = <AiMention>[];
      _activeQuery = null;
      _suggestions = const <AiMention>[];
      _pendingDisambiguationText = null;
      _disambiguationOptions = null;
    });

    await ref
        .read(aiChatProvider.notifier)
        .sendMessage(
          text,
          mentions: mentionsToSend,
          chosenScreen: explicitScreen,
        );
  }

  void _showContextGaugeSheet(
    AiContextUsage usage,
    bool canCompact,
    bool compactNext,
  ) {
    showAdaptiveFormSheet<void>(
      context: context,
      title: 'Janela de contexto',
      subtitle: usage.reported
          ? 'Medida informada pelo provedor'
          : 'Estimativa pelo modelo selecionado',
      maxWidth: 420,
      builder: (sheetCtx) {
        final tone = usage.ratio >= autoCompactRatio
            ? SemanticTone.error
            : (usage.ratio >= 0.7
                  ? SemanticTone.warning
                  : SemanticTone.primary);
        final colors = sheetCtx.semanticColors.resolve(
          tone,
          sheetCtx.colorScheme,
        );
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            LinearProgressIndicator(
              value: usage.ratio,
              minHeight: 10,
              borderRadius: BorderRadius.circular(8),
              color: colors.foreground,
              backgroundColor: colors.container,
            ),
            const SizedBox(height: 10),
            Text(
              '${formatInteger(usage.used)} / ${formatInteger(usage.window)} tokens (${formatPercent(usage.ratio * 100, digits: 0)})',
              style: sheetCtx.textTheme.bodyMedium?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 12),
            if (canCompact)
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Compactar na próxima pergunta'),
                subtitle: const Text(
                  'Resume mensagens antigas para liberar contexto.',
                ),
                value: compactNext,
                onChanged: (_) {
                  ref.read(aiChatProvider.notifier).toggleCompactNext();
                  Navigator.of(sheetCtx).pop();
                },
              ),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final chat = ref.watch(aiChatProvider);
    final screen = ref.watch(aiScreenProvider);
    final scheme = context.colorScheme;

    // Restaura rascunho quando o usuário clicou em "Desfazer" (`rewind`).
    if (chat.draftRevision != _appliedDraftRevision) {
      _appliedDraftRevision = chat.draftRevision;
      if (chat.draftText != null) {
        _textCtrl.text = chat.draftText!;
        _textCtrl.selection = TextSelection.collapsed(
          offset: _textCtrl.text.length,
        );
        _mentions = List<AiMention>.from(chat.draftMentions);
      }
    }

    final usage = contextUsage(chat.messages);
    final canCompact = chat.messages.length >= 3;

    return SafeArea(
      top: false,
      child: Container(
        padding: EdgeInsets.fromLTRB(
          12,
          8,
          12,
          MediaQuery.viewInsetsOf(context).bottom > 0 ? 8 : 12,
        ),
        decoration: BoxDecoration(
          color: scheme.surface,
          border: Border(top: BorderSide(color: scheme.outlineVariant)),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            // Desambiguação de tela
            if (_disambiguationOptions != null) ...<Widget>[
              NestedPanel(
                tone: SemanticTone.info,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      'Sobre qual tela é a pergunta?',
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: <Widget>[
                        for (final opt in _disambiguationOptions!)
                          FilledButton.tonal(
                            onPressed: () => _submit(explicitScreen: opt),
                            child: Text(opt.label),
                          ),
                        TextButton(
                          onPressed: () => setState(() {
                            _pendingDisambiguationText = null;
                            _disambiguationOptions = null;
                          }),
                          child: const Text('Editar pergunta'),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 8),
            ],

            // Sugestões @mentions
            if (_suggestions.isNotEmpty) ...<Widget>[
              ConstrainedBox(
                constraints: const BoxConstraints(maxHeight: 200),
                child: Material(
                  elevation: 3,
                  borderRadius: AppTheme.borderRadius,
                  color: scheme.surfaceContainerHigh,
                  child: ListView.separated(
                    shrinkWrap: true,
                    itemCount: _suggestions.length,
                    separatorBuilder: (_, _) => const Divider(height: 1),
                    itemBuilder: (context, index) {
                      final item = _suggestions[index];
                      return ListTile(
                        dense: true,
                        leading: Icon(item.kind.icon, size: 18),
                        title: Text(item.label),
                        subtitle: item.detail != null
                            ? Text(item.detail!)
                            : null,
                        trailing: SemanticChip(
                          label: item.kind.label,
                          tone: item.kind.tone,
                          compact: true,
                        ),
                        onTap: () => _pickMention(item),
                      );
                    },
                  ),
                ),
              ),
              const SizedBox(height: 8),
            ],

            // Chips de tela anexada + menções
            if (screen.activeScreen != null ||
                _mentions.isNotEmpty) ...<Widget>[
              Wrap(
                spacing: 6,
                runSpacing: 6,
                children: <Widget>[
                  if (screen.isAttached && screen.activeScreen != null)
                    InputChip(
                      label: Text('Tela: ${screen.activeScreen!.label}'),
                      avatar: const Icon(Icons.layers_outlined, size: 16),
                      onDeleted: () =>
                          ref.read(aiScreenProvider.notifier).dismissCurrent(),
                    )
                  else if (screen.activeScreen != null)
                    ActionChip(
                      label: Text('+ Tela: ${screen.activeScreen!.label}'),
                      onPressed: () =>
                          ref.read(aiScreenProvider.notifier).restoreCurrent(),
                    ),
                  for (final m in _mentions)
                    InputChip(
                      label: Text('@${m.label}'),
                      avatar: Icon(m.kind.icon, size: 15),
                      onDeleted: () {
                        setState(() {
                          _mentions = _mentions
                              .where((x) => !(x.kind == m.kind && x.id == m.id))
                              .toList(growable: false);
                        });
                      },
                    ),
                ],
              ),
              const SizedBox(height: 6),
            ],

            Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: <Widget>[
                IconButton(
                  tooltip: 'Marcar recurso (@)',
                  onPressed: chat.isStreaming ? null : _insertAtSymbol,
                  icon: const Icon(Icons.alternate_email),
                ),
                Expanded(
                  child: TextField(
                    controller: _textCtrl,
                    focusNode: _focusNode,
                    enabled: !chat.isStreaming,
                    minLines: 1,
                    maxLines: widget.compact ? 4 : 6,
                    textInputAction: TextInputAction.send,
                    onChanged: (_) => _syncMentionsAndQuery(),
                    onSubmitted: (_) => _submit(),
                    decoration: const InputDecoration(
                      hintText:
                          'Pergunte sobre alertas, dispositivos, logs... (use @ para marcar)',
                      isDense: true,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                if (chat.isStreaming)
                  FilledButton.tonalIcon(
                    style: FilledButton.styleFrom(
                      backgroundColor: context.semanticColors.errorContainer,
                      foregroundColor: context.semanticColors.onErrorContainer,
                    ),
                    onPressed: () =>
                        ref.read(aiChatProvider.notifier).stopStreaming(),
                    icon: const Icon(Icons.stop, size: 18),
                    label: const Text('Parar'),
                  )
                else
                  FilledButton(
                    onPressed: () => _submit(),
                    child: const Icon(Icons.send, size: 18),
                  ),
              ],
            ),

            if (usage != null || chat.compactNext) ...<Widget>[
              const SizedBox(height: 6),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: <Widget>[
                  if (chat.compactNext)
                    const SemanticChip(
                      label: 'Compactar na próxima pergunta',
                      tone: SemanticTone.info,
                      icon: Icons.compress,
                      compact: true,
                    )
                  else
                    const SizedBox.shrink(),
                  if (usage != null)
                    InkWell(
                      onTap: () => _showContextGaugeSheet(
                        usage,
                        canCompact,
                        chat.compactNext,
                      ),
                      child: SemanticChip(
                        label:
                            'Contexto ${formatPercent(usage.ratio * 100, digits: 0)}',
                        tone: usage.ratio >= autoCompactRatio
                            ? SemanticTone.error
                            : (usage.ratio >= 0.7
                                  ? SemanticTone.warning
                                  : SemanticTone.primary),
                        icon: Icons.memory_outlined,
                        compact: true,
                      ),
                    ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

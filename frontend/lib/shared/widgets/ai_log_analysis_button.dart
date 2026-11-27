import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// "Analisar com a IA" para o cabeçalho de um [CodeViewer] de console: manda
/// o conteúdo inteiro para o Assistente apontar erros, avisos e
/// descontinuações. Some quando o Assistente não está montado.
class AiLogAnalysisButton extends ConsumerWidget {
  const AiLogAnalysisButton({
    super.key,
    required this.source,
    required this.content,
    this.chatPrompt,
  });

  final String source;
  final String content;
  final String? chatPrompt;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final launcher = ref.watch(aiLogAnalysisLauncherProvider);
    if (launcher == null) return const SizedBox.shrink();

    final ai = context.semanticColors.resolve(
      SemanticTone.ai,
      context.colorScheme,
    );
    final hasContent = content.trim().isNotEmpty;

    return Tooltip(
      message: hasContent
          ? 'A IA lê o console e aponta erros, avisos e descontinuações'
          : 'O console ainda está vazio',
      child: TextButton.icon(
        onPressed: hasContent
            ? () => launcher(
                context,
                AiLogAnalysisRequest(
                  source: source,
                  content: content,
                  chatPrompt: chatPrompt,
                ),
              )
            : null,
        icon: const Icon(Icons.auto_awesome, size: 15),
        label: const Text('Analisar com a IA'),
        style: TextButton.styleFrom(
          foregroundColor: ai.foreground,
          visualDensity: VisualDensity.compact,
          padding: const EdgeInsets.symmetric(horizontal: 8),
        ),
      ),
    );
  }
}

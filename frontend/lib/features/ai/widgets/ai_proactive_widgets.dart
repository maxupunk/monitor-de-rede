import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Botão roxo exclusivo de IA (`SemanticTone.ai`) para abrir o chat sobre um recurso.
class AiAskButton extends ConsumerWidget {
  const AiAskButton({
    super.key,
    required this.prompt,
    this.label = 'Perguntar à IA',
    this.contextData = const AiChatContext(),
    this.compact = false,
  });

  final String prompt;
  final String label;
  final AiChatContext contextData;
  final bool compact;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final enabled = ref.watch(aiSettingsProvider).settings?.enabled ?? true;
    if (!enabled) return const SizedBox.shrink();

    final aiColors = context.semanticColors.resolve(
      SemanticTone.ai,
      context.colorScheme,
    );

    return FilledButton.tonalIcon(
      style: FilledButton.styleFrom(
        backgroundColor: aiColors.container,
        foregroundColor: aiColors.onContainer,
        visualDensity: compact ? VisualDensity.compact : VisualDensity.standard,
      ),
      onPressed: () => ref
          .read(aiChatProvider.notifier)
          .askAbout(prompt, context: contextData),
      icon: const Icon(Icons.auto_awesome, size: 17),
      label: Text(label),
    );
  }
}

/// Resumo de incidente gerado pela IA + leitura do Decisor (`AiAlertSummary.vue`).
class AiAlertSummaryCard extends ConsumerStatefulWidget {
  const AiAlertSummaryCard({
    super.key,
    required this.alertId,
    this.initialSummary,
    this.initialTriage,
  });

  final int alertId;
  final AiIncidentSummary? initialSummary;
  final DeciderTriage? initialTriage;

  @override
  ConsumerState<AiAlertSummaryCard> createState() => _AiAlertSummaryCardState();
}

class _AiAlertSummaryCardState extends ConsumerState<AiAlertSummaryCard> {
  bool _generating = false;

  Future<void> _generateNow() async {
    setState(() => _generating = true);
    try {
      await ref
          .read(aiAlertFeedbackProvider.notifier)
          .requestSummary(widget.alertId);
    } finally {
      if (mounted) setState(() => _generating = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final feedback = ref.watch(aiAlertFeedbackProvider);
    final summary = feedback.summaries[widget.alertId] ?? widget.initialSummary;
    final triage = feedback.triages[widget.alertId] ?? widget.initialTriage;

    if (summary == null && triage == null) return const SizedBox.shrink();

    final triageLabel = switch (triage?.classification) {
      'actionable' => 'Acionável',
      'symptom' => 'Sintoma secundário',
      'transient' => 'Transitório',
      _ => triage?.classification ?? '',
    };
    final triageTone = switch (triage?.classification) {
      'actionable' => SemanticTone.error,
      'symptom' => SemanticTone.warning,
      _ => SemanticTone.info,
    };

    if (summary == null && triage?.suppressed == true) {
      return NestedPanel(
        tone: SemanticTone.info,
        child: Row(
          children: <Widget>[
            Expanded(
              child: Text(
                'Triagem do Decisor: $triageLabel (${formatPercent(triage!.confidence * 100, digits: 0)}) — resumo automático suprimido.',
                style: context.textTheme.bodySmall,
              ),
            ),
            const SizedBox(width: 8),
            FilledButton.tonal(
              onPressed: _generating ? null : _generateNow,
              child: _generating
                  ? const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Gerar resumo agora'),
            ),
          ],
        ),
      );
    }

    return NestedPanel(
      tone: SemanticTone.ai,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(
                Icons.auto_awesome,
                size: 16,
                color: context.semanticColors.ai,
              ),
              const SizedBox(width: 6),
              Text(
                'Análise da IA',
                style: context.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const Spacer(),
              if (triage != null)
                SemanticChip(
                  label:
                      '$triageLabel · ${formatPercent(triage.confidence * 100, digits: 0)}',
                  tone: triageTone,
                  compact: true,
                ),
            ],
          ),
          if (summary != null) ...<Widget>[
            const SizedBox(height: 6),
            Text(summary.text, style: context.textTheme.bodySmall),
            if (summary.recommendation != null &&
                summary.recommendation!.isNotEmpty) ...<Widget>[
              const SizedBox(height: 6),
              Text(
                'Recomendação: ${summary.recommendation}',
                style: context.textTheme.bodySmall?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ],
        ],
      ),
    );
  }
}

/// Cartão de digest proativo periódico da IA (`AiDigest`).
class AiDigestCard extends StatelessWidget {
  const AiDigestCard({super.key, required this.digest});

  final AiDigest digest;

  @override
  Widget build(BuildContext context) {
    return SectionCard(
      title: digest.title,
      subtitle:
          digest.period ??
          (digest.createdAt != null ? formatDateTime(digest.createdAt) : null),
      icon: Icons.auto_awesome,
      iconTone: SemanticTone.ai,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(digest.summary, style: context.textTheme.bodyMedium),
          if (digest.highlights.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            for (final item in digest.highlights)
              Padding(
                padding: const EdgeInsets.only(bottom: 4),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Icon(
                      Icons.circle,
                      size: 6,
                      color: context.semanticColors.ai,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(item, style: context.textTheme.bodySmall),
                    ),
                  ],
                ),
              ),
          ],
        ],
      ),
    );
  }
}

/// Chip de sugestão do Decisor com confiança e botão opcional de aplicar.
class DeciderSuggestionChip extends StatelessWidget {
  const DeciderSuggestionChip({
    super.key,
    required this.suggestion,
    required this.confidence,
    this.onApply,
  });

  final String suggestion;
  final double confidence;
  final VoidCallback? onApply;

  @override
  Widget build(BuildContext context) {
    final label =
        'Sugestão: $suggestion (${formatPercent(confidence * 100, digits: 0)})';
    if (onApply == null) {
      return SemanticChip(
        label: label,
        tone: SemanticTone.ai,
        icon: Icons.auto_awesome,
        compact: true,
      );
    }
    return ActionChip(
      avatar: Icon(
        Icons.auto_awesome,
        size: 16,
        color: context.semanticColors.ai,
      ),
      label: Text(label),
      onPressed: onApply,
    );
  }
}

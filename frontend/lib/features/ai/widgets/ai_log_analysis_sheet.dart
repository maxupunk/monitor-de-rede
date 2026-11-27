import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/utils/clipboard_utils.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_log_analysis_presentation.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// "Analisar com a IA" de um console: a IA lê o conteúdo inteiro (com as
/// credenciais ocultadas) e devolve erros, avisos, descontinuações e
/// sugestões, cada um com a linha de origem. Registrado como
/// [aiLogAnalysisLauncherProvider] pelo Assistente.
Future<void> showAiLogAnalysisSheet(
  BuildContext context,
  AiLogAnalysisRequest request,
) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Análise do console pela IA',
    subtitle: request.source,
    maxWidth: 720,
    builder: (_) => AiLogAnalysisBody(request: request),
  );
}

@visibleForTesting
class AiLogAnalysisBody extends ConsumerWidget {
  const AiLogAnalysisBody({super.key, required this.request});

  final AiLogAnalysisRequest request;

  int get _lineCount => '\n'.allMatches(request.content.trimRight()).length + 1;

  void _openSettings(BuildContext context) {
    Navigator.of(context).pop();
    GoRouter.of(context).go('/settings?tab=ia');
  }

  void _openChat(BuildContext context, WidgetRef ref) {
    final chat = ref.read(aiChatProvider.notifier);
    Navigator.of(context).pop();
    chat.askAbout(request.chatPrompt!);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final enabled = ref.watch(aiSettingsProvider).settings?.enabled;
    final disabled = enabled == false;
    final analysis = disabled
        ? null
        : ref.watch(aiLogAnalysisProvider(request));
    final loading = analysis?.isLoading ?? false;
    final failed = analysis?.hasError == true && !loading;
    final report = loading ? null : analysis?.valueOrNull;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        NestedPanel(
          tone: SemanticTone.info,
          child: Text(
            'A IA lê o console (${countNoun(_lineCount, 'linha')}) e aponta '
            'erros, avisos, descontinuações e sugestões, com a linha de '
            'origem. Senhas, tokens e chaves são ocultados antes do envio; '
            'nada é alterado.',
          ),
        ),
        const SizedBox(height: 12),
        if (disabled)
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'A IA está desligada. Ligue em Configurações › Inteligência '
              'artificial para analisar o console.',
            ),
          )
        else
          analysis!.when(
            skipLoadingOnRefresh: false,
            skipError: false,
            loading: () => const _Analyzing(),
            error: (err, _) => NestedPanel(
              tone: SemanticTone.error,
              child: Text(
                describeError(
                  err,
                  fallback: 'A IA não conseguiu analisar o console.',
                ),
              ),
            ),
            data: (report) => _ReportView(report: report),
          ),
        const SizedBox(height: 16),
        SymmetricalActionBar(
          children: <Widget>[
            if (disabled || failed)
              FilledButton.icon(
                onPressed: () => _openSettings(context),
                icon: const Icon(Icons.settings_outlined, size: 18),
                label: const Text('Abrir configurações de IA'),
              ),
            if (!disabled)
              OutlinedButton.icon(
                onPressed: loading
                    ? null
                    : () => ref.invalidate(aiLogAnalysisProvider(request)),
                icon: const Icon(Icons.refresh, size: 18),
                label: Text(failed ? 'Tentar de novo' : 'Analisar de novo'),
              ),
            if (report != null)
              OutlinedButton.icon(
                onPressed: () => copyToClipboardWithFeedback(
                  context,
                  formatAiLogAnalysisForCopy(report, source: request.source),
                  message:
                      'Análise copiada — cole em outra IA, num chamado ou '
                      'numa issue.',
                ),
                icon: const Icon(Icons.copy_all_outlined, size: 18),
                label: const Text('Copiar análise'),
              ),
            if (!disabled && request.chatPrompt != null)
              FilledButton.tonalIcon(
                onPressed: loading ? null : () => _openChat(context, ref),
                icon: const Icon(Icons.forum_outlined, size: 18),
                label: const Text('Continuar no chat'),
              ),
          ],
        ),
      ],
    );
  }
}

class _Analyzing extends StatelessWidget {
  const _Analyzing();

  @override
  Widget build(BuildContext context) {
    return const NestedPanel(
      tone: SemanticTone.ai,
      child: Row(
        children: <Widget>[
          SizedBox(
            width: 20,
            height: 20,
            child: CircularProgressIndicator(strokeWidth: 2.5),
          ),
          SizedBox(width: 12),
          Expanded(
            child: Text(
              'A IA está lendo o console… costuma levar de alguns segundos '
              'a um minuto.',
            ),
          ),
        ],
      ),
    );
  }
}

/// Rótulo, tom e ícone de cada tipo de item.
({String label, SemanticTone tone, IconData icon}) _kindMeta(
  AiLogFindingKind kind,
) {
  return switch (kind) {
    AiLogFindingKind.error => (
      label: aiLogFindingKindLabel(kind),
      tone: SemanticTone.error,
      icon: Icons.error_outline,
    ),
    AiLogFindingKind.warning => (
      label: aiLogFindingKindLabel(kind),
      tone: SemanticTone.warning,
      icon: Icons.warning_amber_rounded,
    ),
    AiLogFindingKind.deprecation => (
      label: aiLogFindingKindLabel(kind),
      tone: SemanticTone.caution,
      icon: Icons.history_toggle_off,
    ),
    AiLogFindingKind.suggestion => (
      label: aiLogFindingKindLabel(kind),
      tone: SemanticTone.info,
      icon: Icons.lightbulb_outline,
    ),
  };
}

class _ReportView extends StatelessWidget {
  const _ReportView({required this.report});

  final AiLogAnalysisReport report;

  @override
  Widget build(BuildContext context) {
    final kinds = AiLogFindingKind.values
        .where((kind) => report.count(kind) > 0)
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
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
                    'Resumo',
                    style: context.textTheme.labelLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ],
              ),
              if (report.summary.isNotEmpty) ...<Widget>[
                const SizedBox(height: 6),
                SelectableText(report.summary),
              ],
              if (kinds.isNotEmpty) ...<Widget>[
                const SizedBox(height: 10),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: <Widget>[
                    for (final kind in kinds)
                      SemanticChip(
                        label: aiLogFindingCountLabel(kind, report.count(kind)),
                        tone: _kindMeta(kind).tone,
                        icon: _kindMeta(kind).icon,
                        compact: true,
                      ),
                  ],
                ),
              ],
            ],
          ),
        ),
        if (report.truncated) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'O console tem ${countNoun(report.totalLines, 'linha')}; a IA '
              'leu ${report.analyzedLines} — as que citam erro, aviso ou '
              'descontinuação e o final. Para um trecho específico, continue '
              'no chat.',
            ),
          ),
        ],
        if (report.masked) ...<Widget>[
          const SizedBox(height: 10),
          const NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              'Credenciais encontradas no console foram ocultadas antes do '
              'envio à IA.',
            ),
          ),
        ],
        const SizedBox(height: 10),
        if (!report.structured)
          const NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              'A IA respondeu sem a lista de itens; o resumo acima é o texto '
              'dela. Tente de novo ou continue no chat.',
            ),
          )
        else if (report.findings.isEmpty)
          const NestedPanel(
            tone: SemanticTone.success,
            child: Row(
              children: <Widget>[
                Icon(Icons.check_circle_outline, size: 18),
                SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Nenhum problema encontrado: sem erros, avisos ou '
                    'descontinuações no console.',
                  ),
                ),
              ],
            ),
          )
        else
          for (final (index, finding) in report.findings.indexed) ...<Widget>[
            if (index > 0) const SizedBox(height: 8),
            _FindingTile(finding: finding),
          ],
        if (report.model != null || report.generatedAt != null) ...<Widget>[
          const SizedBox(height: 10),
          Text(
            <String>[
              if (report.model != null) 'Modelo ${report.model}',
              if (report.generatedAt != null)
                'gerado ${formatRelativeTime(report.generatedAt)}',
            ].join(' · '),
            style: context.textTheme.bodySmall,
          ),
        ],
      ],
    );
  }
}

class _FindingTile extends StatelessWidget {
  const _FindingTile({required this.finding});

  final AiLogFinding finding;

  @override
  Widget build(BuildContext context) {
    final meta = _kindMeta(finding.kind);
    final scheme = context.colorScheme;

    return NestedPanel(
      tone: meta.tone,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: <Widget>[
              SemanticChip(
                label: meta.label,
                tone: meta.tone,
                icon: meta.icon,
                compact: true,
              ),
              if (finding.line != null)
                SemanticChip(
                  label: 'Linha ${finding.line}',
                  tone: SemanticTone.primary,
                  compact: true,
                ),
            ],
          ),
          const SizedBox(height: 8),
          Text(
            finding.message,
            style: context.textTheme.bodyMedium?.copyWith(
              fontWeight: FontWeight.w600,
            ),
          ),
          if (finding.excerpt.isNotEmpty &&
              finding.excerpt != finding.message) ...<Widget>[
            const SizedBox(height: 6),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
              decoration: BoxDecoration(
                color: scheme.surfaceContainerLowest,
                borderRadius: AppTheme.borderRadius,
                border: Border.all(color: scheme.outlineVariant),
              ),
              child: SelectableText(
                finding.excerpt,
                style: TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 12.5,
                  color: scheme.onSurface,
                ),
              ),
            ),
          ],
          if (finding.suggestion != null) ...<Widget>[
            const SizedBox(height: 6),
            Text.rich(
              TextSpan(
                children: <InlineSpan>[
                  const TextSpan(
                    text: 'Como resolver: ',
                    style: TextStyle(fontWeight: FontWeight.w700),
                  ),
                  TextSpan(text: finding.suggestion),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}

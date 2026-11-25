import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// O alerta que a IA vai resumir — só o que a folha precisa.
@immutable
class AiAlertSummaryTarget {
  const AiAlertSummaryTarget({
    required this.alertId,
    required this.title,
    this.deviceId,
    this.monitorId,
    this.resolved = false,
    this.existing,
  });

  final int alertId;
  final String title;
  final int? deviceId;
  final int? monitorId;
  final bool resolved;

  /// O resumo que o alerta já tem, se tiver.
  final AiIncidentSummary? existing;

  AiChatContext get chatContext =>
      AiChatContext(alertId: alertId, deviceId: deviceId, monitorId: monitorId);
}

/// "Resumo IA" de um alerta: explica o que a IA faz, gera o resumo com
/// progresso visível e oferece continuar a conversa no chat.
///
/// O pedido leva de segundos a um minuto (a IA consulta o alerta, o
/// equipamento, a causa raiz, o comportamento normal e os logs antes de
/// responder). Disparado sem folha, ele não dava sinal nenhum até terminar.
Future<void> showAiAlertSummarySheet({
  required BuildContext context,
  required AiAlertSummaryTarget target,
  ValueChanged<AiIncidentSummary>? onGenerated,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Resumo IA do alerta',
    subtitle: target.title,
    maxWidth: 600,
    builder: (_) =>
        AiAlertSummaryBody(target: target, onGenerated: onGenerated),
  );
}

enum _Phase { generating, ready, nothingToDo, disabled, failed }

@visibleForTesting
class AiAlertSummaryBody extends ConsumerStatefulWidget {
  const AiAlertSummaryBody({super.key, required this.target, this.onGenerated});

  final AiAlertSummaryTarget target;
  final ValueChanged<AiIncidentSummary>? onGenerated;

  @override
  ConsumerState<AiAlertSummaryBody> createState() => _AiAlertSummaryBodyState();
}

class _AiAlertSummaryBodyState extends ConsumerState<AiAlertSummaryBody> {
  late _Phase _phase;
  AiIncidentSummary? _summary;
  String? _error;

  @override
  void initState() {
    super.initState();
    _summary = widget.target.existing;
    final enabled = ref.read(aiSettingsProvider).settings?.enabled;
    if (_summary != null) {
      _phase = _Phase.ready;
    } else if (enabled == false) {
      _phase = _Phase.disabled;
    } else if (widget.target.resolved) {
      _phase = _Phase.nothingToDo;
    } else {
      _phase = _Phase.generating;
      Future<void>.microtask(_generate);
    }
  }

  Future<void> _generate() async {
    setState(() {
      _phase = _Phase.generating;
      _error = null;
    });
    try {
      final summary = await ref
          .read(aiAlertFeedbackProvider.notifier)
          .requestSummary(widget.target.alertId);
      if (!mounted) return;
      if (summary != null) widget.onGenerated?.call(summary);
      setState(() {
        _summary = summary;
        _phase = summary != null ? _Phase.ready : _Phase.nothingToDo;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err, fallback: 'A IA não conseguiu resumir.');
        _phase = _Phase.failed;
      });
    }
  }

  void _openChat() {
    final target = widget.target;
    Navigator.of(context).pop();
    ref
        .read(aiChatProvider.notifier)
        .askAbout(
          'Sobre o alerta #${target.alertId} (${target.title}): qual a causa '
          'provável, que evidência sustenta isso e o que devo fazer agora?',
          context: target.chatContext,
        );
  }

  void _openSettings() {
    Navigator.of(context).pop();
    GoRouter.of(context).go('/settings?tab=ia');
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        const _HowItWorks(),
        const SizedBox(height: 12),
        _content(context),
        const SizedBox(height: 16),
        SymmetricalActionBar(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            if (_phase == _Phase.disabled)
              FilledButton.icon(
                onPressed: _openSettings,
                icon: const Icon(Icons.settings_outlined, size: 18),
                label: const Text('Abrir configurações de IA'),
              )
            else
              FilledButton.tonalIcon(
                onPressed: _phase == _Phase.generating ? null : _openChat,
                icon: const Icon(Icons.forum_outlined, size: 18),
                label: const Text('Continuar no chat'),
              ),
            if (_phase == _Phase.failed)
              OutlinedButton.icon(
                onPressed: _generate,
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('Tentar de novo'),
              ),
          ],
        ),
      ],
    );
  }

  Widget _content(BuildContext context) {
    switch (_phase) {
      case _Phase.generating:
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
                  'A IA está investigando o alerta… costuma levar de alguns '
                  'segundos a um minuto. Pode fechar: o resumo fica gravado '
                  'no alerta quando ficar pronto.',
                ),
              ),
            ],
          ),
        );
      case _Phase.ready:
        final summary = _summary!;
        return NestedPanel(
          tone: SemanticTone.ai,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              SelectableText(summary.text),
              if (summary.recommendation?.isNotEmpty == true) ...<Widget>[
                const SizedBox(height: 8),
                Text(
                  'Recomendação: ${summary.recommendation}',
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
              ],
              if (summary.generatedAt != null) ...<Widget>[
                const SizedBox(height: 8),
                Text(
                  'Gerado ${formatRelativeTime(summary.generatedAt)}',
                  style: context.textTheme.bodySmall,
                ),
              ],
            ],
          ),
        );
      case _Phase.nothingToDo:
        return const NestedPanel(
          tone: SemanticTone.info,
          child: Text(
            'Não há resumo novo para gerar: o alerta já foi resolvido ou já '
            'tem um resumo gravado. Para outra análise, continue no chat.',
          ),
        );
      case _Phase.disabled:
        return const NestedPanel(
          tone: SemanticTone.warning,
          child: Text(
            'A IA está desligada. Ligue em Configurações › Inteligência '
            'artificial para gerar resumos de alerta.',
          ),
        );
      case _Phase.failed:
        return NestedPanel(
          tone: SemanticTone.error,
          child: Text(_error ?? 'A IA não conseguiu resumir.'),
        );
    }
  }
}

class _HowItWorks extends StatelessWidget {
  const _HowItWorks();

  @override
  Widget build(BuildContext context) {
    return NestedPanel(
      tone: SemanticTone.info,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            'Como funciona',
            style: context.textTheme.labelLarge?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 4),
          const Text(
            'A IA consulta o alerta, o equipamento, a análise de causa raiz, '
            'a comparação com o comportamento normal e os logs da última '
            'hora — só leitura, nada é alterado. Responde em poucas linhas: '
            'causa provável, a evidência e a primeira ação. O resumo fica '
            'gravado no alerta. Alertas novos podem ganhar resumo automático '
            '(Configurações › Inteligência artificial).',
          ),
        ],
      ),
    );
  }
}

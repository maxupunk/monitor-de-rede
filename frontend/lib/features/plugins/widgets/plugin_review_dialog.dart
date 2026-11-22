import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Diálogo obrigatório de revisão de segurança da quarentena (`PluginReviewDialog.vue`, ADR 012).
///
/// Regras de segurança inegociáveis:
/// - `risk == 'critical'` bloqueia permanentemente a instalação como rascunho.
/// - `!review.ai || risk == 'high'` exige checkbox explícito de ciência de risco (`acknowledgeRisk: true`).
Future<PluginDetail?> showPluginReviewDialog({
  required BuildContext context,
  required PluginDetail detail,
  int? refreshDeviceId,
}) {
  return showAdaptiveFormSheet<PluginDetail>(
    context: context,
    title: 'Revisão de segurança · ${detail.summary.name}',
    subtitle:
        'Plugins importados entram em quarentena até aprovação explícita.',
    maxWidth: 660,
    builder: (_) => _PluginReviewBody(
      initialDetail: detail,
      refreshDeviceId: refreshDeviceId,
    ),
  );
}

class _PluginReviewBody extends ConsumerStatefulWidget {
  const _PluginReviewBody({required this.initialDetail, this.refreshDeviceId});

  final PluginDetail initialDetail;
  final int? refreshDeviceId;

  @override
  ConsumerState<_PluginReviewBody> createState() => _PluginReviewBodyState();
}

class _PluginReviewBodyState extends ConsumerState<_PluginReviewBody> {
  late PluginDetail _detail;
  bool _acknowledged = false;
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _detail = widget.initialDetail;
  }

  Future<void> _reReview() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final updated = await ref
          .read(pluginsProvider.notifier)
          .lifecycle(
            _detail.summary.id,
            'review',
            refreshDeviceId: widget.refreshDeviceId,
          );
      if (mounted) {
        setState(() {
          _detail = updated;
          _acknowledged = false;
        });
      }
    } catch (err) {
      if (mounted) setState(() => _error = describeError(err));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _acceptReview() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final updated = await ref
          .read(pluginsProvider.notifier)
          .acceptReview(
            _detail.summary.id,
            acknowledgeRisk: _acknowledged,
            refreshDeviceId: widget.refreshDeviceId,
          );
      if (mounted) Navigator.of(context).pop(updated);
    } catch (err) {
      if (mounted) setState(() => _error = describeError(err));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final review = _detail.review;
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;

    if (review == null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text('Este plugin ainda não possui relatório de revisão.'),
          ),
          const SizedBox(height: 14),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: _busy ? null : _reReview,
              icon: const Icon(Icons.security_outlined, size: 18),
              label: const Text('Executar revisão agora'),
            ),
          ),
        ],
      );
    }

    final riskPres = severityPresentation(review.risk);
    final blocked = review.isBlocked;
    final needsAck = review.requiresAcknowledgement;
    final findings = review.allFindings;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 6,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            SemanticChip(
              label: 'Risco: ${riskPres.label}',
              tone: riskPres.tone,
              icon: riskPres.icon,
            ),
            if (review.reviewedAt != null)
              Text(
                'Revisado ${formatRelativeTime(review.reviewedAt)}',
                style: context.textTheme.bodySmall?.copyWith(
                  color: scheme.onSurfaceVariant,
                ),
              ),
          ],
        ),
        const SizedBox(height: 12),
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 10),
        ],
        if (blocked) ...<Widget>[
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: semantics.error,
              borderRadius: AppTheme.borderRadius,
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(Icons.block, color: semantics.onError),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'BLOQUEADO: risco crítico detectado. Este plugin não pode ser instalado como rascunho enquanto os problemas críticos não forem removidos e revisados novamente.',
                    style: context.textTheme.bodySmall?.copyWith(
                      color: semantics.onError,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (review.ai == null) ...<Widget>[
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Revisão por IA indisponível — apenas análise estática foi executada. Confirme manualmente antes de aceitar.',
            ),
          ),
          const SizedBox(height: 10),
        ] else ...<Widget>[
          if (review.ai!.mismatchWithUsage) ...<Widget>[
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: semantics.errorContainer,
                borderRadius: AppTheme.borderRadius,
                border: Border.all(color: semantics.error),
              ),
              child: Text(
                'Divergência crítica: o script realiza ações não documentadas na seção de Uso!',
                style: context.textTheme.bodySmall?.copyWith(
                  color: semantics.onErrorContainer,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(height: 10),
          ],
          if (review.ai!.summary.isNotEmpty) ...<Widget>[
            NestedPanel(tone: SemanticTone.ai, child: Text(review.ai!.summary)),
            const SizedBox(height: 10),
          ],
        ],
        if (findings.isEmpty)
          const NestedPanel(
            tone: SemanticTone.success,
            child: Text('Nenhum problema encontrado na análise de segurança.'),
          )
        else ...<Widget>[
          Text(
            'Achados (${findings.length})',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          for (final finding in findings) ...<Widget>[
            Builder(
              builder: (_) {
                final pres = severityPresentation(finding.severity);
                return NestedPanel(
                  tone: pres.tone,
                  padding: const EdgeInsets.all(10),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Icon(
                            pres.icon,
                            size: 16,
                            color: semantics
                                .resolve(pres.tone, scheme)
                                .onContainer,
                          ),
                          const SizedBox(width: 6),
                          Expanded(
                            child: Text(
                              finding.message,
                              style: context.textTheme.bodySmall?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                          ),
                          if (finding.line != null)
                            Text(
                              'Linha ${finding.line}',
                              style: context.textTheme.labelSmall,
                            ),
                        ],
                      ),
                      if (finding.rule != null)
                        Text(
                          'Regra: ${finding.rule} (${finding.source})',
                          style: context.textTheme.labelSmall?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      if (finding.excerpt != null &&
                          finding.excerpt!.isNotEmpty) ...<Widget>[
                        const SizedBox(height: 6),
                        Text(
                          finding.excerpt!,
                          style: const TextStyle(
                            fontFamily: 'monospace',
                            fontSize: 11.5,
                          ),
                        ),
                      ],
                    ],
                  ),
                );
              },
            ),
            const SizedBox(height: 8),
          ],
        ],
        if (!blocked && needsAck) ...<Widget>[
          const SizedBox(height: 8),
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            controlAffinity: ListTileControlAffinity.leading,
            value: _acknowledged,
            onChanged: _busy
                ? null
                : (v) => setState(() => _acknowledged = v == true),
            title: const Text(
              'Revisei os riscos apontados e quero instalar este plugin como rascunho',
              style: TextStyle(fontSize: 13.5, fontWeight: FontWeight.w600),
            ),
          ),
        ],
        const SizedBox(height: 14),
        ResponsiveFieldRow(
          breakpoint: 400,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: _busy ? null : _reReview,
              icon: const Icon(Icons.refresh, size: 18),
              label: const Text(
                'Revisar de novo',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            FilledButton.icon(
              onPressed: _busy || blocked || (needsAck && !_acknowledged)
                  ? null
                  : _acceptReview,
              icon: _busy
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.check_circle_outline, size: 18),
              label: const Text(
                'Instalar como rascunho',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

String linkTestKindLabel(String kind) => switch (kind.trim().toLowerCase()) {
  'link' => 'Enlace',
  'server' => 'Servidor',
  'end_to_end' => 'Ponta a ponta',
  'path' => 'Caminho',
  _ => kind,
};

String linkTestDirectionLabel(String direction) =>
    switch (direction.trim().toLowerCase()) {
      'both' => 'Ida e volta',
      'forward' => 'Somente ida',
      'reverse' => 'Somente volta',
      _ => direction,
    };

String linkTestStatusLabel(String status) =>
    switch (status.trim().toLowerCase()) {
      'completed' || 'succeeded' => 'Concluído',
      'running' => 'Em execução',
      'preparing' => 'Preparando',
      'queued' || 'pending' => 'Na fila',
      'failed' || 'error' => 'Falhou',
      'cancelled' || 'canceled' => 'Cancelado',
      _ => status,
    };

StatusIndicator buildLinkTestStatus(String status, {bool compact = true}) {
  final normalized = status.trim().toLowerCase();
  return switch (normalized) {
    'completed' || 'succeeded' => StatusIndicator(
      label: 'Concluído',
      tone: SemanticTone.success,
      compact: compact,
    ),
    'running' => StatusIndicator(
      label: 'Em execução',
      tone: SemanticTone.info,
      compact: compact,
    ),
    'preparing' => StatusIndicator(
      label: 'Preparando',
      tone: SemanticTone.info,
      compact: compact,
    ),
    'queued' || 'pending' => StatusIndicator(
      label: 'Na fila',
      tone: SemanticTone.secondary,
      compact: compact,
    ),
    'failed' || 'error' => StatusIndicator(
      label: 'Falhou',
      tone: SemanticTone.error,
      compact: compact,
    ),
    'cancelled' || 'canceled' => StatusIndicator(
      label: 'Cancelado',
      tone: SemanticTone.warning,
      compact: compact,
    ),
    _ => StatusIndicator.fromStatus(status, compact: compact),
  };
}

class FocusedLinkTestCard extends StatelessWidget {
  const FocusedLinkTestCard({
    super.key,
    required this.test,
    required this.state,
    required this.canWrite,
    required this.onClose,
    required this.onCancel,
  });

  final LinkTestModel test;
  final LinkTestsState state;
  final bool canWrite;
  final VoidCallback onClose;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final firstLeg = test.legs.firstOrNull;
    final fwdSamples = firstLeg != null
        ? state.samplesFor(test.id, firstLeg.seq, 'forward')
        : const <LinkTestSampleModel>[];
    final revSamples = firstLeg != null
        ? state.samplesFor(test.id, firstLeg.seq, 'reverse')
        : const <LinkTestSampleModel>[];
    final healthSamples = firstLeg != null
        ? state.healthFor(test.id, firstLeg.seq)
        : const <LinkTestHealthSampleModel>[];

    final fwdPts = fwdSamples
        .map((s) => MetricSeriesPoint(x: s.tMs / 1000.0, y: s.bps))
        .toList(growable: false);
    final revPts = revSamples
        .map((s) => MetricSeriesPoint(x: s.tMs / 1000.0, y: s.bps))
        .toList(growable: false);
    final cpuPts = healthSamples
        .where((h) => h.cpu != null)
        .map((h) => MetricSeriesPoint(x: h.atMs / 1000.0, y: h.cpu!))
        .toList(growable: false);

    return SectionCard(
      title: 'Teste #${test.id} · ${test.title}',
      subtitle:
          '${linkTestKindLabel(test.kind)} · ${linkTestDirectionLabel(test.spec.direction)} · ${test.spec.durationS}s (${countNoun(test.spec.streams, 'fluxo', 'fluxos')})',
      icon: Icons.speed,
      iconTone: test.isActive ? SemanticTone.info : SemanticTone.success,
      trailing: IconButton(
        tooltip: 'Fechar painel',
        icon: const Icon(Icons.close),
        onPressed: onClose,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              buildLinkTestStatus(test.status),
              if (test.isActive && canWrite)
                OutlinedButton.icon(
                  onPressed: onCancel,
                  icon: const Icon(Icons.stop, size: 16),
                  label: const Text('Cancelar'),
                ),
            ],
          ),
          const SizedBox(height: 10),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: test.legs
                  .map((leg) {
                    final isBottleneck =
                        test.summary?.bottleneckLegSeq == leg.seq;
                    return Padding(
                      padding: const EdgeInsets.only(right: 10),
                      child: NestedPanel(
                        tone: isBottleneck
                            ? SemanticTone.warning
                            : (leg.status == 'succeeded' ||
                                  leg.status == 'completed')
                            ? SemanticTone.success
                            : SemanticTone.info,
                        padding: const EdgeInsets.all(10),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Row(
                              mainAxisSize: MainAxisSize.min,
                              children: <Widget>[
                                Text(
                                  leg.a.name,
                                  style: context.textTheme.labelLarge?.copyWith(
                                    fontWeight: FontWeight.w800,
                                  ),
                                ),
                                const Padding(
                                  padding: EdgeInsets.symmetric(horizontal: 6),
                                  child: Icon(
                                    Icons.arrow_forward_rounded,
                                    size: 15,
                                  ),
                                ),
                                Text(
                                  leg.b.name,
                                  style: context.textTheme.labelLarge?.copyWith(
                                    fontWeight: FontWeight.w800,
                                  ),
                                ),
                              ],
                            ),
                            const SizedBox(height: 4),
                            Text(
                              'Ida: ${formatBps(leg.forwardBps)} · Volta: ${formatBps(leg.reverseBps)}'
                              '${leg.engine != null ? ' · ${leg.engine}' : ''}',
                              style: context.textTheme.bodySmall,
                            ),
                            if (isBottleneck)
                              const Padding(
                                padding: EdgeInsets.only(top: 4),
                                child: SemanticChip(
                                  label: 'Gargalo do caminho',
                                  tone: SemanticTone.warning,
                                  compact: true,
                                ),
                              ),
                          ],
                        ),
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
          ),
          if (fwdPts.isNotEmpty || revPts.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            MetricLineChart(
              height: 180,
              yFormatter: (v) => formatBps(v),
              xFormatter: (v) => '${v.round()}s',
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'Ida',
                  points: fwdPts,
                  tone: SemanticTone.success,
                ),
                MetricLineSeries(
                  name: 'Volta',
                  points: revPts,
                  tone: SemanticTone.info,
                ),
              ],
            ),
          ],
          if (cpuPts.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            Text(
              'Saúde das pontas durante a medição (CPU %)',
              style: context.textTheme.labelMedium,
            ),
            const SizedBox(height: 4),
            MetricLineChart(
              height: 110,
              minY: 0,
              maxY: 100,
              yFormatter: (v) => formatPercent(v, digits: 0),
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'CPU %',
                  points: cpuPts,
                  tone: SemanticTone.warning,
                ),
              ],
            ),
          ],
          if (test.error != null) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(tone: SemanticTone.error, child: Text(test.error!)),
          ],
        ],
      ),
    );
  }
}

class LinkTestComparisonCard extends StatelessWidget {
  const LinkTestComparisonCard({
    super.key,
    required this.a,
    required this.b,
    required this.onClear,
  });

  final LinkTestModel a;
  final LinkTestModel b;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    return NestedPanel(
      tone: SemanticTone.primary,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  'Comparação: #${a.id} vs #${b.id}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              IconButton(
                visualDensity: VisualDensity.compact,
                onPressed: onClear,
                icon: const Icon(Icons.close, size: 18),
              ),
            ],
          ),
          KeyValueRow(
            label: 'Ida mínima',
            value:
                '${formatBps(a.summary?.minForwardBps)}  vs  ${formatBps(b.summary?.minForwardBps)}',
          ),
          KeyValueRow(
            label: 'Volta mínima',
            value:
                '${formatBps(a.summary?.minReverseBps)}  vs  ${formatBps(b.summary?.minReverseBps)}',
          ),
        ],
      ),
    );
  }
}

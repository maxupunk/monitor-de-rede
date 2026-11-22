import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// 1. Cartões KPI principais do Dashboard (`stat_cards`).
class StatCardsWidget extends ConsumerWidget {
  const StatCardsWidget({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final monitors = ref.watch(monitorsProvider);
    final alerts = ref.watch(alertsProvider);
    final totalDevices = monitors.devices.length;
    final onlineDevices = monitors.devices
        .where((d) => (d.status ?? '').toLowerCase() == 'online')
        .length;

    return ResponsiveGrid(
      xs: 2,
      sm: 2,
      md: 4,
      lg: 4,
      xl: 4,
      children: <Widget>[
        KpiCard(
          label: 'Dispositivos',
          value: formatInteger(totalDevices),
          subtitle: '$onlineDevices online',
          icon: Icons.devices_outlined,
          tone: SemanticTone.primary,
          onTap: () => context.go('/devices'),
        ),
        KpiCard(
          label: 'Monitores ativos',
          value: formatInteger(monitors.upCount),
          subtitle: 'de ${formatInteger(monitors.totalCount)} monitorados',
          icon: Icons.monitor_heart_outlined,
          tone: SemanticTone.success,
          onTap: () => context.go('/monitors?status=up'),
        ),
        KpiCard(
          label: 'Indisponíveis',
          value: formatInteger(monitors.downCount),
          subtitle: monitors.warningCount > 0
              ? '${monitors.warningCount} em atenção'
              : (monitors.downCount > 0
                    ? 'Exige verificação'
                    : 'Sem degradação'),
          icon: Icons.error_outline,
          tone: monitors.downCount > 0 ? SemanticTone.error : SemanticTone.info,
          onTap: () => context.go('/monitors?status=down'),
        ),
        KpiCard(
          label: 'Alertas abertos',
          value: formatInteger(alerts.activeAlerts.length),
          subtitle: alerts.criticalCount > 0
              ? '${alerts.criticalCount} críticos'
              : 'Operação estável',
          icon: Icons.notifications_active_outlined,
          tone: alerts.criticalCount > 0
              ? SemanticTone.error
              : (alerts.activeAlerts.isNotEmpty
                    ? SemanticTone.warning
                    : SemanticTone.success),
          onTap: () => context.go('/alerts'),
        ),
      ],
    );
  }
}

/// 2. Medidor circular de saúde geral dos monitores (`health_gauge`).
class HealthGaugeWidget extends ConsumerWidget {
  const HealthGaugeWidget({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final monitors = ref.watch(monitorsProvider);
    final active = monitors.monitors
        .where((m) => m.isEnabled)
        .toList(growable: false);
    final total = active.length;
    final up = active.where((m) => m.normalizedStatus == 'up').length;
    final warning = active.where((m) => m.normalizedStatus == 'warning').length;
    final down = active.where((m) => m.normalizedStatus == 'down').length;
    final unknown = (total - up - warning - down).clamp(0, total);
    final healthPct = total > 0 ? (up / total) * 100.0 : 100.0;

    final scheme = context.colorScheme;
    final sem = context.semanticColors;
    final healthTone = healthPct >= 95
        ? SemanticTone.success
        : (healthPct >= 80 ? SemanticTone.warning : SemanticTone.error);

    return SectionCard(
      title: 'Saúde geral',
      subtitle: total > 0
          ? '$up de $total monitores ativos saudáveis'
          : 'Nenhum monitor ativo cadastrado',
      icon: Icons.favorite_outline,
      iconTone: healthTone,
      trailing: SemanticChip(
        label: formatPercent(healthPct, digits: 1),
        tone: healthTone,
        filled: true,
        compact: true,
      ),
      child: total == 0
          ? const EmptyStateView(
              title: 'Sem monitores ativos',
              description: 'Crie monitores para acompanhar a saúde da rede.',
              icon: Icons.monitor_heart_outlined,
            )
          : LayoutBuilder(
              builder: (context, constraints) {
                final narrow = constraints.maxWidth < 330;
                final gaugeSize = narrow ? 124.0 : 140.0;
                final gauge = SizedBox(
                  width: gaugeSize,
                  height: gaugeSize,
                  child: Stack(
                    alignment: Alignment.center,
                    children: <Widget>[
                      PieChart(
                        PieChartData(
                          sectionsSpace: 2,
                          centerSpaceRadius: narrow ? 36 : 42,
                          sections: <PieChartSectionData>[
                            if (up > 0)
                              PieChartSectionData(
                                value: up.toDouble(),
                                color: sem.success,
                                radius: 18,
                                showTitle: false,
                              ),
                            if (warning > 0)
                              PieChartSectionData(
                                value: warning.toDouble(),
                                color: sem.warning,
                                radius: 18,
                                showTitle: false,
                              ),
                            if (down > 0)
                              PieChartSectionData(
                                value: down.toDouble(),
                                color: sem.error,
                                radius: 18,
                                showTitle: false,
                              ),
                            if (unknown > 0)
                              PieChartSectionData(
                                value: unknown.toDouble(),
                                color: scheme.outline,
                                radius: 18,
                                showTitle: false,
                              ),
                          ],
                        ),
                      ),
                      Column(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          Text(
                            formatPercent(healthPct, digits: 0),
                            style: context.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                              color: scheme.onSurface,
                            ),
                          ),
                          Text(
                            'saúde',
                            style: context.textTheme.labelSmall?.copyWith(
                              color: scheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                );
                final legend = Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    _LegendRow(
                      label: 'Online',
                      count: up,
                      tone: SemanticTone.success,
                    ),
                    const SizedBox(height: 8),
                    _LegendRow(
                      label: 'Atenção',
                      count: warning,
                      tone: SemanticTone.warning,
                    ),
                    const SizedBox(height: 8),
                    _LegendRow(
                      label: 'Indisponível',
                      count: down,
                      tone: SemanticTone.error,
                    ),
                    if (unknown > 0) ...<Widget>[
                      const SizedBox(height: 8),
                      _LegendRow(
                        label: 'Desconhecido',
                        count: unknown,
                        tone: SemanticTone.secondary,
                      ),
                    ],
                  ],
                );
                if (narrow) {
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      Center(child: gauge),
                      const SizedBox(height: 12),
                      legend,
                    ],
                  );
                }
                return Row(
                  children: <Widget>[
                    gauge,
                    const SizedBox(width: 16),
                    Expanded(child: legend),
                  ],
                );
              },
            ),
    );
  }
}

class _LegendRow extends StatelessWidget {
  const _LegendRow({
    required this.label,
    required this.count,
    required this.tone,
  });

  final String label;
  final int count;
  final SemanticTone tone;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: <Widget>[
        Expanded(
          child: StatusIndicator(label: label, tone: tone, compact: true),
        ),
        Text(
          formatInteger(count),
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
      ],
    );
  }
}

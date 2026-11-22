import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/features/core/presentation/dashboard/widgets/alerts_and_events_widgets.dart';
import 'package:netmonitor_app/features/core/presentation/dashboard/widgets/metrics_and_monitors_widgets.dart';
import 'package:netmonitor_app/features/core/presentation/dashboard/widgets/overview_stat_widgets.dart';

export 'widgets/alerts_and_events_widgets.dart';
export 'widgets/metrics_and_monitors_widgets.dart';
export 'widgets/overview_stat_widgets.dart';

/// Os 11 widgets do Núcleo registrados em `coreArea.widgets` (ARQ-04).
Map<String, DashboardWidgetContribution> buildCoreDashboardWidgets() {
  return <String, DashboardWidgetContribution>{
    'stat_cards': DashboardWidgetContribution(
      type: 'stat_cards',
      builder: (_, config) => const StatCardsWidget(),
    ),
    'health_gauge': DashboardWidgetContribution(
      type: 'health_gauge',
      builder: (_, config) => const HealthGaugeWidget(),
    ),
    'active_alerts': DashboardWidgetContribution(
      type: 'active_alerts',
      builder: (_, config) => const ActiveAlertsWidget(),
    ),
    'latency_time_series': DashboardWidgetContribution(
      type: 'latency_time_series',
      builder: (_, config) => LatencyTimeSeriesWidget(config: config),
    ),
    'event_distribution': DashboardWidgetContribution(
      type: 'event_distribution',
      builder: (_, config) => EventDistributionWidget(config: config),
    ),
    'unstable_targets': DashboardWidgetContribution(
      type: 'unstable_targets',
      builder: (_, config) => const UnstableTargetsWidget(),
    ),
    'events_feed': DashboardWidgetContribution(
      type: 'events_feed',
      builder: (_, config) => const EventsFeedWidget(),
    ),
    'network_monitors': DashboardWidgetContribution(
      type: 'network_monitors',
      builder: (_, config) => const NetworkMonitorsWidget(),
    ),
    'cpu_usage': DashboardWidgetContribution(
      type: 'cpu_usage',
      builder: (_, config) =>
          ResourceUsageWidget(metric: 'cpu', config: config),
    ),
    'ram_usage': DashboardWidgetContribution(
      type: 'ram_usage',
      builder: (_, config) =>
          ResourceUsageWidget(metric: 'ram', config: config),
    ),
    'binary_status': DashboardWidgetContribution(
      type: 'binary_status',
      builder: (_, config) => BinaryStatusWidget(config: config),
    ),
  };
}

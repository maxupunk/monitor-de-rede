import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/features/core/presentation/settings/widgets/general_and_areas_cards.dart';
import 'package:netmonitor_app/features/core/presentation/settings/widgets/notifications_and_catalogs_cards.dart';
import 'package:netmonitor_app/features/core/presentation/settings/widgets/server_addresses_card.dart';

export 'widgets/general_and_areas_cards.dart';
export 'widgets/notifications_and_catalogs_cards.dart';
export 'widgets/server_addresses_card.dart';

/// Cartões de Configurações do Núcleo registrados em `coreArea.settingsCards`.
List<SettingsCardContribution> buildCoreSettingsCards() {
  return <SettingsCardContribution>[
    SettingsCardContribution(
      tab: 'geral',
      order: 10,
      mdCols: 6,
      lgCols: 6,
      builder: (_) => const PreferencesCard(),
    ),
    SettingsCardContribution(
      tab: 'geral',
      order: 20,
      mdCols: 6,
      lgCols: 6,
      builder: (_) => const DashboardSyncCard(),
    ),
    SettingsCardContribution(
      tab: 'geral',
      order: 30,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const AreasCard(),
    ),
    SettingsCardContribution(
      tab: 'geral',
      order: 40,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const OnboardingCard(),
    ),
    SettingsCardContribution(
      tab: 'conectividade',
      order: 10,
      mdCols: 6,
      lgCols: 6,
      builder: (_) => const ServerAddressesCard(),
    ),
    SettingsCardContribution(
      tab: 'notificacoes',
      order: 10,
      mdCols: 8,
      lgCols: 6,
      builder: (_) => const NotificationsCard(),
    ),
    SettingsCardContribution(
      tab: 'sistema',
      order: 10,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const BackupShortcutCard(),
    ),
    SettingsCardContribution(
      tab: 'sistema',
      order: 30,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const CoreCatalogsCard(),
    ),
  ];
}

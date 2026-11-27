import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/shared/widgets/section_card.dart';
import 'package:netmonitor_app/shared/widgets/semantic_chip.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Metadados de rota declarados por cada área para guards e layout.
@immutable
class AreaRouteMeta {
  const AreaRouteMeta({
    required this.area,
    this.requiresAdmin = false,
    this.fullBleed = false,
    this.aiScreen,
  });

  final ProductArea area;
  final bool requiresAdmin;
  final bool fullBleed;
  final AiScreenContribution? aiScreen;
}

/// Contexto de tela entregue ao Assistente IA.
@immutable
class AiScreenContribution {
  const AiScreenContribution({
    required this.id,
    required this.title,
    this.description,
    this.contextBuilder,
  });

  final String id;
  final String title;
  final String? description;
  final Map<String, dynamic> Function(WidgetRef ref, GoRouterState state)?
  contextBuilder;
}

/// Aba de Configurações declarada por uma área (somente aparece com área visível).
@immutable
class SettingsTabContribution {
  const SettingsTabContribution({
    required this.value,
    required this.title,
    required this.icon,
    required this.order,
    this.area,
  });

  final String value;
  final String title;
  final IconData icon;
  final int order;

  /// Área de visibilidade ADR 017/019; se `null`, herda `AreaContribution.id`.
  final ProductArea? area;
}

/// Cartão de Configurações dentro de uma aba (inclusive de outra área).
@immutable
class SettingsCardContribution {
  const SettingsCardContribution({
    required this.tab,
    required this.order,
    required this.builder,
    this.mdCols = 12,
    this.lgCols = 12,
    this.area,
  });

  final String tab;
  final int order;
  final WidgetBuilder builder;
  final int mdCols;
  final int lgCols;

  /// Área de visibilidade ADR 017/019; se `null`, herda `AreaContribution.id`.
  final ProductArea? area;
}

/// Aba de Configurações pronta para renderização, já filtrada e ordenada.
@immutable
class SettingsTabView {
  const SettingsTabView({required this.tab, required this.cards});

  final SettingsTabContribution tab;
  final List<SettingsCardContribution> cards;
}

/// Card de Dashboard registrado por uma área.
@immutable
class DashboardWidgetContribution {
  const DashboardWidgetContribution({
    required this.type,
    required this.builder,
  });

  final String type;
  final Widget Function(BuildContext context, Map<String, dynamic> config)
  builder;
}

/// Caminho de conexão de servidor remoto (LAN, VPN, Internet HTTPS).
@immutable
class ServerConnectPathContribution {
  const ServerConnectPathContribution({
    required this.id,
    required this.order,
    required this.title,
    required this.description,
    required this.icon,
    this.recommended = false,
    this.addressKinds = const <String>[],
    this.dialogBuilder,
    this.moveExistingLabel,
    this.moveExistingIcon,
  });

  final String id;
  final int order;
  final String title;
  final String description;
  final IconData icon;
  final bool recommended;
  final List<String> addressKinds;
  final Widget Function(
    BuildContext context, {
    String? serverName,
    int? agentId,
    VoidCallback? onDone,
  })?
  dialogBuilder;
  final String? moveExistingLabel;
  final IconData? moveExistingIcon;
}

/// Aba plugável na tela de Backup (`BackupPage`).
@immutable
class BackupTabContribution {
  const BackupTabContribution({
    required this.id,
    required this.title,
    required this.icon,
    required this.order,
    required this.builder,
    this.requiresAdmin = true,
  });

  final String id;
  final String title;
  final IconData icon;
  final int order;
  final WidgetBuilder builder;
  final bool requiresAdmin;
}

/// Aba plugável na tela de Detalhe de Dispositivo (`DeviceDetailPage`).
@immutable
class DeviceDetailTabContribution {
  const DeviceDetailTabContribution({
    required this.id,
    required this.title,
    required this.icon,
    required this.order,
    required this.builder,
    this.area = ProductArea.core,
    this.tone,
    this.visibleWhen,
  });

  final String id;
  final String title;
  final IconData icon;
  final int order;
  final Widget Function(BuildContext context, int deviceId) builder;
  final ProductArea area;
  final SemanticTone? tone;
  final bool Function(int deviceId)? visibleWhen;
}

/// O alerta, como uma ação de outra feature o recebe — tipo neutro: quem
/// declara a ação (a IA, por exemplo) não importa o modelo do Núcleo.
@immutable
class AlertActionTarget {
  const AlertActionTarget({
    required this.alertId,
    required this.title,
    required this.status,
    this.deviceId,
    this.monitorId,
    this.data,
  });

  final int alertId;
  final String title;
  final String status;
  final int? deviceId;
  final int? monitorId;

  /// O `data` do alerta (resumo da IA, triagem, silêncio…).
  final Map<String, dynamic>? data;
}

/// Ação que outra feature oferece no cartão de alerta (ADR 017): o Núcleo
/// mostra o botão sem conhecer quem o atende.
@immutable
class AlertActionContribution {
  const AlertActionContribution({
    required this.id,
    required this.label,
    required this.icon,
    required this.tone,
    required this.area,
    required this.onPressed,
  });

  final String id;
  final String Function(AlertActionTarget target) label;
  final IconData icon;
  final SemanticTone tone;

  /// Área que precisa estar visível para a ação aparecer.
  final ProductArea area;
  final void Function(
    BuildContext context,
    WidgetRef ref,
    AlertActionTarget target,
  )
  onPressed;
}

typedef NavActionHandler = void Function(BuildContext context, WidgetRef ref);
typedef AreaSseRoutesBuilder = Map<String, SseRoute> Function(Ref ref);

/// Contrato declarativo que cada área do produto (ADR 017) entrega ao app.
/// Cada worker de domínio edita apenas o seu `lib/areas/<area>_area.dart`
/// e `lib/features/<area>/`, sem conflito de arquivos.
@immutable
class AreaContribution {
  const AreaContribution({
    required this.id,
    this.routes = const <RouteBase>[],
    this.routeMeta = const <String, AreaRouteMeta>{},
    this.nav = const <NavEntry>[],
    this.navActions = const <NavAction, NavActionHandler>{},
    this.sseBuilder,
    this.extraSseBuilders = const <ProductArea, AreaSseRoutesBuilder>{},
    this.widgets = const <String, DashboardWidgetContribution>{},
    this.aiScreens = const <String, AiScreenContribution>{},
    this.settingsTabs = const <SettingsTabContribution>[],
    this.settingsCards = const <SettingsCardContribution>[],
    this.serverConnectPaths = const <ServerConnectPathContribution>[],
    this.backupTabs = const <BackupTabContribution>[],
    this.deviceDetailTabs = const <DeviceDetailTabContribution>[],
    this.alertActions = const <AlertActionContribution>[],
    this.shellOverlays = const <WidgetBuilder>[],
    this.extraShellOverlays = const <ProductArea, List<WidgetBuilder>>{},
  });

  final ProductArea id;
  final List<RouteBase> routes;

  /// Mapeia prefixo de caminho (ex.: `'/devices'`, `'/docker'`) → metadados de guard/layout.
  final Map<String, AreaRouteMeta> routeMeta;
  final List<NavEntry> nav;
  final Map<NavAction, NavActionHandler> navActions;
  final AreaSseRoutesBuilder? sseBuilder;

  /// Builders SSE extras com gating independente por área ADR 017
  /// (ex.: rotas SSE de IA sob `ProductArea.core` ou Plugins sob `ProductArea.network`
  /// declaradas a partir de `vpn_area.dart`).
  final Map<ProductArea, AreaSseRoutesBuilder> extraSseBuilders;
  final Map<String, DashboardWidgetContribution> widgets;
  final Map<String, AiScreenContribution> aiScreens;
  final List<SettingsTabContribution> settingsTabs;
  final List<SettingsCardContribution> settingsCards;
  final List<ServerConnectPathContribution> serverConnectPaths;
  final List<BackupTabContribution> backupTabs;
  final List<DeviceDetailTabContribution> deviceDetailTabs;
  final List<AlertActionContribution> alertActions;

  /// Widgets globais montados dentro do shell sob gating de `this.id`.
  final List<WidgetBuilder> shellOverlays;

  /// Widgets globais extras com gating independente por área ADR 017
  /// (ex.: `DeviceAccessHost` ou `AiApprovalNotice` sob `ProductArea.core`).
  final Map<ProductArea, List<WidgetBuilder>> extraShellOverlays;
}

/// Plugins de frota exibidos no grupo dinâmico "Aplicativos" do menu.
final pluginAppsNavProvider = StateProvider<List<NavApp>>(
  (ref) => const <NavApp>[],
);

/// Estado de abertura do drawer lateral do Assistente IA no shell.
final aiDrawerOpenProvider = StateProvider<bool>((ref) => false);

/// Builder plugável do conteúdo do Assistente IA (fornecido pela área Core).
final aiDrawerBuilderProvider = StateProvider<WidgetBuilder?>((ref) => null);

/// Um console (compilação, deploy, container) a mandar para a IA analisar.
@immutable
class AiLogAnalysisRequest {
  const AiLogAnalysisRequest({
    required this.source,
    required this.content,
    this.chatPrompt,
  });

  /// De onde vem o console ("Compilação #12 · api").
  final String source;

  /// O console inteiro, no momento do pedido.
  final String content;

  /// Pergunta para "Continuar no chat", quando a tela tem uma.
  final String? chatPrompt;
}

/// Abre a análise de console da IA (fornecido pelo Assistente, área Core).
typedef AiLogAnalysisLauncher =
    Future<void> Function(BuildContext context, AiLogAnalysisRequest request);

/// Lançador plugável da análise de console; `null` sem o Assistente montado.
final aiLogAnalysisLauncherProvider = StateProvider<AiLogAnalysisLauncher?>(
  (ref) => null,
);

/// Gatilho plugável do Assistente Inicial (fornecido pela área Core).
final onboardingTriggerProvider =
    StateProvider<void Function(BuildContext context)?>((ref) => null);

/// Tela placeholder limpa usada enquanto o worker da área ainda não ligou a tela final.
class FeaturePlaceholderPage extends StatelessWidget {
  const FeaturePlaceholderPage({
    super.key,
    required this.title,
    required this.area,
    this.description,
    this.icon = Icons.construction_outlined,
  });

  final String title;
  final ProductArea area;
  final String? description;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: SectionCard(
          title: title,
          subtitle: 'Área ${area.defaultTitle}',
          icon: icon,
          trailing: SemanticChip(
            label: area.defaultTitle,
            tone: SemanticTone.info,
            compact: true,
          ),
          child: Text(
            description ??
                'Módulo registrado no roteador da área ${area.defaultTitle}. '
                    'A implementação completa será conectada nesta rota.',
            style: context.textTheme.bodyMedium?.copyWith(
              color: context.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      ),
    );
  }
}

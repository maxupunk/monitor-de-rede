import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/ai/pages/ai_chat_page.dart';
import 'package:netmonitor_app/features/ai/pages/decider_benchmark_page.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_approval_notice.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_chat_drawer.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_data_export_card.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_proactive_widgets.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_settings_card.dart';
import 'package:netmonitor_app/features/ai/widgets/decider_settings_card.dart';
import 'package:netmonitor_app/features/plugins/pages/plugin_app_page.dart';
import 'package:netmonitor_app/features/plugins/pages/plugins_page.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_plugins_tab.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_approval_dialog.dart';
import 'package:netmonitor_app/features/vpn/pages/vpn_devices_page.dart';
import 'package:netmonitor_app/features/vpn/pages/vpn_server_page.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/features/vpn/widgets/device_vpn_tab.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_server_connect.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Contribuição do Bucket 4 (VPN WireGuard, Assistente IA & Plugins de Dispositivo).
///
/// Preserva estritamente as áreas ADR 017 / ADR 019:
/// - VPN (`/vpn`, `/vpn/devices`): `ProductArea.vpn`
/// - Assistente IA (`/ai-chat`, `/ai-benchmark`, aba `ia` de Configurações): `ProductArea.core`
/// - Plugins (`/plugins`, `/apps/:id`, aba Plugins do dispositivo): `ProductArea.network`
final AreaContribution vpnArea = AreaContribution(
  id: ProductArea.vpn,
  routes: <RouteBase>[
    GoRoute(
      path: '/vpn',
      name: 'vpn-server',
      builder: (context, state) => const VpnServerPage(),
      routes: <RouteBase>[
        GoRoute(
          path: 'devices',
          name: 'vpn-devices',
          builder: (context, state) => const VpnDevicesPage(),
        ),
      ],
    ),
    GoRoute(
      path: '/ai-chat',
      name: 'ai-chat',
      builder: (context, state) => const AiChatPage(),
    ),
    GoRoute(
      path: '/ai-benchmark',
      name: 'decider-benchmark',
      builder: (context, state) =>
          DeciderBenchmarkPage(initialTab: state.uri.queryParameters['tab']),
    ),
    GoRoute(
      path: '/plugins',
      name: 'plugins',
      builder: (context, state) => const PluginsPage(),
    ),
    GoRoute(
      path: '/apps/:id',
      name: 'plugin-app',
      builder: (context, state) {
        final id = int.tryParse(state.pathParameters['id'] ?? '') ?? 0;
        return PluginAppPage(pluginId: id);
      },
    ),
  ],
  routeMeta: const <String, AreaRouteMeta>{
    '/vpn': AreaRouteMeta(
      area: ProductArea.vpn,
      aiScreen: AiScreenContribution(
        id: 'vpn',
        title: 'VPN',
        description: 'Servidor WireGuard e peers conectados.',
      ),
    ),
    '/ai-chat': AreaRouteMeta(area: ProductArea.core, fullBleed: true),
    '/ai-benchmark': AreaRouteMeta(area: ProductArea.core),
    '/plugins': AreaRouteMeta(
      area: ProductArea.network,
      requiresAdmin: true,
      aiScreen: AiScreenContribution(
        id: 'plugins',
        title: 'Plugins',
        description:
            'Biblioteca de plugins de dispositivo e revisão de quarentena.',
      ),
    ),
    '/apps': AreaRouteMeta(
      area: ProductArea.network,
      aiScreen: AiScreenContribution(
        id: 'plugin-app',
        title: 'App de plugin',
        description: 'Gerenciamento em frota por aplicativo de plugin.',
      ),
    ),
  },
  nav: const <NavEntry>[
    NavEntry(
      area: ProductArea.vpn,
      section: NavSectionId.vpn,
      order: 10,
      item: NavItemSpec(
        title: 'VPN WireGuard',
        icon: Icons.vpn_lock_outlined,
        to: '/vpn',
      ),
    ),
  ],
  aiScreens: const <String, AiScreenContribution>{
    'vpn': AiScreenContribution(
      id: 'vpn',
      title: 'VPN',
      description: 'Servidor WireGuard e peers conectados.',
    ),
    'plugins': AiScreenContribution(
      id: 'plugins',
      title: 'Plugins',
      description: 'Biblioteca de plugins de dispositivo.',
    ),
    'plugin-app': AiScreenContribution(
      id: 'plugin-app',
      title: 'App de plugin',
      description: 'Aplicativo de frota de um plugin.',
    ),
  },
  // 1. SSE da área VPN (`vpn:peers_updated`)
  sseBuilder: (ref) => <String, SseRoute>{
    'vpn:peers_updated': SseRoute(
      handle: (data, _, _) =>
          ref.read(vpnProvider.notifier).applyRealtimePeers(data),
    ),
  },
  // 2. SSE das áreas Core (IA) e Network (Plugins), com gating independente ADR 017
  extraSseBuilders: <ProductArea, AreaSseRoutesBuilder>{
    ProductArea.core: (ref) => <String, SseRoute>{
      'stream:connected': SseRoute(
        handle: (_, _, _) => ref.read(aiApprovalsProvider.notifier).reset(),
      ),
      'ai:approval_pending': SseRoute(
        handle: (data, _, _) =>
            ref.read(aiApprovalsProvider.notifier).applyPending(data),
      ),
      'ai:approval_resolved': SseRoute(
        handle: (data, _, _) {
          ref.read(aiApprovalsProvider.notifier).applyResolved(data);
          ref.read(aiChatProvider.notifier).applyApprovalDecision(data);
        },
      ),
      'decider:model_state': SseRoute(
        handle: (data, _, _) =>
            ref.read(aiDeciderProvider.notifier).applyModelState(data),
      ),
      'alert:decider_triage': SseRoute(
        handle: (data, _, _) =>
            ref.read(aiAlertFeedbackProvider.notifier).applyDeciderTriage(data),
      ),
      'alert:ai_summary': SseRoute(
        handle: (data, _, _) =>
            ref.read(aiAlertFeedbackProvider.notifier).applyAiSummary(data),
      ),
    },
    ProductArea.network: (ref) => <String, SseRoute>{
      'stream:connected': SseRoute(
        handle: (_, _, _) =>
            ref.read(pluginsProvider.notifier).resetApprovals(),
      ),
      'plugin:run_started': SseRoute(
        handle: (data, _, _) =>
            ref.read(pluginsProvider.notifier).applyRunStarted(data),
      ),
      'plugin:run_output': SseRoute(
        handle: (data, _, _) {
          ref.read(pluginsProvider.notifier).applyRunOutput(data);
          ref.read(pluginAppsProvider.notifier).applyRunStep(data);
        },
      ),
      'plugin:run_finished': SseRoute(
        handle: (data, _, _) =>
            ref.read(pluginsProvider.notifier).applyRunFinished(data),
      ),
      'plugin:run_progress': SseRoute(
        ephemeral: true,
        handle: (data, _, _) =>
            ref.read(pluginsProvider.notifier).applyRunProgress(data),
      ),
      'plugin:run_step': SseRoute(
        ephemeral: true,
        handle: (data, _, _) =>
            ref.read(pluginAppsProvider.notifier).applyRunStep(data),
      ),
      'plugin:approval_required': SseRoute(
        handle: (data, _, _) =>
            ref.read(pluginsProvider.notifier).applyApprovalRequired(data),
      ),
      'plugin:approval_resolved': SseRoute(
        handle: (data, _, _) =>
            ref.read(pluginsProvider.notifier).applyApprovalResolved(data),
      ),
      'plugin:batch_updated': SseRoute(
        handle: (data, _, _) =>
            ref.read(pluginAppsProvider.notifier).applyBatchUpdated(data),
      ),
      'plugin:apps_changed': SseRoute(
        handle: (_, _, _) => ref.read(pluginAppsProvider.notifier).fetchApps(),
      ),
    },
  },
  // 3. Cartões da aba `ia` em Configurações (sempre `ProductArea.core`)
  settingsCards: <SettingsCardContribution>[
    SettingsCardContribution(
      tab: 'ia',
      order: 10,
      area: ProductArea.core,
      builder: (_) => const AiSettingsCard(),
    ),
    SettingsCardContribution(
      tab: 'ia',
      order: 20,
      area: ProductArea.core,
      builder: (_) => const DeciderSettingsCard(),
    ),
    SettingsCardContribution(
      tab: 'ia',
      order: 30,
      area: ProductArea.core,
      builder: (_) => const AiDataExportCard(),
    ),
  ],
  // 4. Abas plugáveis em `/devices/:id` (`DeviceDetailTabRegistry`)
  deviceDetailTabs: <DeviceDetailTabContribution>[
    DeviceDetailTabContribution(
      id: 'vpn',
      title: 'VPN',
      icon: Icons.vpn_lock_outlined,
      order: 70,
      area: ProductArea.vpn,
      builder: (_, deviceId) => DeviceVpnTab(deviceId: deviceId),
    ),
    DeviceDetailTabContribution(
      id: 'plugins',
      title: 'Plugins',
      icon: Icons.extension_outlined,
      order: 80,
      area: ProductArea.network,
      tone: SemanticTone.plugin,
      builder: (_, deviceId) => DevicePluginsTab(
        deviceId: deviceId,
        aiActionBuilder: (_, ref, id, platform) {
          final aiEnabled =
              ref.watch(aiSettingsProvider).settings?.enabled ?? true;
          if (!aiEnabled) return null;
          return AiAskButton(
            label: 'Criar ou usar com IA',
            prompt:
                'Quero consultar ou criar um plugin para o equipamento #$id ($platform).',
            contextData: AiChatContext(deviceId: id),
            compact: true,
          );
        },
      ),
    ),
  ],
  // 5. Caminho recomendado de conexão de servidor remoto pela VPN
  serverConnectPaths: <ServerConnectPathContribution>[
    ServerConnectPathContribution(
      id: 'vpn',
      order: 20,
      title: 'Pela VPN',
      description:
          'Servidor em outra rede. Um script só sobe o túnel WireGuard até o IP público desta central e instala o agente, que passa a falar com ela apenas por dentro do túnel.',
      icon: Icons.vpn_lock_outlined,
      recommended: true,
      moveExistingLabel: 'Mover para a VPN',
      moveExistingIcon: Icons.vpn_lock_outlined,
      dialogBuilder: (context, {serverName, agentId, onDone}) =>
          VpnServerConnectDialog(
            serverName: serverName,
            agentId: agentId,
            onDone: onDone,
          ),
    ),
  ],
  // 6. Overlays globais (IA sob `core`, Aprovação de Plugin sob `network`)
  extraShellOverlays: <ProductArea, List<WidgetBuilder>>{
    ProductArea.core: <WidgetBuilder>[
      (_) => const AiAndAppsShellBootstrapOverlay(),
      (_) => const AiApprovalNoticeOverlay(),
    ],
    ProductArea.network: <WidgetBuilder>[
      (_) => const PluginAppsBootstrapOverlay(),
      (_) => const PluginApprovalOverlay(),
    ],
  },
);

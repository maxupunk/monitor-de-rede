import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/navigation/build_nav_sections.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/pwa/pwa_install_service.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/shared/widgets/menu_customize_sheet.dart';
import 'package:netmonitor_app/shared/widgets/nested_panel.dart';
import 'package:netmonitor_app/shared/widgets/semantic_chip.dart';
import 'package:netmonitor_app/shared/widgets/shell_navigation_bars.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'menu_customize_sheet.dart';
export 'shell_navigation_bars.dart';

/// Shell principal responsivo em Material Design 3:
/// - Compact (< 600px): AppBar + `NavigationBar` inferior + Drawer "Mais".
/// - Medium (600–1199px): AppBar + `NavigationRail` + Drawer completo.
/// - Expanded (>= 1200px): AppBar + `NavigationDrawer` permanente.
class AdaptiveAppShell extends ConsumerStatefulWidget {
  const AdaptiveAppShell({
    super.key,
    required this.location,
    required this.child,
  });

  final String location;
  final Widget child;

  @override
  ConsumerState<AdaptiveAppShell> createState() => _AdaptiveAppShellState();
}

class _AdaptiveAppShellState extends ConsumerState<AdaptiveAppShell> {
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();
  final Set<String> _expandedGroups = <String>{};
  SseService? _sseNotifier;

  String? _lastAutoExpandedLocation;

  @override
  void initState() {
    super.initState();
    _sseNotifier = ref.read(sseServiceProvider.notifier);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      // Garante rotas SSE das áreas registradas antes de conectar o stream.
      ref.read(areaRealtimeCoordinatorProvider);
      ref.read(authProvider.notifier).fetchMe();
      _sseNotifier?.connect();
      _syncCurrentArea(widget.location);
    });
  }

  @override
  void didUpdateWidget(covariant AdaptiveAppShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.location != widget.location) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        _syncCurrentArea(widget.location);
      });
    }
  }

  @override
  void dispose() {
    try {
      _sseNotifier?.disconnect();
    } catch (_) {
      // Container já encerrado durante troca de ProviderScope.
    }
    super.dispose();
  }

  void _syncCurrentArea(String location) {
    final meta = resolveRouteMeta(location);
    final area = meta?.area ?? ProductArea.core;
    ref.read(authProvider.notifier).setCurrentArea(area);
  }

  @override
  Widget build(BuildContext context) {
    // Mantém o coordenador de rotas SSE ativo durante todo o ciclo do shell.
    ref.watch(areaRealtimeCoordinatorProvider);

    final auth = ref.watch(authProvider);
    final visibleAreas = ref.watch(visibleAreasProvider);
    final realtime = ref.watch(sseServiceProvider);
    final pwa = ref.watch(pwaInstallProvider);
    final apps = ref.watch(pluginAppsNavProvider);
    final aiOpen = ref.watch(aiDrawerOpenProvider);
    final aiBuilder = ref.watch(aiDrawerBuilderProvider);

    // Em telas estreitas (< 900px), espelha `aiDrawerOpenProvider` no `endDrawer`
    // para que ações programáticas ("Perguntar à IA", aprovação) abram o painel.
    ref.listen<bool>(aiDrawerOpenProvider, (prev, next) {
      if (!mounted) return;
      final isNarrow = MediaQuery.sizeOf(context).width < 900;
      if (!isNarrow) return;
      final scaffold = _scaffoldKey.currentState;
      if (scaffold == null) return;
      if (next && !scaffold.isEndDrawerOpen) {
        scaffold.openEndDrawer();
      } else if (!next && scaffold.isEndDrawerOpen) {
        scaffold.closeEndDrawer();
      }
    });

    final navCtx = NavContext(
      isAdmin: auth.isAdmin,
      apps: apps,
      canInstallPwa: pwa.showInstallAction,
      enabledAreas: visibleAreas,
      menu: auth.menuPrefs,
    );
    final baseSections = buildBaseSections(
      NavContext(
        isAdmin: auth.isAdmin,
        apps: apps,
        canInstallPwa: pwa.showInstallAction,
        enabledAreas: visibleAreas,
      ),
      allNavEntries,
    );
    final navSections = buildNavSections(navCtx, allNavEntries);

    if (_lastAutoExpandedLocation != widget.location) {
      _lastAutoExpandedLocation = widget.location;
      final activeGroup = groupOfPath(navSections, widget.location);
      if (activeGroup != null) {
        _expandedGroups.add(activeGroup);
      }
    }

    final routeMeta = resolveRouteMeta(widget.location);
    final fullBleed = routeMeta?.fullBleed ?? false;
    final width = MediaQuery.sizeOf(context).width;
    final isCompact = width < 600;
    final isExpanded = width >= 1200;

    final drawerWidget = ShellNavigationDrawer(
      sections: navSections,
      currentPath: widget.location,
      expandedGroups: _expandedGroups,
      onToggleGroup: (title) {
        setState(() {
          if (_expandedGroups.contains(title)) {
            _expandedGroups.remove(title);
          } else {
            _expandedGroups.add(title);
          }
        });
      },
      onNavigate: (path) {
        if (!isExpanded && _scaffoldKey.currentState?.isDrawerOpen == true) {
          Navigator.of(context).pop();
        }
        context.go(path);
      },
      onAction: (action) {
        if (!isExpanded && _scaffoldKey.currentState?.isDrawerOpen == true) {
          Navigator.of(context).pop();
        }
        _runNavAction(action);
      },
      onCustomizeMenu: () {
        if (!isExpanded && _scaffoldKey.currentState?.isDrawerOpen == true) {
          Navigator.of(context).pop();
        }
        showMenuCustomizeSheet(context, ref, baseSections: baseSections);
      },
    );

    final bodyContent = Column(
      children: <Widget>[
        if (!auth.canWrite && !fullBleed)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
            child: NestedPanel(
              tone: SemanticTone.info,
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              child: Row(
                children: <Widget>[
                  Icon(
                    Icons.visibility_outlined,
                    size: 18,
                    color: context.semanticColors
                        .resolve(SemanticTone.info, context.colorScheme)
                        .onContainer,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      'Modo somente visualização: você pode consultar os dados, mas alterações estão desativadas.',
                      style: context.textTheme.bodySmall?.copyWith(
                        fontWeight: FontWeight.w600,
                        color: context.semanticColors
                            .resolve(SemanticTone.info, context.colorScheme)
                            .onContainer,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        Expanded(
          child: fullBleed
              ? widget.child
              : Align(
                  alignment: Alignment.topCenter,
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 1600),
                    child: Padding(
                      padding: EdgeInsets.symmetric(
                        horizontal: isCompact ? 12 : 20,
                        vertical: isCompact ? 12 : 16,
                      ),
                      child: widget.child,
                    ),
                  ),
                ),
        ),
      ],
    );

    final mainWithAi = Row(
      children: <Widget>[
        if (isExpanded)
          SizedBox(
            width: 276,
            child: Material(
              elevation: 2,
              color: context.colorScheme.surface,
              child: drawerWidget,
            ),
          ),
        if (!isCompact && !isExpanded)
          ShellNavigationRail(
            currentPath: widget.location,
            onNavigate: (path) => context.go(path),
            onOpenMore: () => _scaffoldKey.currentState?.openDrawer(),
          ),
        Expanded(child: bodyContent),
        if (aiOpen && width >= 900)
          SizedBox(
            width: 380,
            child: Material(
              elevation: 4,
              color: context.colorScheme.surface,
              child: _buildAiDrawerPanel(context, aiBuilder),
            ),
          ),
      ],
    );

    return Scaffold(
      key: _scaffoldKey,
      drawer: isExpanded ? null : Drawer(width: 304, child: drawerWidget),
      endDrawer: width < 900 && aiBuilder != null
          ? Drawer(width: 360, child: _buildAiDrawerPanel(context, aiBuilder))
          : null,
      onEndDrawerChanged: (isOpened) {
        if (width < 900 && ref.read(aiDrawerOpenProvider) != isOpened) {
          ref.read(aiDrawerOpenProvider.notifier).state = isOpened;
        }
      },
      appBar: AppBar(
        automaticallyImplyLeading: false,
        leading: isExpanded
            ? null
            : IconButton(
                tooltip: 'Abrir menu',
                icon: const Icon(Icons.menu),
                onPressed: () => _scaffoldKey.currentState?.openDrawer(),
              ),
        titleSpacing: isExpanded ? 16 : 4,
        title: Row(
          children: <Widget>[
            if (!isExpanded) ...<Widget>[
              Container(
                width: 32,
                height: 32,
                decoration: BoxDecoration(
                  color: context.colorScheme.primary,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(
                  Icons.hub_outlined,
                  size: 18,
                  color: context.colorScheme.onPrimary,
                ),
              ),
              const SizedBox(width: 10),
              const Flexible(
                child: Text('NetMonitor', overflow: TextOverflow.ellipsis),
              ),
            ],
          ],
        ),
        actions: <Widget>[
          Padding(
            padding: const EdgeInsets.only(right: 6),
            child: SemanticChip(
              label: realtime.isConnected
                  ? (isCompact ? 'Ao vivo' : 'Tempo Real Ativo')
                  : (isCompact ? 'SSE...' : 'Conectando SSE...'),
              tone: realtime.isConnected
                  ? SemanticTone.success
                  : SemanticTone.warning,
              icon: realtime.isConnected
                  ? Icons.radio_button_checked
                  : Icons.sync,
              compact: true,
            ),
          ),
          IconButton(
            tooltip: 'Assistente IA',
            icon: Icon(
              Icons.smart_toy_outlined,
              color: context.colorScheme.primary,
            ),
            onPressed: () {
              if (aiBuilder == null) {
                context.go('/ai-chat');
                return;
              }
              if (width < 900) {
                _scaffoldKey.currentState?.openEndDrawer();
              } else {
                ref.read(aiDrawerOpenProvider.notifier).state = !aiOpen;
              }
            },
          ),
          ShellUserAccountMenu(
            auth: auth,
            onOpenOnboarding: () {
              final trigger = ref.read(onboardingTriggerProvider);
              if (trigger != null) {
                trigger(context);
              } else {
                context.go('/settings?tab=geral');
              }
            },
            onLogout: () async {
              await ref.read(authProvider.notifier).logout();
              if (context.mounted) {
                context.go('/login');
              }
            },
          ),
          const SizedBox(width: 4),
        ],
      ),
      body: Stack(
        children: <Widget>[
          mainWithAi,
          for (final overlayBuilder in visibleShellOverlays(visibleAreas))
            overlayBuilder(context),
        ],
      ),
      bottomNavigationBar: isCompact
          ? ShellBottomBar(
              currentPath: widget.location,
              onNavigate: (path) => context.go(path),
              onOpenMore: () => _scaffoldKey.currentState?.openDrawer(),
            )
          : null,
    );
  }

  Widget _buildAiDrawerPanel(BuildContext context, WidgetBuilder? builder) {
    if (builder != null) return builder(context);
    return Column(
      children: <Widget>[
        ListTile(
          leading: const Icon(Icons.smart_toy_outlined),
          title: const Text('Assistente IA'),
          trailing: IconButton(
            icon: const Icon(Icons.close),
            onPressed: () =>
                ref.read(aiDrawerOpenProvider.notifier).state = false,
          ),
        ),
        const Divider(height: 1),
        Expanded(
          child: Center(
            child: FilledButton.icon(
              onPressed: () {
                ref.read(aiDrawerOpenProvider.notifier).state = false;
                context.go('/ai-chat');
              },
              icon: const Icon(Icons.chat_outlined),
              label: const Text('Abrir chat completo'),
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _runNavAction(NavAction action) async {
    if (action == NavAction.installPwa) {
      final pwa = ref.read(pwaInstallProvider);
      if (pwa.isIosSafari && !pwa.canInstall) {
        if (!mounted) return;
        await showDialog<void>(
          context: context,
          builder: (ctx) => AlertDialog(
            title: const Text('Instalar no iOS (Safari)'),
            content: const Text(
              'Toque no botão Compartilhar na barra inferior do Safari, '
              'selecione "Adicionar à Tela de Início" e confirme em "Adicionar".',
            ),
            actions: <Widget>[
              FilledButton(
                onPressed: () => Navigator.of(ctx).pop(),
                child: const Text('Entendi'),
              ),
            ],
          ),
        );
      } else {
        await ref.read(pwaInstallProvider.notifier).promptInstall();
      }
      return;
    }

    final handled = dispatchAreaNavAction(action, context, ref);
    if (!handled && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Ferramenta disponível no módulo de Rede.'),
        ),
      );
    }
  }
}

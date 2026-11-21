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
import 'package:netmonitor_app/core/utils/theme_preference.dart';
import 'package:netmonitor_app/shared/widgets/adaptive_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/nested_panel.dart';
import 'package:netmonitor_app/shared/widgets/semantic_chip.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      // Garante rotas SSE das áreas registradas antes de conectar o stream.
      ref.read(areaRealtimeCoordinatorProvider);
      ref.read(authProvider.notifier).fetchMe();
      ref.read(sseServiceProvider.notifier).connect();
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
    ref.read(sseServiceProvider.notifier).disconnect();
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

    // Se a área aberta desligar ou perder acesso em tempo real (SSE), redireciona.
    ref.listen<Set<ProductArea>>(visibleAreasProvider, (_, next) {
      final meta = resolveRouteMeta(widget.location);
      if (meta == null || meta.area == ProductArea.core) return;
      final areasState = ref.read(areasProvider);
      final authState = ref.read(authProvider);
      if (!areasState.isEnabled(meta.area)) {
        final encodedFrom = Uri.encodeQueryComponent(widget.location);
        context.go('/area-desligada/${meta.area.id}?from=$encodedFrom');
      } else if (!authState.hasAccess(meta.area)) {
        final encodedFrom = Uri.encodeQueryComponent(widget.location);
        context.go('/area-desligada/${meta.area.id}?from=$encodedFrom&motivo=acesso');
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

    final activeGroup = groupOfPath(navSections, widget.location);
    if (activeGroup != null) {
      _expandedGroups.add(activeGroup);
    }

    final routeMeta = resolveRouteMeta(widget.location);
    final fullBleed = routeMeta?.fullBleed ?? false;
    final width = MediaQuery.sizeOf(context).width;
    final isCompact = width < 600;
    final isExpanded = width >= 1200;

    final drawerWidget = _NetMonitorDrawer(
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
        _openMenuCustomizeSheet(baseSections, auth.menuPrefs);
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
          _NetMonitorRail(
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
                child: Text(
                  'NetMonitor',
                  overflow: TextOverflow.ellipsis,
                ),
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
              tone: realtime.isConnected ? SemanticTone.success : SemanticTone.warning,
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
          _UserAccountMenu(
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
          ? _NetMonitorBottomBar(
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
            onPressed: () => ref.read(aiDrawerOpenProvider.notifier).state = false,
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

  Future<void> _openMenuCustomizeSheet(
    List<NavSection> baseSections,
    MenuPrefs currentPrefs,
  ) async {
    final leaves = navLeaves(baseSections);
    final pinned = List<String>.from(currentPrefs.pinned);
    final hidden = Set<String>.from(currentPrefs.hidden);

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Personalizar menu',
      subtitle: 'Fixe atalhos no topo ou oculte itens pouco usados.',
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                for (final entry in leaves) ...<Widget>[
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: Icon(entry.leaf.icon, size: 20),
                    title: Text(entry.leaf.title),
                    subtitle: Text(entry.section),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        IconButton(
                          tooltip: pinned.contains(entry.leaf.to)
                              ? 'Remover dos fixados'
                              : 'Fixar no topo',
                          icon: Icon(
                            pinned.contains(entry.leaf.to)
                                ? Icons.push_pin
                                : Icons.push_pin_outlined,
                            color: pinned.contains(entry.leaf.to)
                                ? context.colorScheme.primary
                                : context.colorScheme.onSurfaceVariant,
                          ),
                          onPressed: () {
                            final path = entry.leaf.to!;
                            setModalState(() {
                              if (pinned.contains(path)) {
                                pinned.remove(path);
                              } else {
                                pinned.add(path);
                                hidden.remove(path);
                              }
                            });
                          },
                        ),
                        IconButton(
                          tooltip: hidden.contains(entry.leaf.to)
                              ? 'Mostrar no menu'
                              : 'Ocultar do menu',
                          icon: Icon(
                            hidden.contains(entry.leaf.to)
                                ? Icons.visibility_off
                                : Icons.visibility_outlined,
                            color: hidden.contains(entry.leaf.to)
                                ? context.colorScheme.error
                                : context.colorScheme.onSurfaceVariant,
                          ),
                          onPressed: () {
                            final path = entry.leaf.to!;
                            setModalState(() {
                              if (hidden.contains(path)) {
                                hidden.remove(path);
                              } else {
                                hidden.add(path);
                                pinned.remove(path);
                              }
                            });
                          },
                        ),
                      ],
                    ),
                  ),
                  const Divider(height: 1),
                ],
                const SizedBox(height: 16),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: <Widget>[
                    TextButton(
                      onPressed: () {
                        setModalState(() {
                          pinned.clear();
                          hidden.clear();
                        });
                      },
                      child: const Text('Restaurar padrão'),
                    ),
                    const SizedBox(width: 8),
                    FilledButton(
                      onPressed: () async {
                        await ref.read(authProvider.notifier).saveMenuPrefs(
                              MenuPrefs(
                                pinned: List<String>.unmodifiable(pinned),
                                hidden: List<String>.unmodifiable(hidden),
                              ),
                            );
                        if (sheetCtx.mounted) {
                          Navigator.of(sheetCtx).pop();
                        }
                      },
                      child: const Text('Salvar menu'),
                    ),
                  ],
                ),
              ],
            );
          },
        );
      },
    );
  }
}

class _NetMonitorBottomBar extends StatelessWidget {
  const _NetMonitorBottomBar({
    required this.currentPath,
    required this.onNavigate,
    required this.onOpenMore,
  });

  final String currentPath;
  final ValueChanged<String> onNavigate;
  final VoidCallback onOpenMore;

  int _selectedIndex() {
    if (currentPath == '/') return 0;
    if (currentPath.startsWith('/devices')) return 1;
    if (currentPath.startsWith('/alerts')) return 2;
    return 3;
  }

  @override
  Widget build(BuildContext context) {
    return NavigationBar(
      selectedIndex: _selectedIndex(),
      onDestinationSelected: (index) {
        switch (index) {
          case 0:
            onNavigate('/');
          case 1:
            onNavigate('/devices');
          case 2:
            onNavigate('/alerts');
          case 3:
            onOpenMore();
        }
      },
      destinations: const <NavigationDestination>[
        NavigationDestination(
          icon: Icon(Icons.dashboard_outlined),
          selectedIcon: Icon(Icons.dashboard),
          label: 'Dashboard',
        ),
        NavigationDestination(
          icon: Icon(Icons.devices_outlined),
          selectedIcon: Icon(Icons.devices),
          label: 'Dispositivos',
        ),
        NavigationDestination(
          icon: Icon(Icons.notifications_outlined),
          selectedIcon: Icon(Icons.notifications),
          label: 'Alertas',
        ),
        NavigationDestination(
          icon: Icon(Icons.menu),
          selectedIcon: Icon(Icons.menu_open),
          label: 'Mais',
        ),
      ],
    );
  }
}

class _NetMonitorRail extends StatelessWidget {
  const _NetMonitorRail({
    required this.currentPath,
    required this.onNavigate,
    required this.onOpenMore,
  });

  final String currentPath;
  final ValueChanged<String> onNavigate;
  final VoidCallback onOpenMore;

  int? _selectedIndex() {
    if (currentPath == '/') return 0;
    if (currentPath.startsWith('/devices')) return 1;
    if (currentPath.startsWith('/monitors')) return 2;
    if (currentPath.startsWith('/alerts')) return 3;
    if (currentPath.startsWith('/settings')) return 4;
    return null;
  }

  @override
  Widget build(BuildContext context) {
    return NavigationRail(
      selectedIndex: _selectedIndex(),
      labelType: NavigationRailLabelType.all,
      leading: IconButton(
        tooltip: 'Todas as seções',
        icon: const Icon(Icons.apps),
        onPressed: onOpenMore,
      ),
      onDestinationSelected: (index) {
        switch (index) {
          case 0:
            onNavigate('/');
          case 1:
            onNavigate('/devices');
          case 2:
            onNavigate('/monitors');
          case 3:
            onNavigate('/alerts');
          case 4:
            onNavigate('/settings');
        }
      },
      destinations: const <NavigationRailDestination>[
        NavigationRailDestination(
          icon: Icon(Icons.dashboard_outlined),
          selectedIcon: Icon(Icons.dashboard),
          label: Text('Dashboard'),
        ),
        NavigationRailDestination(
          icon: Icon(Icons.devices_outlined),
          selectedIcon: Icon(Icons.devices),
          label: Text('Dispositivos'),
        ),
        NavigationRailDestination(
          icon: Icon(Icons.monitor_heart_outlined),
          selectedIcon: Icon(Icons.monitor_heart),
          label: Text('Monitores'),
        ),
        NavigationRailDestination(
          icon: Icon(Icons.notifications_outlined),
          selectedIcon: Icon(Icons.notifications),
          label: Text('Alertas'),
        ),
        NavigationRailDestination(
          icon: Icon(Icons.settings_outlined),
          selectedIcon: Icon(Icons.settings),
          label: Text('Config.'),
        ),
      ],
    );
  }
}

class _NetMonitorDrawer extends StatelessWidget {
  const _NetMonitorDrawer({
    required this.sections,
    required this.currentPath,
    required this.expandedGroups,
    required this.onToggleGroup,
    required this.onNavigate,
    required this.onAction,
    required this.onCustomizeMenu,
  });

  final List<NavSection> sections;
  final String currentPath;
  final Set<String> expandedGroups;
  final ValueChanged<String> onToggleGroup;
  final ValueChanged<String> onNavigate;
  final ValueChanged<NavAction> onAction;
  final VoidCallback onCustomizeMenu;

  bool _isActive(String? to) {
    if (to == null) return false;
    if (to == '/') return currentPath == '/';
    return currentPath == to || currentPath.startsWith('$to/');
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;

    return SafeArea(
      child: Column(
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              children: <Widget>[
                Container(
                  width: 42,
                  height: 42,
                  decoration: BoxDecoration(
                    color: scheme.primary,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Icon(
                    Icons.hub_outlined,
                    color: scheme.onPrimary,
                    size: 24,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        'NetMonitor',
                        style: context.textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.w800,
                          color: scheme.onSurface,
                        ),
                      ),
                      Text(
                        'Monitor de Rede v1.0',
                        style: context.textTheme.bodySmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          Expanded(
            child: ListView.builder(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
              itemCount: sections.length,
              itemBuilder: (context, index) {
                final section = sections[index];
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Padding(
                      padding: EdgeInsets.fromLTRB(12, index == 0 ? 4 : 14, 12, 4),
                      child: Text(
                        section.title.toUpperCase(),
                        style: TextStyle(
                          fontSize: 11,
                          letterSpacing: 0.9,
                          fontWeight: FontWeight.w800,
                          color: scheme.primary,
                        ),
                      ),
                    ),
                    for (final item in section.items)
                      if (!item.isGroup)
                        _DrawerNavTile(
                          title: item.title,
                          icon: item.icon,
                          selected: _isActive(item.to),
                          onTap: () {
                            if (item.action != null) {
                              onAction(item.action!);
                            } else if (item.to != null) {
                              onNavigate(item.to!);
                            }
                          },
                        )
                      else ...<Widget>[
                        _DrawerNavTile(
                          title: item.title,
                          icon: item.icon,
                          selected: false,
                          trailing: Icon(
                            expandedGroups.contains(item.title)
                                ? Icons.expand_less
                                : Icons.expand_more,
                            size: 18,
                          ),
                          onTap: () => onToggleGroup(item.title),
                        ),
                        if (expandedGroups.contains(item.title))
                          for (final child in item.children!)
                            Padding(
                              padding: const EdgeInsets.only(left: 16),
                              child: _DrawerNavTile(
                                title: child.title,
                                icon: child.icon,
                                selected: _isActive(child.to),
                                onTap: () {
                                  if (child.action != null) {
                                    onAction(child.action!);
                                  } else if (child.to != null) {
                                    onNavigate(child.to!);
                                  }
                                },
                              ),
                            ),
                      ],
                  ],
                );
              },
            ),
          ),
          const Divider(height: 1),
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 6),
            child: SizedBox(
              width: double.infinity,
              child: FilledButton.tonalIcon(
                onPressed: onCustomizeMenu,
                icon: const Icon(Icons.tune, size: 18),
                label: const Text('Personalizar menu'),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 10),
            child: Text(
              '© 2026 Master Sistemas',
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _DrawerNavTile extends StatelessWidget {
  const _DrawerNavTile({
    required this.title,
    required this.icon,
    required this.selected,
    required this.onTap,
    this.trailing,
  });

  final String title;
  final IconData icon;
  final bool selected;
  final VoidCallback onTap;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: Material(
        color: selected ? scheme.primaryContainer : Colors.transparent,
        borderRadius: BorderRadius.circular(10),
        child: InkWell(
          borderRadius: BorderRadius.circular(10),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: <Widget>[
                Icon(
                  icon,
                  size: 20,
                  color: selected ? scheme.onPrimaryContainer : scheme.onSurfaceVariant,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    title,
                    style: TextStyle(
                      fontSize: 13.5,
                      fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
                      color: selected ? scheme.onPrimaryContainer : scheme.onSurface,
                    ),
                  ),
                ),
                ?trailing,
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _UserAccountMenu extends ConsumerWidget {
  const _UserAccountMenu({
    required this.auth,
    required this.onOpenOnboarding,
    required this.onLogout,
  });

  final AuthState auth;
  final VoidCallback onOpenOnboarding;
  final VoidCallback onLogout;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final themeMode = ref.watch(themePreferenceProvider);
    final scheme = context.colorScheme;

    return PopupMenuButton<String>(
      tooltip: 'Conta e tema',
      offset: const Offset(0, 48),
      shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
      onSelected: (value) {
        switch (value) {
          case 'onboarding':
            onOpenOnboarding();
          case 'theme_light':
            ref.read(themePreferenceProvider.notifier).setThemeMode(ThemeMode.light);
          case 'theme_dark':
            ref.read(themePreferenceProvider.notifier).setThemeMode(ThemeMode.dark);
          case 'theme_system':
            ref.read(themePreferenceProvider.notifier).setThemeMode(ThemeMode.system);
          case 'logout':
            onLogout();
        }
      },
      itemBuilder: (context) => <PopupMenuEntry<String>>[
        PopupMenuItem<String>(
          enabled: false,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                auth.user?.displayName ?? 'Conta autenticada',
                style: TextStyle(
                  fontWeight: FontWeight.w700,
                  color: scheme.onSurface,
                ),
              ),
              Text(
                '${auth.user?.email ?? ''} · ${auth.currentRoleLabel}',
                style: TextStyle(
                  fontSize: 12,
                  color: scheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
        const PopupMenuDivider(),
        if (auth.canWrite)
          const PopupMenuItem<String>(
            value: 'onboarding',
            child: ListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              leading: Icon(Icons.rocket_launch_outlined),
              title: Text('Assistente Inicial'),
            ),
          ),
        PopupMenuItem<String>(
          value: 'theme_dark',
          child: ListTile(
            contentPadding: EdgeInsets.zero,
            dense: true,
            leading: Icon(
              Icons.dark_mode_outlined,
              color: themeMode == ThemeMode.dark ? scheme.primary : null,
            ),
            title: const Text('Tema escuro'),
            trailing: themeMode == ThemeMode.dark
                ? Icon(Icons.check, color: scheme.primary, size: 18)
                : null,
          ),
        ),
        PopupMenuItem<String>(
          value: 'theme_light',
          child: ListTile(
            contentPadding: EdgeInsets.zero,
            dense: true,
            leading: Icon(
              Icons.light_mode_outlined,
              color: themeMode == ThemeMode.light ? scheme.primary : null,
            ),
            title: const Text('Tema claro'),
            trailing: themeMode == ThemeMode.light
                ? Icon(Icons.check, color: scheme.primary, size: 18)
                : null,
          ),
        ),
        PopupMenuItem<String>(
          value: 'theme_system',
          child: ListTile(
            contentPadding: EdgeInsets.zero,
            dense: true,
            leading: Icon(
              Icons.brightness_auto_outlined,
              color: themeMode == ThemeMode.system ? scheme.primary : null,
            ),
            title: const Text('Seguir sistema'),
            trailing: themeMode == ThemeMode.system
                ? Icon(Icons.check, color: scheme.primary, size: 18)
                : null,
          ),
        ),
        const PopupMenuDivider(),
        PopupMenuItem<String>(
          value: 'logout',
          child: ListTile(
            contentPadding: EdgeInsets.zero,
            dense: true,
            leading: Icon(Icons.logout, color: scheme.error),
            title: Text(
              'Sair da conta',
              style: TextStyle(color: scheme.error, fontWeight: FontWeight.w700),
            ),
          ),
        ),
      ],
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            CircleAvatar(
              radius: 16,
              backgroundColor: scheme.primary,
              child: Icon(Icons.person, size: 18, color: scheme.onPrimary),
            ),
            const SizedBox(width: 4),
            Icon(Icons.arrow_drop_down, color: scheme.onSurfaceVariant),
          ],
        ),
      ),
    );
  }
}

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/utils/theme_preference.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Barra inferior compacta (< 600px) do `AdaptiveAppShell`.
class ShellBottomBar extends StatelessWidget {
  const ShellBottomBar({
    super.key,
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

/// Barra lateral compacta `NavigationRail` (600–1199px) do `AdaptiveAppShell`.
class ShellNavigationRail extends StatelessWidget {
  const ShellNavigationRail({
    super.key,
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

/// Gaveta completa de seções e grupos expansíveis do `AdaptiveAppShell`.
class ShellNavigationDrawer extends StatelessWidget {
  const ShellNavigationDrawer({
    super.key,
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
                      padding: EdgeInsets.fromLTRB(
                        12,
                        index == 0 ? 4 : 14,
                        12,
                        4,
                      ),
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
                  color: selected
                      ? scheme.onPrimaryContainer
                      : scheme.onSurfaceVariant,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    title,
                    style: TextStyle(
                      fontSize: 13.5,
                      fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
                      color: selected
                          ? scheme.onPrimaryContainer
                          : scheme.onSurface,
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

/// Menu suspenso de conta autenticada, assistente inicial e seletor de tema.
class ShellUserAccountMenu extends ConsumerWidget {
  const ShellUserAccountMenu({
    super.key,
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
            ref
                .read(themePreferenceProvider.notifier)
                .setThemeMode(ThemeMode.light);
          case 'theme_dark':
            ref
                .read(themePreferenceProvider.notifier)
                .setThemeMode(ThemeMode.dark);
          case 'theme_system':
            ref
                .read(themePreferenceProvider.notifier)
                .setThemeMode(ThemeMode.system);
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
                style: TextStyle(fontSize: 12, color: scheme.onSurfaceVariant),
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
              style: TextStyle(
                color: scheme.error,
                fontWeight: FontWeight.w700,
              ),
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

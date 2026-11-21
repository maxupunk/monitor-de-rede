import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/build_nav_sections.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';

void main() {
  group('buildNavSections', () {
    test('filters admin-only and PWA-only items for non-admin without PWA', () {
      final sections = buildNavSections(
        const NavContext(isAdmin: false, canInstallPwa: false),
        allNavEntries,
      );
      final paths = navLeaves(sections).map((e) => e.leaf.to).toSet();
      expect(paths, contains('/'));
      expect(paths, contains('/devices'));
      expect(paths, contains('/settings'));
      expect(paths, isNot(contains('/users')));
      expect(paths, isNot(contains('/audit')));
      expect(paths, isNot(contains('/backup')));
      expect(paths, isNot(contains('/plugins')));

      final allActions = sections
          .expand((s) => s.items)
          .map((i) => i.action)
          .whereType<NavAction>()
          .toSet();
      expect(allActions, isNot(contains(NavAction.installPwa)));
    });

    test('shows admin items and PWA action when permitted', () {
      final sections = buildNavSections(
        const NavContext(isAdmin: true, canInstallPwa: true),
        allNavEntries,
      );
      final paths = navLeaves(sections).map((e) => e.leaf.to).toSet();
      expect(
        paths,
        containsAll(<String>['/users', '/audit', '/backup', '/plugins']),
      );

      final adminSection = sections.firstWhere(
        (s) => s.title == 'Administração',
      );
      expect(
        adminSection.items.any((i) => i.action == NavAction.installPwa),
        isTrue,
      );
    });

    test('hides disabled areas while keeping core items always visible', () {
      final sections = buildNavSections(
        const NavContext(
          isAdmin: true,
          enabledAreas: <ProductArea>{ProductArea.core},
        ),
        allNavEntries,
      );
      final sectionTitles = sections.map((s) => s.title).toSet();
      expect(sectionTitles, isNot(contains('Rede')));
      expect(sectionTitles, isNot(contains('VPN')));

      final paths = navLeaves(sections).map((e) => e.leaf.to).toSet();
      expect(paths, contains('/'));
      expect(paths, contains('/remote-servers'));
      expect(paths, isNot(contains('/docker')));
      expect(paths, isNot(contains('/topology')));
      expect(paths, isNot(contains('/vpn')));
    });

    test('builds dynamic plugin fleet apps group and finds groupOfPath', () {
      final sections = buildNavSections(
        const NavContext(
          isAdmin: false,
          apps: <NavApp>[NavApp(id: 7, title: 'Wi-Fi Frota', icon: Icons.wifi)],
        ),
        allNavEntries,
      );
      expect(groupOfPath(sections, '/apps/7'), 'Aplicativos');
      expect(groupOfPath(sections, '/devices'), isNull);
    });

    test(
      'applies pinned and hidden menu prefs without bypassing area/admin filters',
      () {
        final sections = buildNavSections(
          const NavContext(
            isAdmin: false,
            enabledAreas: <ProductArea>{ProductArea.core, ProductArea.servers},
            menu: MenuPrefs(
              pinned: <String>['/docker', '/users', '/alerts'],
              hidden: <String>['/events', '/logs'],
            ),
          ),
          allNavEntries,
        );

        expect(sections.first.title, pinnedSectionTitle);
        final pinnedPaths = sections.first.items.map((i) => i.to).toList();
        // `/users` é admin-only e não pode reaparecer mesmo fixado.
        expect(pinnedPaths, <String>['/docker', '/alerts']);

        final allPaths = navLeaves(sections).map((e) => e.leaf.to).toSet();
        expect(allPaths, isNot(contains('/events')));
        expect(allPaths, isNot(contains('/logs')));
      },
    );
  });
}

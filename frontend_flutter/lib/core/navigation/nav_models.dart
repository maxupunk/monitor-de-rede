import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';

/// Ações que o shell sabe disparar a partir do menu lateral.
enum NavAction {
  playbook,
  traceroute,
  speedTest,
  installPwa,
}

/// Seções do menu lateral, lidas de cima para baixo por frequência de uso.
enum NavSectionId {
  monitoring('Monitoramento'),
  network('Rede'),
  servers('Servidores'),
  vpn('VPN'),
  operations('Operação'),
  admin('Administração');

  const NavSectionId(this.title);

  final String title;
}

/// Preferências pessoais de menu (Fase 16 — `GET /auth/me`, `PUT /me/menu`).
@immutable
class MenuPrefs {
  const MenuPrefs({
    this.pinned = const <String>[],
    this.hidden = const <String>[],
  });

  static const MenuPrefs empty = MenuPrefs();

  final List<String> pinned;
  final List<String> hidden;

  bool get isEmpty => pinned.isEmpty && hidden.isEmpty;

  Map<String, dynamic> toJson() => <String, dynamic>{
        'pinned': pinned,
        'hidden': hidden,
      };

  static MenuPrefs? tryParse(Object? value) {
    if (value is! Map) return null;
    final rawPinned = value['pinned'];
    final rawHidden = value['hidden'];
    if (rawPinned is! List || rawHidden is! List) return null;
    if (!rawPinned.every((item) => item is String)) return null;
    if (!rawHidden.every((item) => item is String)) return null;
    return MenuPrefs(
      pinned: List<String>.unmodifiable(rawPinned.cast<String>()),
      hidden: List<String>.unmodifiable(rawHidden.cast<String>()),
    );
  }
}

/// Folha navegável (rota ou ação) no menu.
@immutable
class NavLeaf {
  const NavLeaf({
    required this.title,
    required this.icon,
    required this.area,
    this.to,
    this.action,
  });

  final String title;
  final IconData icon;
  final ProductArea area;
  final String? to;
  final NavAction? action;
}

/// Item de menu (folha direta ou grupo expansível com filhos).
@immutable
class NavItem extends NavLeaf {
  const NavItem({
    required super.title,
    required super.icon,
    required super.area,
    super.to,
    super.action,
    this.children,
  });

  final List<NavLeaf>? children;

  bool get isGroup => children != null;
}

/// Seção renderizada no menu lateral.
@immutable
class NavSection {
  const NavSection({
    required this.title,
    required this.items,
  });

  final String title;
  final List<NavItem> items;
}

/// Folha achatada acompanhada do título da seção.
@immutable
class NavLeafWithSection {
  const NavLeafWithSection({
    required this.leaf,
    required this.section,
  });

  final NavLeaf leaf;
  final String section;
}

/// Plugin com página de frota (grupo dinâmico "Aplicativos").
@immutable
class NavApp {
  const NavApp({
    required this.id,
    required this.title,
    required this.icon,
  });

  final int id;
  final String title;
  final IconData icon;
}

/// Contexto puro usado por `buildNavSections`.
@immutable
class NavContext {
  const NavContext({
    required this.isAdmin,
    this.apps = const <NavApp>[],
    this.canInstallPwa = false,
    this.enabledAreas,
    this.menu,
  });

  final bool isAdmin;
  final List<NavApp> apps;
  final bool canInstallPwa;

  /// Áreas visíveis (ligadas e com acesso). Se `null`, todas são exibidas.
  /// `ProductArea.core` é sempre considerado ligado.
  final Set<ProductArea>? enabledAreas;

  /// Menu pessoal (fixados no topo, ocultos fora).
  final MenuPrefs? menu;
}

/// Especificação de subitem dentro de um grupo (sem `area`, carimbada pela entrada).
@immutable
class NavLeafSpec {
  const NavLeafSpec({
    required this.title,
    required this.icon,
    this.to,
    this.action,
  });

  final String title;
  final IconData icon;
  final String? to;
  final NavAction? action;
}

/// Especificação declarada por cada área.
@immutable
class NavItemSpec {
  const NavItemSpec({
    required this.title,
    required this.icon,
    this.to,
    this.action,
    this.children,
    this.admin = false,
    this.pwa = false,
  });

  final String title;
  final IconData icon;
  final String? to;
  final NavAction? action;
  final List<NavLeafSpec>? children;
  final bool admin;
  final bool pwa;
}

/// Contribuição de uma área para o menu lateral.
@immutable
class NavEntry {
  const NavEntry({
    required this.area,
    required this.section,
    required this.order,
    this.item,
    this.builder,
  }) : assert(
          (item != null) ^ (builder != null),
          'NavEntry deve declarar exatamente item ou builder.',
        );

  final ProductArea area;
  final NavSectionId section;
  final int order;
  final NavItemSpec? item;
  final NavItemSpec? Function(NavContext ctx)? builder;

  NavItemSpec? resolve(NavContext ctx) => builder != null ? builder!(ctx) : item;
}

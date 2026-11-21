import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';

/// Título da seção dos itens fixados pelo usuário.
const String pinnedSectionTitle = 'Fixados';

bool _isAreaOn(ProductArea area, NavContext ctx) {
  return area == ProductArea.core ||
      ctx.enabledAreas == null ||
      ctx.enabledAreas!.contains(area);
}

bool _isVisible(NavItemSpec spec, ProductArea area, NavContext ctx) {
  if (spec.admin && !ctx.isAdmin) return false;
  if (spec.pwa && !ctx.canInstallPwa) return false;
  return _isAreaOn(area, ctx);
}

NavItem _toNavItem(NavItemSpec spec, ProductArea area) {
  final children = spec.children;
  if (children == null) {
    return NavItem(
      title: spec.title,
      icon: spec.icon,
      area: area,
      to: spec.to,
      action: spec.action,
    );
  }
  return NavItem(
    title: spec.title,
    icon: spec.icon,
    area: area,
    to: spec.to,
    action: spec.action,
    children: children
        .map(
          (child) => NavLeaf(
            title: child.title,
            icon: child.icon,
            area: area,
            to: child.to,
            action: child.action,
          ),
        )
        .toList(growable: false),
  );
}

/// Os itens do menu que levam a uma página (`to != null`), na ordem do menu.
List<NavLeafWithSection> navLeaves(List<NavSection> sections) {
  final result = <NavLeafWithSection>[];
  for (final section in sections) {
    for (final item in section.items) {
      final leaves = item.children ?? <NavLeaf>[item];
      for (final leaf in leaves) {
        if (leaf.to != null && leaf.to!.isNotEmpty) {
          result.add(NavLeafWithSection(leaf: leaf, section: section.title));
        }
      }
    }
  }
  return result;
}

List<NavSection> _applyMenu(List<NavSection> sections, MenuPrefs? menu) {
  if (menu == null || menu.isEmpty) return sections;

  final hidden = menu.hidden.toSet();
  bool shown(NavLeaf leaf) => leaf.to == null || !hidden.contains(leaf.to);

  final byPath = <String, NavLeaf>{};
  for (final entry in navLeaves(sections)) {
    final path = entry.leaf.to;
    if (path != null) {
      byPath[path] = entry.leaf;
    }
  }

  final pinned = <NavItem>[];
  for (final path in menu.pinned) {
    final leaf = byPath[path];
    if (leaf != null) {
      pinned.add(
        NavItem(
          title: leaf.title,
          icon: leaf.icon,
          area: leaf.area,
          to: leaf.to,
        ),
      );
    }
  }

  final rest = <NavSection>[];
  for (final section in sections) {
    final filteredItems = <NavItem>[];
    for (final item in section.items) {
      if (!shown(item)) continue;
      if (item.children != null) {
        final visibleChildren = item.children!.where(shown).toList(growable: false);
        if (visibleChildren.isEmpty) continue;
        filteredItems.add(
          NavItem(
            title: item.title,
            icon: item.icon,
            area: item.area,
            to: item.to,
            action: item.action,
            children: visibleChildren,
          ),
        );
      } else {
        filteredItems.add(item);
      }
    }
    if (filteredItems.isNotEmpty) {
      rest.add(NavSection(title: section.title, items: filteredItems));
    }
  }

  if (pinned.isEmpty) return rest;
  return <NavSection>[
    NavSection(title: pinnedSectionTitle, items: pinned),
    ...rest,
  ];
}

/// Menu filtrado por áreas e papéis, antes das preferências pessoais do usuário.
List<NavSection> buildBaseSections(NavContext ctx, List<NavEntry> entries) {
  final result = <NavSection>[];
  for (final section in NavSectionId.values) {
    final matching = entries.where((entry) => entry.section == section).toList()
      ..sort((a, b) => a.order.compareTo(b.order));

    final items = <NavItem>[];
    for (final entry in matching) {
      final spec = entry.resolve(ctx);
      if (spec == null || !_isVisible(spec, entry.area, ctx)) continue;
      final item = _toNavItem(spec, entry.area);
      if (item.children != null && item.children!.isEmpty) continue;
      items.add(item);
    }

    if (items.isNotEmpty) {
      result.add(NavSection(title: section.title, items: items));
    }
  }
  return result;
}

/// Menu completo para este contexto (áreas + papéis + preferências pessoais).
/// Seções e grupos vazios são omitidos automaticamente.
List<NavSection> buildNavSections(NavContext ctx, List<NavEntry> entries) {
  return _applyMenu(buildBaseSections(ctx, entries), ctx.menu);
}

/// O título do grupo expansível que contém a rota `path` (para auto-abrir).
String? groupOfPath(List<NavSection> sections, String path) {
  for (final section in sections) {
    for (final item in section.items) {
      final children = item.children;
      if (children == null) continue;
      final found = children.any(
        (child) =>
            child.to != null &&
            (path == child.to || path.startsWith('${child.to}/')),
      );
      if (found) return item.title;
    }
  }
  return null;
}

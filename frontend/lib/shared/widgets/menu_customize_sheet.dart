import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/navigation/build_nav_sections.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/pwa/pwa_install_service.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/shared/widgets/adaptive_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/nested_panel.dart';
import 'package:netmonitor_app/shared/widgets/symmetrical_action_bar.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o diálogo unificado de personalização do menu lateral (fixar, reordenar
/// fixados e ocultar itens pouco usados — DRY entre `AdaptiveAppShell` e `PreferencesCard`).
Future<void> showMenuCustomizeSheet(
  BuildContext context,
  WidgetRef ref, {
  List<NavSection>? baseSections,
}) {
  final auth = ref.read(authProvider);
  final resolvedSections =
      baseSections ??
      buildBaseSections(
        NavContext(
          isAdmin: auth.isAdmin,
          apps: ref.read(pluginAppsNavProvider),
          canInstallPwa: ref.read(pwaInstallProvider).showInstallAction,
          enabledAreas: ref.read(visibleAreasProvider),
        ),
        allNavEntries,
      );

  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Personalizar menu',
    subtitle:
        'Fixe atalhos no topo, ajuste a ordem ou oculte itens pouco usados.',
    maxWidth: 560,
    builder: (_) => MenuCustomizeSheetBody(
      leaves: navLeaves(resolvedSections),
      initialPrefs: auth.menuPrefs,
    ),
  );
}

class MenuCustomizeSheetBody extends ConsumerStatefulWidget {
  const MenuCustomizeSheetBody({
    super.key,
    required this.leaves,
    required this.initialPrefs,
  });

  final List<NavLeafWithSection> leaves;
  final MenuPrefs initialPrefs;

  @override
  ConsumerState<MenuCustomizeSheetBody> createState() =>
      _MenuCustomizeSheetBodyState();
}

class _MenuCustomizeSheetBodyState
    extends ConsumerState<MenuCustomizeSheetBody> {
  late final List<String> _pinned;
  late final Set<String> _hidden;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _pinned = List<String>.from(widget.initialPrefs.pinned);
    _hidden = Set<String>.from(widget.initialPrefs.hidden);
  }

  Future<void> _save() async {
    setState(() => _saving = true);
    try {
      await ref
          .read(authProvider.notifier)
          .saveMenuPrefs(
            MenuPrefs(
              pinned: List<String>.unmodifiable(_pinned),
              hidden: List<String>.unmodifiable(_hidden),
            ),
          );
      if (!mounted) return;
      Navigator.of(context).pop();
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Preferências do menu atualizadas.')),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(describeError(err))));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final byPath = <String, NavLeafWithSection>{
      for (final leaf in widget.leaves)
        if (leaf.leaf.to != null) leaf.leaf.to!: leaf,
    };
    final visiblePinnedIndices = <int>[
      for (var i = 0; i < _pinned.length; i++)
        if (byPath.containsKey(_pinned[i])) i,
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (visiblePinnedIndices.isNotEmpty) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.primary,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  'Ordem dos fixados (${visiblePinnedIndices.length})',
                  style: context.textTheme.labelLarge?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
                const SizedBox(height: 6),
                for (var vi = 0; vi < visiblePinnedIndices.length; vi++)
                  Builder(
                    builder: (_) {
                      final idx = visiblePinnedIndices[vi];
                      final leaf = byPath[_pinned[idx]]!.leaf;
                      return Row(
                        children: <Widget>[
                          Icon(leaf.icon, size: 18),
                          const SizedBox(width: 8),
                          Expanded(
                            child: Text(
                              leaf.title,
                              style: const TextStyle(
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                          ),
                          IconButton(
                            tooltip: 'Subir',
                            visualDensity: VisualDensity.compact,
                            icon: const Icon(Icons.arrow_upward, size: 18),
                            onPressed: vi == 0
                                ? null
                                : () {
                                    final targetIdx =
                                        visiblePinnedIndices[vi - 1];
                                    setState(() {
                                      final tmp = _pinned[idx];
                                      _pinned[idx] = _pinned[targetIdx];
                                      _pinned[targetIdx] = tmp;
                                    });
                                  },
                          ),
                          IconButton(
                            tooltip: 'Descer',
                            visualDensity: VisualDensity.compact,
                            icon: const Icon(Icons.arrow_downward, size: 18),
                            onPressed: vi == visiblePinnedIndices.length - 1
                                ? null
                                : () {
                                    final targetIdx =
                                        visiblePinnedIndices[vi + 1];
                                    setState(() {
                                      final tmp = _pinned[idx];
                                      _pinned[idx] = _pinned[targetIdx];
                                      _pinned[targetIdx] = tmp;
                                    });
                                  },
                          ),
                          IconButton(
                            tooltip: 'Desafixar',
                            visualDensity: VisualDensity.compact,
                            icon: const Icon(Icons.close, size: 18),
                            onPressed: () =>
                                setState(() => _pinned.removeAt(idx)),
                          ),
                        ],
                      );
                    },
                  ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        for (final entry in widget.leaves) ...<Widget>[
          if (entry.leaf.to != null) ...<Widget>[
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: Icon(entry.leaf.icon, size: 20),
              title: Text(entry.leaf.title),
              subtitle: Text(entry.section),
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  IconButton(
                    tooltip: _pinned.contains(entry.leaf.to)
                        ? 'Remover dos fixados'
                        : 'Fixar no topo',
                    icon: Icon(
                      _pinned.contains(entry.leaf.to)
                          ? Icons.push_pin
                          : Icons.push_pin_outlined,
                      color: _pinned.contains(entry.leaf.to)
                          ? context.colorScheme.primary
                          : context.colorScheme.onSurfaceVariant,
                    ),
                    onPressed: () {
                      final path = entry.leaf.to!;
                      setState(() {
                        if (_pinned.contains(path)) {
                          _pinned.remove(path);
                        } else {
                          _pinned.add(path);
                          _hidden.remove(path);
                        }
                      });
                    },
                  ),
                  IconButton(
                    tooltip: _hidden.contains(entry.leaf.to)
                        ? 'Mostrar no menu'
                        : 'Ocultar do menu',
                    icon: Icon(
                      _hidden.contains(entry.leaf.to)
                          ? Icons.visibility_off
                          : Icons.visibility_outlined,
                      color: _hidden.contains(entry.leaf.to)
                          ? context.colorScheme.error
                          : context.colorScheme.onSurfaceVariant,
                    ),
                    onPressed: () {
                      final path = entry.leaf.to!;
                      setState(() {
                        if (_hidden.contains(path)) {
                          _hidden.remove(path);
                        } else {
                          _hidden.add(path);
                          _pinned.remove(path);
                        }
                      });
                    },
                  ),
                ],
              ),
            ),
            const Divider(height: 1),
          ],
        ],
        const SizedBox(height: 16),
        SymmetricalActionBar(
          children: <Widget>[
            OutlinedButton(
              onPressed: _saving
                  ? null
                  : () {
                      setState(() {
                        _pinned.clear();
                        _hidden.clear();
                      });
                    },
              child: const Text('Restaurar padrão'),
            ),
            FilledButton.icon(
              onPressed: _saving ? null : _save,
              icon: _saving
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.save_outlined, size: 18),
              label: const Text('Salvar menu'),
            ),
          ],
        ),
      ],
    );
  }
}

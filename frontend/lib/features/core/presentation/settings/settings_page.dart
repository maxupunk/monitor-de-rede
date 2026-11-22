import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';

/// Tela de Configurações com abas e cartões compostos dinamicamente por área
/// via `SettingsSectionRegistry` (paridade 100% com `SettingsPage.vue`).
class SettingsPage extends ConsumerStatefulWidget {
  const SettingsPage({super.key, this.initialTab});

  final String? initialTab;

  @override
  ConsumerState<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends ConsumerState<SettingsPage> {
  String? _selectedTab;

  @override
  void initState() {
    super.initState();
    _selectedTab = widget.initialTab;
  }

  @override
  void didUpdateWidget(covariant SettingsPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.initialTab != widget.initialTab &&
        widget.initialTab != null &&
        widget.initialTab!.isNotEmpty) {
      _selectedTab = widget.initialTab;
    }
  }

  @override
  Widget build(BuildContext context) {
    final visibleAreas = ref.watch(visibleAreasProvider);
    final tabs = SettingsSectionRegistry.visibleTabs(visibleAreas);

    if (tabs.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhuma configuração disponível',
        icon: Icons.settings_outlined,
      );
    }

    final activeIndex = _selectedTab != null
        ? tabs
              .indexWhere((t) => t.tab.value == _selectedTab)
              .clamp(0, tabs.length - 1)
        : 0;
    final currentView = tabs[activeIndex];

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;

        return CustomScrollView(
          slivers: <Widget>[
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: SearchFilterHeader(
                  title: 'Configurações',
                  subtitle: isCompact
                      ? null
                      : 'Preferências globais, áreas do produto, conectividade, notificações e sistema',
                ),
              ),
            ),
            SliverPersistentHeader(
              pinned: true,
              delegate: PinnedSliverHeaderDelegate(
                height: 52,
                bottomBorder: true,
                child: SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: Row(
                    children: <Widget>[
                      for (var i = 0; i < tabs.length; i++) ...<Widget>[
                        if (i > 0) const SizedBox(width: 8),
                        ChoiceChip(
                          visualDensity: isCompact
                              ? VisualDensity.compact
                              : VisualDensity.standard,
                          selected: i == activeIndex,
                          avatar: Icon(tabs[i].tab.icon, size: 16),
                          label: Text(tabs[i].tab.title),
                          onSelected: (_) =>
                              setState(() => _selectedTab = tabs[i].tab.value),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ),
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.only(top: 12, bottom: 24),
                child: _SettingsCardsGrid(cards: currentView.cards),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _SettingsCardsGrid extends StatelessWidget {
  const _SettingsCardsGrid({required this.cards});

  final List<SettingsCardContribution> cards;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        const spacing = 14.0;

        double cardWidth(SettingsCardContribution c) {
          if (width < 760) return width;
          final cols = width >= 1120 ? c.lgCols : c.mdCols;
          if (cols >= 12) return width;
          final frac = (cols.clamp(4, 12)) / 12.0;
          final perRow = (1 / frac).round();
          return (width - spacing * (perRow - 1)) / perRow;
        }

        return Wrap(
          spacing: spacing,
          runSpacing: spacing,
          children: <Widget>[
            for (final card in cards)
              SizedBox(width: cardWidth(card), child: card.builder(context)),
          ],
        );
      },
    );
  }
}

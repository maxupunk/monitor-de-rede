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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SearchFilterHeader(
          title: 'Configurações',
          subtitle: 'Preferências globais, áreas do produto, conectividade, notificações e sistema',
        ),
        const SizedBox(height: 12),
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            children: <Widget>[
              for (var i = 0; i < tabs.length; i++) ...<Widget>[
                Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: ChoiceChip(
                    selected: i == activeIndex,
                    avatar: Icon(tabs[i].tab.icon, size: 16),
                    label: Text(tabs[i].tab.title),
                    onSelected: (_) =>
                        setState(() => _selectedTab = tabs[i].tab.value),
                  ),
                ),
              ],
            ],
          ),
        ),
        const SizedBox(height: 14),
        Expanded(
          child: ListView(
            children: <Widget>[
              _SettingsCardsGrid(cards: currentView.cards),
              const SizedBox(height: 24),
            ],
          ),
        ),
      ],
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

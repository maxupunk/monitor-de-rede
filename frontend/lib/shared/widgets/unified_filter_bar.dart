import 'package:flutter/material.dart';
import 'package:netmonitor_app/shared/widgets/adaptive_form_sheet.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Opção de ordenação para o menu compacto do [UnifiedFilterBar].
@immutable
class FilterSortOption {
  const FilterSortOption({required this.value, required this.label, this.icon});

  final String value;
  final String label;
  final IconData? icon;
}

/// Chip removível representando um filtro secundário ativo no [UnifiedFilterBar].
@immutable
class ActiveFilterChipData {
  const ActiveFilterChipData({
    required this.label,
    required this.onDeleted,
    this.icon,
  });

  final String label;
  final VoidCallback onDeleted;
  final IconData? icon;
}

/// Construtor do conteúdo interno do sheet de filtros do [UnifiedFilterBar].
typedef FilterSheetContentBuilder =
    Widget Function(BuildContext sheetContext, StateSetter setSheetState);

/// Barra de busca, filtros e ordenação canônica Material Design 3 (mobile-first).
///
/// Garante simetria visual em todas as telas de listagem do NetMonitor:
/// - **Linha 1**: campo de busca denso (`Expanded`) + botão tonal `Filtros` com
///   `Badge` de contagem ativa (abre `showAdaptiveFormSheet`) + menu compacto
///   `Ordenar` (`PopupMenuButton`) opcional.
/// - **Linha 2** (somente quando há chips): faixa horizontal única rolável com
///   `quickChips` (ex.: `FilterChip`s de status) e/ou `activeChips` removíveis
///   (`InputChip`), evitando misturar dropdowns e chips num mesmo `Wrap`.
class UnifiedFilterBar extends StatelessWidget {
  const UnifiedFilterBar({
    super.key,
    this.searchController,
    this.searchHint = 'Buscar...',
    this.onSearchChanged,
    this.activeFilterCount = 0,
    this.filterSheetTitle = 'Filtros',
    this.filterSheetSubtitle,
    this.filterSheetMaxWidth = 560,
    this.filterSheetBuilder,
    this.onApplyFilters,
    this.onClearFilters,
    this.sortOptions = const <FilterSortOption>[],
    this.activeSort,
    this.onSortChanged,
    this.quickChips = const <Widget>[],
    this.activeChips = const <ActiveFilterChipData>[],
    this.onClearAll,
    this.trailingActions = const <Widget>[],
  });

  final TextEditingController? searchController;
  final String searchHint;
  final ValueChanged<String>? onSearchChanged;

  /// Quantidade de filtros secundários ativos exibida no `Badge` do botão `Filtros`.
  final int activeFilterCount;
  final String filterSheetTitle;
  final String? filterSheetSubtitle;
  final double filterSheetMaxWidth;
  final FilterSheetContentBuilder? filterSheetBuilder;
  final VoidCallback? onApplyFilters;
  final VoidCallback? onClearFilters;

  final List<FilterSortOption> sortOptions;
  final String? activeSort;
  final ValueChanged<String>? onSortChanged;

  /// Chips rápidos de filtro (ex.: `FilterChip`s de status com contagem).
  final List<Widget> quickChips;

  /// Chips removíveis para filtros secundários ativos selecionados no sheet.
  final List<ActiveFilterChipData> activeChips;

  /// Ação para limpar todos os filtros ativos (usada no chip `Limpar` e no sheet).
  final VoidCallback? onClearAll;

  /// Ações compactas opcionais ao final da linha 1.
  final List<Widget> trailingActions;

  bool get _hasSearch => searchController != null || onSearchChanged != null;
  bool get _hasFilterSheet => filterSheetBuilder != null;
  bool get _hasSort => sortOptions.isNotEmpty && onSortChanged != null;
  bool get _hasChipRow => quickChips.isNotEmpty || activeChips.isNotEmpty;

  Future<void> _openFilterSheet(BuildContext context) async {
    final builder = filterSheetBuilder;
    if (builder == null) return;
    final clearAction = onClearFilters ?? onClearAll;

    await showAdaptiveFormSheet<void>(
      context: context,
      title: filterSheetTitle,
      subtitle: filterSheetSubtitle,
      maxWidth: filterSheetMaxWidth,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (ctx, setSheetState) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                builder(ctx, setSheetState),
                const SizedBox(height: 16),
                Wrap(
                  alignment: WrapAlignment.end,
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    if (clearAction != null)
                      TextButton(
                        onPressed: () {
                          clearAction();
                          setSheetState(() {});
                        },
                        child: const Text('Limpar'),
                      ),
                    FilledButton(
                      onPressed: () {
                        onApplyFilters?.call();
                        Navigator.of(ctx).pop();
                      },
                      child: const Text('Aplicar'),
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

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final clearCallback = onClearAll ?? onClearFilters;

    return LayoutBuilder(
      builder: (context, constraints) {
        final iconOnlyFilter = constraints.maxWidth < 310;

        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            if (_hasSearch ||
                _hasFilterSheet ||
                _hasSort ||
                trailingActions.isNotEmpty)
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: <Widget>[
                  if (_hasSearch)
                    Expanded(
                      child: TextField(
                        controller: searchController,
                        onChanged: onSearchChanged,
                        decoration: InputDecoration(
                          hintText: searchHint,
                          prefixIcon: const Icon(Icons.search, size: 20),
                          isDense: true,
                          contentPadding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 11,
                          ),
                          suffixIcon: searchController != null
                              ? ValueListenableBuilder<TextEditingValue>(
                                  valueListenable: searchController!,
                                  builder: (context, value, _) {
                                    if (value.text.isEmpty) {
                                      return const SizedBox.shrink();
                                    }
                                    return IconButton(
                                      tooltip: 'Limpar busca',
                                      icon: const Icon(Icons.close, size: 18),
                                      onPressed: () {
                                        searchController!.clear();
                                        onSearchChanged?.call('');
                                      },
                                    );
                                  },
                                )
                              : null,
                        ),
                      ),
                    )
                  else
                    const Spacer(),
                  if (_hasFilterSheet) ...<Widget>[
                    const SizedBox(width: 8),
                    _FilterSheetTriggerButton(
                      activeCount: activeFilterCount,
                      iconOnly: iconOnlyFilter,
                      onPressed: () => _openFilterSheet(context),
                    ),
                  ],
                  if (_hasSort) ...<Widget>[
                    const SizedBox(width: 8),
                    _SortMenuButton(
                      options: sortOptions,
                      activeSort: activeSort,
                      onSelected: onSortChanged!,
                    ),
                  ],
                  for (final action in trailingActions) ...<Widget>[
                    const SizedBox(width: 8),
                    action,
                  ],
                ],
              ),
            if (_hasChipRow) ...<Widget>[
              const SizedBox(height: 10),
              SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    for (var i = 0; i < quickChips.length; i++) ...<Widget>[
                      if (i > 0) const SizedBox(width: 8),
                      quickChips[i],
                    ],
                    if (quickChips.isNotEmpty &&
                        activeChips.isNotEmpty) ...<Widget>[
                      const SizedBox(width: 10),
                      Container(
                        width: 1,
                        height: 22,
                        color: scheme.outlineVariant,
                      ),
                      const SizedBox(width: 10),
                    ],
                    for (var i = 0; i < activeChips.length; i++) ...<Widget>[
                      if (i > 0) const SizedBox(width: 8),
                      InputChip(
                        avatar: activeChips[i].icon != null
                            ? Icon(
                                activeChips[i].icon,
                                size: 16,
                                color: scheme.primary,
                              )
                            : null,
                        label: Text(activeChips[i].label),
                        deleteIcon: const Icon(Icons.close, size: 16),
                        deleteButtonTooltipMessage: 'Remover filtro',
                        onDeleted: activeChips[i].onDeleted,
                        visualDensity: VisualDensity.compact,
                        materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      ),
                    ],
                    if (clearCallback != null &&
                        (activeChips.length >= 2 ||
                            (activeFilterCount > 1 &&
                                activeChips.isNotEmpty))) ...<Widget>[
                      const SizedBox(width: 8),
                      ActionChip(
                        avatar: Icon(
                          Icons.filter_alt_off_outlined,
                          size: 16,
                          color: scheme.primary,
                        ),
                        label: const Text('Limpar'),
                        onPressed: clearCallback,
                        visualDensity: VisualDensity.compact,
                        materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ],
        );
      },
    );
  }
}

class _FilterSheetTriggerButton extends StatelessWidget {
  const _FilterSheetTriggerButton({
    required this.activeCount,
    required this.iconOnly,
    required this.onPressed,
  });

  final int activeCount;
  final bool iconOnly;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final badgeIcon = Badge.count(
      count: activeCount,
      isLabelVisible: activeCount > 0,
      child: const Icon(Icons.tune, size: 18),
    );

    if (iconOnly) {
      return IconButton.filledTonal(
        tooltip: 'Filtros',
        onPressed: onPressed,
        style: IconButton.styleFrom(
          minimumSize: const Size(44, 44),
          shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        ),
        icon: badgeIcon,
      );
    }

    return Tooltip(
      message: 'Filtros',
      child: FilledButton.tonalIcon(
        onPressed: onPressed,
        style: FilledButton.styleFrom(
          minimumSize: const Size(0, 44),
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        ),
        icon: badgeIcon,
        label: const Text('Filtros'),
      ),
    );
  }
}

class _SortMenuButton extends StatelessWidget {
  const _SortMenuButton({
    required this.options,
    required this.activeSort,
    required this.onSelected,
  });

  final List<FilterSortOption> options;
  final String? activeSort;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;

    return PopupMenuButton<String>(
      tooltip: 'Ordenar',
      onSelected: onSelected,
      itemBuilder: (context) => options
          .map((opt) {
            final selected = opt.value == activeSort;
            return PopupMenuItem<String>(
              value: opt.value,
              child: Row(
                children: <Widget>[
                  SizedBox(
                    width: 20,
                    child: selected
                        ? Icon(Icons.check, size: 18, color: scheme.primary)
                        : (opt.icon != null
                              ? Icon(
                                  opt.icon,
                                  size: 18,
                                  color: scheme.onSurfaceVariant,
                                )
                              : null),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      opt.label,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontWeight: selected
                            ? FontWeight.w700
                            : FontWeight.w500,
                        color: selected ? scheme.primary : scheme.onSurface,
                      ),
                    ),
                  ),
                ],
              ),
            );
          })
          .toList(growable: false),
      child: Container(
        width: 44,
        height: 44,
        decoration: BoxDecoration(
          color: scheme.secondaryContainer,
          borderRadius: AppTheme.borderRadius,
        ),
        child: Icon(
          Icons.swap_vert,
          size: 20,
          color: scheme.onSecondaryContainer,
        ),
      ),
    );
  }
}

import 'package:flutter/material.dart';

/// Cabeçalho responsivo com campo de busca, filtros e ações principais.
/// Empilha verticalmente em telas compactas (< 680px) e fica em linha em telas amplas.
class SearchFilterHeader extends StatelessWidget {
  const SearchFilterHeader({
    super.key,
    this.title,
    this.subtitle,
    this.searchController,
    this.searchHint = 'Buscar...',
    this.onSearchChanged,
    this.filters = const <Widget>[],
    this.actions = const <Widget>[],
  });

  final String? title;
  final String? subtitle;
  final TextEditingController? searchController;
  final String searchHint;
  final ValueChanged<String>? onSearchChanged;
  final List<Widget> filters;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final hasSearch = searchController != null || onSearchChanged != null;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        final searchField = hasSearch
            ? TextField(
                controller: searchController,
                onChanged: onSearchChanged,
                decoration: InputDecoration(
                  hintText: searchHint,
                  prefixIcon: const Icon(Icons.search, size: 20),
                  isDense: true,
                  suffixIcon: searchController != null
                      ? ValueListenableBuilder<TextEditingValue>(
                          valueListenable: searchController!,
                          builder: (context, value, _) {
                            if (value.text.isEmpty) return const SizedBox.shrink();
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
              )
            : null;

        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            if (title != null || actions.isNotEmpty)
              Wrap(
                spacing: 12,
                runSpacing: 10,
                alignment: WrapAlignment.spaceBetween,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  if (title != null)
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          title!,
                          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                                fontWeight: FontWeight.w800,
                                color: scheme.onSurface,
                              ),
                        ),
                        if (subtitle != null && subtitle!.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 2),
                          Text(
                            subtitle!,
                            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                                  color: scheme.onSurfaceVariant,
                                ),
                          ),
                        ],
                      ],
                    ),
                  if (actions.isNotEmpty)
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: actions,
                    ),
                ],
              ),
            if (hasSearch || filters.isNotEmpty) ...<Widget>[
              if (title != null || actions.isNotEmpty) const SizedBox(height: 12),
              if (isCompact)
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    ?searchField,
                    if (filters.isNotEmpty) ...<Widget>[
                      if (searchField != null) const SizedBox(height: 10),
                      Wrap(spacing: 8, runSpacing: 8, children: filters),
                    ],
                  ],
                )
              else
                Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: <Widget>[
                    if (searchField != null)
                      ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 340),
                        child: searchField,
                      ),
                    if (searchField != null && filters.isNotEmpty)
                      const SizedBox(width: 12),
                    if (filters.isNotEmpty)
                      Expanded(
                        child: Wrap(
                          spacing: 8,
                          runSpacing: 8,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: filters,
                        ),
                      ),
                  ],
                ),
            ],
          ],
        );
      },
    );
  }
}

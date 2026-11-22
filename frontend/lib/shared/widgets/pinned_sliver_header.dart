import 'package:flutter/material.dart';

/// Delegate canônico para fixar `TabBar` ou barras compactas de filtro/abas no
/// topo de um `NestedScrollView` / `CustomScrollView` com superfície limpa (DRY).
class PinnedSliverHeaderDelegate extends SliverPersistentHeaderDelegate {
  const PinnedSliverHeaderDelegate({
    required this.height,
    required this.child,
    this.backgroundColor,
    this.bottomBorder = false,
  });

  final double height;
  final Widget child;
  final Color? backgroundColor;
  final bool bottomBorder;

  @override
  double get minExtent => height;

  @override
  double get maxExtent => height;

  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) {
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: backgroundColor ?? Theme.of(context).scaffoldBackgroundColor,
      elevation: overlapsContent ? 1 : 0,
      shadowColor: scheme.shadow,
      surfaceTintColor: Colors.transparent,
      child: Container(
        decoration: bottomBorder
            ? BoxDecoration(
                border: Border(
                  bottom: BorderSide(color: scheme.outlineVariant),
                ),
              )
            : null,
        alignment: Alignment.centerLeft,
        child: child,
      ),
    );
  }

  @override
  bool shouldRebuild(covariant PinnedSliverHeaderDelegate oldDelegate) {
    return height != oldDelegate.height ||
        backgroundColor != oldDelegate.backgroundColor ||
        bottomBorder != oldDelegate.bottomBorder ||
        child != oldDelegate.child;
  }
}

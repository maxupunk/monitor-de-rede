import 'package:flutter/material.dart';

/// Grade responsiva baseada em breakpoints de largura (mobile-first).
class ResponsiveGrid extends StatelessWidget {
  const ResponsiveGrid({
    super.key,
    required this.children,
    this.xs = 1,
    this.sm = 2,
    this.md = 2,
    this.lg = 3,
    this.xl = 4,
    this.spacing = 12,
    this.runSpacing = 12,
  });

  final List<Widget> children;
  final int xs;
  final int sm;
  final int md;
  final int lg;
  final int xl;
  final double spacing;
  final double runSpacing;

  int _columnsFor(double width) {
    if (width >= 1440) return xl;
    if (width >= 1100) return lg;
    if (width >= 820) return md;
    if (width >= 560) return sm;
    return xs;
  }

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) return const SizedBox.shrink();

    return LayoutBuilder(
      builder: (context, constraints) {
        final cols = _columnsFor(constraints.maxWidth).clamp(1, 12);
        final totalSpacing = spacing * (cols - 1);
        final itemWidth = (constraints.maxWidth - totalSpacing) / cols;

        return Wrap(
          spacing: spacing,
          runSpacing: runSpacing,
          children: children
              .map((child) => SizedBox(width: itemWidth, child: child))
              .toList(growable: false),
        );
      },
    );
  }
}

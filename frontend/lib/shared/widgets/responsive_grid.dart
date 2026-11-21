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

/// Linha de campos de formulário mobile-first:
/// empilha verticalmente abaixo de [breakpoint] e distribui lado a lado em
/// colunas iguais acima dele, evitando campos espremidos em telas estreitas.
class ResponsiveFieldRow extends StatelessWidget {
  const ResponsiveFieldRow({
    super.key,
    required this.children,
    this.breakpoint = 480,
    this.spacing = 10,
    this.runSpacing = 12,
  });

  final List<Widget> children;
  final double breakpoint;
  final double spacing;
  final double runSpacing;

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) return const SizedBox.shrink();
    if (children.length == 1) return children.first;

    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < breakpoint) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              for (var i = 0; i < children.length; i++) ...<Widget>[
                if (i > 0) SizedBox(height: runSpacing),
                children[i],
              ],
            ],
          );
        }

        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            for (var i = 0; i < children.length; i++) ...<Widget>[
              if (i > 0) SizedBox(width: spacing),
              Expanded(child: children[i]),
            ],
          ],
        );
      },
    );
  }
}

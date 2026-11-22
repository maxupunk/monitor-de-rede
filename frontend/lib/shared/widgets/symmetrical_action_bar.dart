import 'package:flutter/material.dart';

/// Barra de ações simétrica Material Design 3 (mobile-first e desktop).
///
/// Elimina linhas de 2 botões pendurados à esquerda e o 3º botão "órfão" de meia largura:
/// - `0` filhos: [SizedBox.shrink]
/// - `1` filho: largura total (`100%`)
/// - `2` filhos: `50% / 50%` lado a lado (ou empilhados `100%` se `< 260px`)
/// - `3` filhos:
///   - `>= threeColumnBreakpoint`: `33% / 33% / 33%` em linha única
///   - `< threeColumnBreakpoint` (mobile): `50% / 50%` + `100%` (ou `100%` + `50% / 50%`
///     quando [primaryFullWidthWhenOdd] é `true`)
/// - `4+` filhos: pares `50% / 50%` e, se ímpar, item destacado em `100%` para que
///   todas as linhas ocupem 100% da largura horizontal.
class SymmetricalActionBar extends StatelessWidget {
  const SymmetricalActionBar({
    super.key,
    required this.children,
    this.spacing = 8,
    this.runSpacing = 8,
    this.threeColumnBreakpoint = 520,
    this.primaryFullWidthWhenOdd = false,
  });

  final List<Widget> children;
  final double spacing;
  final double runSpacing;
  final double threeColumnBreakpoint;
  final bool primaryFullWidthWhenOdd;

  Widget _fullWidthItem(Widget child, double? boundedWidth) {
    return SizedBox(width: boundedWidth ?? double.infinity, child: child);
  }

  Widget _equalRow(List<Widget> rowChildren) {
    return Row(
      children: <Widget>[
        for (var i = 0; i < rowChildren.length; i++) ...<Widget>[
          if (i > 0) SizedBox(width: spacing),
          Expanded(child: rowChildren[i]),
        ],
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) {
      return const SizedBox.shrink();
    }
    if (children.length == 1) {
      return SizedBox(width: double.infinity, child: children.first);
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final hasBounded = constraints.hasBoundedWidth;
        final maxWidth = hasBounded
            ? constraints.maxWidth
            : threeColumnBreakpoint;
        final fullTargetWidth = hasBounded ? maxWidth : null;

        // Em larguras ultra-estreitas (< 260px), empilha em 100% para evitar truncamento.
        if (maxWidth < 260) {
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              for (var i = 0; i < children.length; i++) ...<Widget>[
                if (i > 0) SizedBox(height: runSpacing),
                _fullWidthItem(children[i], fullTargetWidth),
              ],
            ],
          );
        }

        if (children.length == 2) {
          return _equalRow(children);
        }

        if (children.length == 3 && maxWidth >= threeColumnBreakpoint) {
          return _equalRow(children);
        }

        final rows = <Widget>[];
        if (children.length.isOdd && primaryFullWidthWhenOdd) {
          rows.add(_fullWidthItem(children.first, fullTargetWidth));
          for (var i = 1; i < children.length; i += 2) {
            rows.add(_equalRow(<Widget>[children[i], children[i + 1]]));
          }
        } else {
          final pairLimit = children.length.isOdd
              ? children.length - 1
              : children.length;
          for (var i = 0; i < pairLimit; i += 2) {
            rows.add(_equalRow(<Widget>[children[i], children[i + 1]]));
          }
          if (children.length.isOdd) {
            rows.add(_fullWidthItem(children.last, fullTargetWidth));
          }
        }

        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            for (var i = 0; i < rows.length; i++) ...<Widget>[
              if (i > 0) SizedBox(height: runSpacing),
              rows[i],
            ],
          ],
        );
      },
    );
  }
}

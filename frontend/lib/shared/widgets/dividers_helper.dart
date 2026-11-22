import 'package:flutter/material.dart';

/// Intercala `divider` entre os elementos de `items` sem adicionar divisor no fim.
List<Widget> intersperseDividers(
  Iterable<Widget> items, {
  Widget divider = const Divider(height: 1),
}) {
  final list = items.toList(growable: false);
  if (list.length <= 1) return list;
  final out = <Widget>[];
  for (var i = 0; i < list.length; i++) {
    if (i > 0) out.add(divider);
    out.add(list[i]);
  }
  return out;
}

/// Alias conciso para intercalar `Divider(height: height)` entre widgets.
List<Widget> withDividers(List<Widget> children, {double height = 1}) {
  return intersperseDividers(children, divider: Divider(height: height));
}

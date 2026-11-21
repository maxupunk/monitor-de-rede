import 'package:flutter/material.dart';

/// Campo suspenso Material 3 com `isExpanded: true` e truncamento seguro do
/// item selecionado (`TextOverflow.ellipsis`), evitando overflow horizontal ou
/// quebra de linha dentro de cards estreitos em telas mobile.
class AppDropdownFormField<T> extends StatelessWidget {
  const AppDropdownFormField({
    super.key,
    this.initialValue,
    required this.items,
    required this.onChanged,
    this.decoration = const InputDecoration(),
    this.validator,
    this.hint,
    this.disabledHint,
    this.style,
    this.isExpanded = true,
    this.isDense = true,
    this.autovalidateMode,
    this.onSaved,
  });

  final T? initialValue;
  final List<DropdownMenuItem<T>>? items;
  final ValueChanged<T?>? onChanged;
  final InputDecoration decoration;
  final FormFieldValidator<T>? validator;
  final Widget? hint;
  final Widget? disabledHint;
  final TextStyle? style;
  final bool isExpanded;
  final bool isDense;
  final AutovalidateMode? autovalidateMode;
  final FormFieldSetter<T>? onSaved;

  @override
  Widget build(BuildContext context) {
    final menuItems = items;
    return DropdownButtonFormField<T>(
      initialValue: initialValue,
      isExpanded: isExpanded,
      isDense: isDense,
      decoration: decoration,
      hint: hint,
      disabledHint: disabledHint,
      style: style,
      validator: validator,
      autovalidateMode: autovalidateMode,
      onSaved: onSaved,
      items: menuItems,
      selectedItemBuilder: menuItems == null
          ? null
          : (context) => menuItems
                .map(
                  (item) => Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: DefaultTextStyle.merge(
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      softWrap: false,
                      child: item.child,
                    ),
                  ),
                )
                .toList(growable: false),
      onChanged: onChanged,
    );
  }
}

import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Formulário dinâmico orientado por JSON Schema (`SettingsForm.vue`):
/// suporta grupos, `advanced` expansível, `visibleWhen`, segredos mascarados com
/// botão efêmero `reveal`, `enum`/`enumTitles`, listas de tags e objetos, e todos os 9 widgets.
class PluginSettingsForm extends StatefulWidget {
  const PluginSettingsForm({
    super.key,
    required this.schema,
    required this.values,
    required this.onChanged,
    this.readOnly = false,
    this.onRevealSecret,
  });

  final Map<String, dynamic>? schema;
  final Map<String, dynamic> values;
  final ValueChanged<Map<String, dynamic>> onChanged;
  final bool readOnly;

  /// Callback efêmero para campos `secret: true` com `reveal: <action>`.
  final Future<String?> Function(String fieldName, String revealAction)?
  onRevealSecret;

  @override
  State<PluginSettingsForm> createState() => _PluginSettingsFormState();
}

class _PluginSettingsFormState extends State<PluginSettingsForm> {
  final Set<String> _revealed = <String>{};
  final Set<String> _revealing = <String>{};

  void _updateValue(String name, Object? value) {
    final next = Map<String, dynamic>.from(widget.values)..[name] = value;
    widget.onChanged(next);
  }

  Future<void> _handleReveal(PluginSchemaField field) async {
    if (widget.onRevealSecret == null || field.reveal == null) return;
    setState(() => _revealing.add(field.name));
    try {
      final secret = await widget.onRevealSecret!(field.name, field.reveal!);
      if (secret != null && mounted) {
        _updateValue(field.name, secret);
        setState(() => _revealed.add(field.name));
      }
    } finally {
      if (mounted) setState(() => _revealing.remove(field.name));
    }
  }

  @override
  Widget build(BuildContext context) {
    final fields = parsePluginSchemaFields(widget.schema)
        .where(
          (f) =>
              !f.hidden &&
              isPluginFieldVisible(f, widget.schema, widget.values),
        )
        .toList(growable: false);

    if (fields.isEmpty) {
      return Text(
        'Esta ação não exige parâmetros adicionais.',
        style: context.textTheme.bodySmall?.copyWith(
          color: context.colorScheme.onSurfaceVariant,
        ),
      );
    }

    final regular = fields.where((f) => !f.advanced).toList(growable: false);
    final advanced = fields.where((f) => f.advanced).toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        ..._buildGrouped(context, regular),
        if (advanced.isNotEmpty) ...<Widget>[
          const SizedBox(height: 8),
          NestedPanel(
            padding: EdgeInsets.zero,
            child: Material(
              type: MaterialType.transparency,
              child: ExpansionTile(
                title: const Text(
                  'Opções avançadas',
                  style: TextStyle(fontWeight: FontWeight.w700, fontSize: 14),
                ),
                childrenPadding: const EdgeInsets.fromLTRB(14, 0, 14, 14),
                children: _buildGrouped(context, advanced),
              ),
            ),
          ),
        ],
      ],
    );
  }

  List<Widget> _buildGrouped(
    BuildContext context,
    List<PluginSchemaField> fields,
  ) {
    final groups = <String?, List<PluginSchemaField>>{};
    for (final f in fields) {
      groups.putIfAbsent(f.group, () => <PluginSchemaField>[]).add(f);
    }
    final widgets = <Widget>[];
    for (final entry in groups.entries) {
      if (entry.key != null && entry.key!.isNotEmpty) {
        widgets.add(
          Padding(
            padding: const EdgeInsets.only(top: 10, bottom: 6),
            child: Text(
              entry.key!,
              style: context.textTheme.labelLarge?.copyWith(
                fontWeight: FontWeight.w700,
                color: context.semanticColors.plugin,
              ),
            ),
          ),
        );
      }
      for (final field in entry.value) {
        widgets.add(
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: _buildField(context, field),
          ),
        );
      }
    }
    return widgets;
  }

  Widget _buildField(BuildContext context, PluginSchemaField field) {
    final current = widget.values[field.name] ?? field.schema['default'];

    if (field.kind == 'boolean') {
      return SwitchListTile(
        contentPadding: EdgeInsets.zero,
        title: Text(field.label),
        subtitle: field.hint != null ? Text(field.hint!) : null,
        value: current == true,
        onChanged: widget.readOnly ? null : (v) => _updateValue(field.name, v),
      );
    }

    if (field.options != null && field.options!.isNotEmpty) {
      final hasMatch = field.options!.contains(current);
      return DropdownButtonFormField<Object?>(
        initialValue: hasMatch ? current : null,
        decoration: InputDecoration(
          labelText: '${field.label}${field.required ? ' *' : ''}',
          helperText: field.hint,
        ),
        items: <DropdownMenuItem<Object?>>[
          for (var i = 0; i < field.options!.length; i++)
            DropdownMenuItem<Object?>(
              value: field.options![i],
              child: Text(
                (field.titles != null && i < field.titles!.length)
                    ? field.titles![i]
                    : (field.options![i] == ''
                          ? '(manter o atual)'
                          : '${field.options![i]}'),
              ),
            ),
        ],
        validator: (val) => validatePluginField(field, val),
        onChanged: widget.readOnly
            ? null
            : (val) => _updateValue(field.name, val),
      );
    }

    if (field.widget == 'slider') {
      final min = (field.schema['minimum'] as num?)?.toDouble() ?? 0;
      final max = (field.schema['maximum'] as num?)?.toDouble() ?? 100;
      final val = ((current is num ? current.toDouble() : min)).clamp(min, max);
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            '${field.label}: ${field.kind == 'integer' ? val.round() : val.toStringAsFixed(1)}',
            style: context.textTheme.labelMedium,
          ),
          Slider(
            min: min,
            max: max,
            value: val,
            onChanged: widget.readOnly
                ? null
                : (v) => _updateValue(
                    field.name,
                    field.kind == 'integer' ? v.round() : v,
                  ),
          ),
        ],
      );
    }

    if (field.kind == 'list' || field.widget == 'tags') {
      final list = current is List
          ? current.map((e) => e.toString()).toList(growable: false)
          : const <String>[];
      return TextFormField(
        initialValue: list.join(', '),
        enabled: !widget.readOnly,
        decoration: InputDecoration(
          labelText: '${field.label}${field.required ? ' *' : ''}',
          helperText: field.hint ?? 'Separe itens por vírgula',
        ),
        onChanged: (raw) {
          final parsed = raw
              .split(',')
              .map((s) => s.trim())
              .where((s) => s.isNotEmpty)
              .toList(growable: false);
          _updateValue(field.name, parsed);
        },
      );
    }

    final isSecret = field.secret;
    final visibleSecret = _revealed.contains(field.name);
    final loadingReveal = _revealing.contains(field.name);

    return TextFormField(
      key: ValueKey<String>(
        '${field.name}-${visibleSecret ? current : 'masked'}',
      ),
      initialValue: current?.toString() ?? '',
      enabled: !widget.readOnly,
      obscureText: isSecret && !visibleSecret,
      maxLines: field.widget == 'textarea' ? 4 : 1,
      keyboardType: (field.kind == 'integer' || field.kind == 'number')
          ? TextInputType.number
          : TextInputType.text,
      decoration: InputDecoration(
        labelText: '${field.label}${field.required ? ' *' : ''}',
        helperText: field.hint,
        suffixIcon: isSecret
            ? Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (field.reveal != null && widget.onRevealSecret != null)
                    IconButton(
                      tooltip: 'Revelar valor atual do equipamento',
                      onPressed: loadingReveal
                          ? null
                          : () => _handleReveal(field),
                      icon: loadingReveal
                          ? const SizedBox(
                              width: 14,
                              height: 14,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.vpn_key_outlined, size: 18),
                    ),
                  IconButton(
                    tooltip: visibleSecret ? 'Ocultar' : 'Mostrar',
                    icon: Icon(
                      visibleSecret ? Icons.visibility_off : Icons.visibility,
                      size: 18,
                    ),
                    onPressed: () {
                      setState(() {
                        if (visibleSecret) {
                          _revealed.remove(field.name);
                        } else {
                          _revealed.add(field.name);
                        }
                      });
                    },
                  ),
                ],
              )
            : null,
      ),
      validator: (v) => validatePluginField(field, v),
      onChanged: (v) => _updateValue(field.name, v),
    );
  }
}

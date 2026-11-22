import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_settings_form.dart';
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide CredentialView;
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Modal adaptativo para preencher parâmetros e confirmar escrita de uma ação de plugin.
/// Retorna os parâmetros normalizados se o operador confirmar a execução.
Future<Map<String, dynamic>?> showPluginActionRunnerSheet({
  required BuildContext context,
  required int deviceId,
  required PluginSummary plugin,
  required PluginAction action,
  required Future<void> Function(Future<void> Function() op) runProtectedOp,
}) {
  return showAdaptiveFormSheet<Map<String, dynamic>>(
    context: context,
    title: action.title,
    subtitle: plugin.name,
    maxWidth: 560,
    builder: (_) => _PluginActionRunnerSheetBody(
      deviceId: deviceId,
      plugin: plugin,
      action: action,
      runProtectedOp: runProtectedOp,
    ),
  );
}

class _PluginActionRunnerSheetBody extends ConsumerStatefulWidget {
  const _PluginActionRunnerSheetBody({
    required this.deviceId,
    required this.plugin,
    required this.action,
    required this.runProtectedOp,
  });

  final int deviceId;
  final PluginSummary plugin;
  final PluginAction action;
  final Future<void> Function(Future<void> Function() op) runProtectedOp;

  @override
  ConsumerState<_PluginActionRunnerSheetBody> createState() =>
      _PluginActionRunnerSheetBodyState();
}

class _PluginActionRunnerSheetBodyState
    extends ConsumerState<_PluginActionRunnerSheetBody> {
  final _formKey = GlobalKey<FormState>();
  late Map<String, dynamic> _values;
  bool _confirmWrite = false;

  @override
  void initState() {
    super.initState();
    _values = pluginSchemaDefaults(widget.action.paramsSchema);
  }

  @override
  Widget build(BuildContext context) {
    final action = widget.action;
    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (action.description != null) ...<Widget>[
            Text(action.description!),
            const SizedBox(height: 12),
          ],
          PluginSettingsForm(
            schema: action.paramsSchema,
            values: _values,
            onChanged: (next) => setState(() => _values = next),
            onRevealSecret: (fieldName, revealAction) async {
              Map<String, dynamic>? out;
              await widget.runProtectedOp(() async {
                out = await withDeviceAccess<Map<String, dynamic>>(
                  context: context,
                  ref: ref,
                  action: () => ref
                      .read(pluginsProvider.notifier)
                      .revealSecret(
                        deviceId: widget.deviceId,
                        pluginId: widget.plugin.id,
                        action: revealAction,
                        params: _values,
                      ),
                );
              });
              return out?[fieldName]?.toString() ??
                  out?['password']?.toString();
            },
          ),
          if (action.isWrite) ...<Widget>[
            const SizedBox(height: 10),
            NestedPanel(
              tone: SemanticTone.error,
              child: Material(
                type: MaterialType.transparency,
                child: CheckboxListTile(
                  contentPadding: EdgeInsets.zero,
                  value: _confirmWrite,
                  onChanged: (v) => setState(() => _confirmWrite = v == true),
                  title: Text(
                    action.confirmMessage ??
                        'Confirmo que esta ação altera a configuração do equipamento.',
                    style: const TextStyle(fontSize: 13),
                  ),
                ),
              ),
            ),
          ],
          const SizedBox(height: 14),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: <Widget>[
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('Cancelar'),
              ),
              const SizedBox(width: 8),
              FilledButton.icon(
                onPressed: action.isWrite && !_confirmWrite
                    ? null
                    : () {
                        if (!(_formKey.currentState?.validate() ?? false)) {
                          return;
                        }
                        final normalized = normalizePluginFormParams(
                          action.paramsSchema,
                          _values,
                        );
                        Navigator.of(context).pop(normalized);
                      },
                icon: const Icon(Icons.play_arrow, size: 18),
                label: const Text('Executar'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

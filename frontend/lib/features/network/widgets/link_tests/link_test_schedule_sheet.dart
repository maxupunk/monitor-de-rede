import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Future<void> showLinkTestScheduleSheet(
  BuildContext context, {
  LinkTestScheduleModel? existing,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: existing != null
        ? 'Editar Agendamento'
        : 'Novo Agendamento de Teste',
    maxWidth: 600,
    builder: (ctx) => _LinkTestScheduleFormBody(
      existing: existing,
      onDone: () => Navigator.of(ctx).pop(),
    ),
  );
}

class _LinkTestScheduleFormBody extends ConsumerStatefulWidget {
  const _LinkTestScheduleFormBody({this.existing, required this.onDone});

  final LinkTestScheduleModel? existing;
  final VoidCallback onDone;

  @override
  ConsumerState<_LinkTestScheduleFormBody> createState() =>
      _LinkTestScheduleFormBodyState();
}

class _LinkTestScheduleFormBodyState
    extends ConsumerState<_LinkTestScheduleFormBody> {
  late final TextEditingController _nameCtrl;
  late final TextEditingController _intervalCtrl;
  late final TextEditingController _durationCtrl;
  late final TextEditingController _windowStartCtrl;
  late final TextEditingController _windowEndCtrl;

  late String _kind;
  int? _linkId;
  int? _sourceId;
  int? _targetId;
  late String _direction;
  late bool _enabled;
  bool _saving = false;
  String? _formError;

  @override
  void initState() {
    super.initState();
    final existing = widget.existing;
    _nameCtrl = TextEditingController(text: existing?.name ?? '');
    _intervalCtrl = TextEditingController(
      text: (existing?.intervalMinutes ?? 60).toString(),
    );
    _durationCtrl = TextEditingController(
      text: (existing?.spec.durationS ?? 10).toString(),
    );
    _windowStartCtrl = TextEditingController(text: existing?.windowStart ?? '');
    _windowEndCtrl = TextEditingController(text: existing?.windowEnd ?? '');
    _kind = existing?.kind ?? 'server';
    _linkId = existing?.linkId;
    _sourceId = existing?.sourceDeviceId;
    _targetId = existing?.targetDeviceId;
    _direction = existing?.spec.direction ?? 'both';
    _enabled = existing?.enabled ?? true;
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _intervalCtrl.dispose();
    _durationCtrl.dispose();
    _windowStartCtrl.dispose();
    _windowEndCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit({
    required int? validSourceId,
    required int? validTargetId,
  }) async {
    if (_nameCtrl.text.trim().isEmpty) {
      setState(() => _formError = 'Informe o nome do agendamento.');
      return;
    }
    if (_kind == 'link' && _linkId == null) {
      setState(() => _formError = 'Selecione o enlace da topologia.');
      return;
    }
    if (_kind != 'link' && _kind != 'server' && validSourceId == null) {
      setState(() => _formError = 'Selecione o equipamento de origem.');
      return;
    }
    if (_kind != 'link' && validTargetId == null) {
      setState(() => _formError = 'Selecione o equipamento de destino.');
      return;
    }

    setState(() {
      _saving = true;
      _formError = null;
    });

    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'kind': _kind,
      'linkId': ?_linkId,
      'sourceDeviceId': ?validSourceId,
      'targetDeviceId': ?validTargetId,
      'spec': <String, dynamic>{
        'durationS': int.tryParse(_durationCtrl.text.trim()) ?? 10,
        'direction': _direction,
        'streams': 4,
      },
      'intervalMinutes': (int.tryParse(_intervalCtrl.text.trim()) ?? 60).clamp(
        15,
        43200,
      ),
      if (_windowStartCtrl.text.trim().isNotEmpty)
        'windowStart': _windowStartCtrl.text.trim(),
      if (_windowEndCtrl.text.trim().isNotEmpty)
        'windowEnd': _windowEndCtrl.text.trim(),
      'enabled': _enabled,
    };

    final existing = widget.existing;
    final ok = existing != null
        ? await ref
              .read(linkTestsProvider.notifier)
              .updateSchedule(existing.id, payload)
        : await ref.read(linkTestsProvider.notifier).createSchedule(payload);

    if (!mounted) return;
    if (ok) {
      widget.onDone();
      return;
    }

    setState(() {
      _saving = false;
      _formError =
          ref.read(linkTestsProvider).schedulesError ??
          'Não foi possível salvar o agendamento.';
    });
  }

  @override
  Widget build(BuildContext context) {
    final devices = ref.watch(devicesProvider).items;
    final edges = ref
        .watch(topologyProvider)
        .edges
        .where((e) => !e.isVirtualHierarchy)
        .toList(growable: false);

    final sources = filterLinkTestSourceDevices(devices);
    final targets = filterLinkTestTargetDevices(
      devices,
      kind: _kind,
      excludeSourceId: _kind != 'server' ? _sourceId : null,
    );
    final validSourceId = sources.any((d) => d.id == _sourceId)
        ? _sourceId
        : null;
    final validTargetId = targets.any((d) => d.id == _targetId)
        ? _targetId
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _nameCtrl,
          decoration: const InputDecoration(labelText: 'Nome do agendamento *'),
        ),
        const SizedBox(height: 10),
        AppDropdownFormField<String>(
          initialValue: _kind,
          decoration: const InputDecoration(labelText: 'Modo do teste'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem(
              value: 'server',
              child: Text(
                'Central -> Equipamento',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem(
              value: 'link',
              child: Text(
                'Enlace da Topologia',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem(
              value: 'end_to_end',
              child: Text(
                'Ponta a ponta (A -> B)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem(
              value: 'path',
              child: Text(
                'Caminho completo (salto a salto)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
          onChanged: (v) => setState(() {
            _kind = v ?? 'server';
            _formError = null;
          }),
        ),
        const SizedBox(height: 10),
        if (_kind == 'link')
          AppDropdownFormField<int?>(
            initialValue: edges.any((e) => e.id == _linkId) ? _linkId : null,
            decoration: const InputDecoration(labelText: 'Enlace'),
            items: edges
                .map(
                  (e) => DropdownMenuItem<int?>(
                    value: e.id,
                    child: Text(
                      '${e.sourceDeviceName ?? '#${e.sourceDeviceId}'} <-> ${e.targetDeviceName ?? '#${e.targetDeviceId}'}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) => setState(() {
              _linkId = v;
              _formError = null;
            }),
          )
        else ...<Widget>[
          if (_kind != 'server') ...<Widget>[
            AppDropdownFormField<int?>(
              initialValue: validSourceId,
              decoration: const InputDecoration(labelText: 'Origem'),
              items: sources
                  .map(
                    (d) => DropdownMenuItem<int?>(
                      value: d.id,
                      child: Text(d.name, overflow: TextOverflow.ellipsis),
                    ),
                  )
                  .toList(growable: false),
              onChanged: (v) => setState(() {
                _sourceId = v;
                if (_targetId == v) _targetId = null;
                _formError = null;
              }),
            ),
            const SizedBox(height: 10),
          ],
          AppDropdownFormField<int?>(
            initialValue: validTargetId,
            decoration: const InputDecoration(labelText: 'Destino'),
            items: targets
                .map(
                  (d) => DropdownMenuItem<int?>(
                    value: d.id,
                    child: Text(d.name, overflow: TextOverflow.ellipsis),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) => setState(() {
              _targetId = v;
              _formError = null;
            }),
          ),
        ],
        const SizedBox(height: 10),
        ResponsiveFieldRow(
          breakpoint: 640,
          children: <Widget>[
            TextField(
              controller: _intervalCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: 'Intervalo (min >= 15)',
              ),
            ),
            TextField(
              controller: _durationCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Duração (s)'),
            ),
            AppDropdownFormField<String>(
              initialValue: _direction,
              decoration: const InputDecoration(labelText: 'Sentido'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'both',
                  child: Text('Ambos', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'forward',
                  child: Text('Ida', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'reverse',
                  child: Text('Volta', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) => setState(() => _direction = v ?? 'both'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextField(
              controller: _windowStartCtrl,
              decoration: const InputDecoration(
                labelText: 'Janela início (HH:MM)',
              ),
            ),
            TextField(
              controller: _windowEndCtrl,
              decoration: const InputDecoration(
                labelText: 'Janela fim (HH:MM)',
              ),
            ),
          ],
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Agendamento ativo'),
          value: _enabled,
          onChanged: (v) => setState(() => _enabled = v),
        ),
        if (_formError != null) ...<Widget>[
          const SizedBox(height: 8),
          NestedPanel(tone: SemanticTone.error, child: Text(_formError!)),
        ],
        const SizedBox(height: 12),
        SymmetricalActionBar(
          children: <Widget>[
            OutlinedButton(
              onPressed: _saving ? null : widget.onDone,
              child: const Text('Cancelar'),
            ),
            FilledButton.icon(
              onPressed: _saving
                  ? null
                  : () => _submit(
                      validSourceId: validSourceId,
                      validTargetId: validTargetId,
                    ),
              icon: _saving
                  ? const SizedBox(
                      width: 15,
                      height: 15,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.check),
              label: Text(
                _saving
                    ? 'Salvando...'
                    : (widget.existing != null
                          ? 'Salvar'
                          : 'Criar agendamento'),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

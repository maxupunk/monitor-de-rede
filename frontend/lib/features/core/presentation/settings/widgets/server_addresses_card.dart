import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// 5. ServerAddressesCard + Sheet
class ServerAddressesCard extends ConsumerStatefulWidget {
  const ServerAddressesCard({super.key});

  @override
  ConsumerState<ServerAddressesCard> createState() =>
      _ServerAddressesCardState();
}

class _ServerAddressesCardState extends ConsumerState<ServerAddressesCard> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        ref.read(coreSettingsProvider.notifier).fetchServerAddresses();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(coreSettingsProvider);
    final auth = ref.watch(authProvider);

    return SectionCard(
      title: 'Endereços deste servidor',
      subtitle: 'Usados nos comandos de syslog, agentes remotos e túnel VPN',
      icon: Icons.dns_outlined,
      trailing: auth.isAdmin
          ? OutlinedButton.icon(
              onPressed: () => showServerAddressesSheet(context, ref),
              icon: const Icon(Icons.edit_outlined, size: 18),
              label: const Text('Editar'),
            )
          : null,
      child: settings.addressesLoading && settings.addresses.isEmpty
          ? const LoadingStateView(message: 'Detectando endereços...')
          : Column(
              children: <Widget>[
                for (final entry in settings.addresses) ...<Widget>[
                  KeyValueRow(
                    label: entry.label,
                    value: entry.value ?? 'Não detectado',
                    monospace: true,
                    copyable: entry.value != null,
                    trailing: entry.id == settings.preferredAddressId
                        ? const SemanticChip(
                            label: 'Padrão',
                            tone: SemanticTone.primary,
                            compact: true,
                          )
                        : null,
                  ),
                ],
              ],
            ),
    );
  }
}

Future<void> showServerAddressesSheet(
  BuildContext context,
  WidgetRef ref,
) async {
  await ref
      .read(coreSettingsProvider.notifier)
      .fetchServerAddresses(force: true);
  if (!context.mounted) return;

  final current = ref.read(coreSettingsProvider);
  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Endereços deste servidor',
    subtitle:
        'Corrija endereços detectados ou adicione domínios/FQDNs personalizados.',
    maxWidth: 640,
    builder: (_) => _ServerAddressesSheetBody(initialState: current),
  );
}

class _ServerAddressesSheetBody extends ConsumerStatefulWidget {
  const _ServerAddressesSheetBody({required this.initialState});

  final CoreSettingsState initialState;

  @override
  ConsumerState<_ServerAddressesSheetBody> createState() =>
      _ServerAddressesSheetBodyState();
}

class _ServerAddressesSheetBodyState
    extends ConsumerState<_ServerAddressesSheetBody> {
  late final List<ServerAddressEntry> _builtins;
  late final List<Map<String, String>> _customEntries;
  late final Map<String, TextEditingController> _overrideCtrls;
  String? _preferredId;
  bool _domainHttps = false;
  bool _saving = false;
  int _customSeq = 0;

  @override
  void initState() {
    super.initState();
    final current = widget.initialState;
    _builtins = current.addresses
        .where((e) => e.kind != 'custom')
        .toList(growable: false);
    _customEntries = current.addresses
        .where((e) => e.kind == 'custom')
        .map(
          (e) => <String, String>{
            'id': e.id,
            'label': e.label,
            'value': e.value ?? '',
          },
        )
        .toList();
    _overrideCtrls = <String, TextEditingController>{
      for (final e in _builtins)
        e.id: TextEditingController(text: e.overridden ? (e.value ?? '') : ''),
    };
    _preferredId = current.preferredAddressId;
    _domainHttps = current.domainHttps;
  }

  @override
  void dispose() {
    for (final ctrl in _overrideCtrls.values) {
      ctrl.dispose();
    }
    super.dispose();
  }

  Future<void> _save() async {
    setState(() => _saving = true);
    final overrides = <String, String>{};
    _overrideCtrls.forEach((k, ctrl) {
      if (ctrl.text.trim().isNotEmpty) {
        overrides[k] = ctrl.text.trim();
      }
    });
    final ok = await ref
        .read(coreSettingsProvider.notifier)
        .saveServerAddresses(
          overrides: overrides,
          custom: _customEntries
              .where((c) => (c['value'] ?? '').trim().isNotEmpty)
              .toList(growable: false),
          preferredId: _preferredId,
          domainHttps: _domainHttps,
        );
    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop();
    } else {
      setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final allAddresses = widget.initialState.addresses;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final entry in _builtins) ...<Widget>[
          NestedPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        entry.label,
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                    ),
                    if (_overrideCtrls[entry.id]!.text.isNotEmpty)
                      TextButton(
                        onPressed: () =>
                            setState(() => _overrideCtrls[entry.id]!.clear()),
                        child: const Text('Voltar ao detectado'),
                      ),
                  ],
                ),
                Text(
                  '${entry.description} · Detectado: ${entry.detected ?? 'nenhum'} (${entry.source})',
                  style: context.textTheme.bodySmall?.copyWith(
                    color: context.colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _overrideCtrls[entry.id],
                  onChanged: (_) => setState(() {}),
                  style: const TextStyle(fontFamily: 'monospace'),
                  decoration: InputDecoration(
                    labelText: 'Sobrescrever ${entry.label} (opcional)',
                    hintText: entry.detected ?? 'IP ou domínio',
                    isDense: true,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 10),
        ],
        Wrap(
          spacing: 8,
          runSpacing: 6,
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Text(
              'Endereços personalizados',
              style: context.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            TextButton.icon(
              onPressed: () => setState(() {
                _customSeq++;
                _customEntries.add(<String, String>{
                  'id':
                      'custom:${DateTime.now().millisecondsSinceEpoch}_$_customSeq',
                  'label': 'Domínio',
                  'value': '',
                });
              }),
              icon: const Icon(Icons.add, size: 18),
              label: const Text('Adicionar'),
            ),
          ],
        ),
        for (var i = 0; i < _customEntries.length; i++) ...<Widget>[
          Row(
            key: ValueKey<String>(_customEntries[i]['id']!),
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Expanded(
                child: ResponsiveFieldRow(
                  breakpoint: 480,
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    TextFormField(
                      initialValue: _customEntries[i]['label'],
                      decoration: const InputDecoration(
                        labelText: 'Rótulo',
                        isDense: true,
                      ),
                      onChanged: (v) => _customEntries[i]['label'] = v,
                    ),
                    TextFormField(
                      initialValue: _customEntries[i]['value'],
                      decoration: const InputDecoration(
                        labelText: 'Endereço',
                        isDense: true,
                      ),
                      onChanged: (v) => _customEntries[i]['value'] = v,
                    ),
                  ],
                ),
              ),
              IconButton(
                icon: Icon(
                  Icons.delete_outline,
                  color: context.colorScheme.error,
                ),
                onPressed: () => setState(() => _customEntries.removeAt(i)),
              ),
            ],
          ),
          const SizedBox(height: 8),
        ],
        const SizedBox(height: 8),
        AppDropdownFormField<String?>(
          initialValue: allAddresses.any((e) => e.id == _preferredId)
              ? _preferredId
              : null,
          decoration: const InputDecoration(
            labelText: 'Endereço padrão preferido',
          ),
          items: <DropdownMenuItem<String?>>[
            const DropdownMenuItem<String?>(
              value: null,
              child: Text('Automático', overflow: TextOverflow.ellipsis),
            ),
            for (final e in allAddresses)
              DropdownMenuItem<String?>(
                value: e.id,
                child: Text(e.label, overflow: TextOverflow.ellipsis),
              ),
          ],
          onChanged: (v) => setState(() => _preferredId = v),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _domainHttps,
          onChanged: (v) => setState(() => _domainHttps = v),
          title: const Text('Domínio atrás de proxy HTTPS'),
          subtitle: const Text(
            'Agentes remotos passam a usar https:// automaticamente.',
          ),
        ),
        const SizedBox(height: 14),
        FilledButton.icon(
          onPressed: _saving ? null : _save,
          icon: const Icon(Icons.save_outlined),
          label: const Text('Salvar endereços'),
        ),
      ],
    );
  }
}

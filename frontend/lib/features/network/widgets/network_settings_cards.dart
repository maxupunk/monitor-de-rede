import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── 1. VendorRegistryCard (Settings -> Rede) ──────────────────────────────────

class VendorRegistryCard extends ConsumerStatefulWidget {
  const VendorRegistryCard({super.key});

  @override
  ConsumerState<VendorRegistryCard> createState() => _VendorRegistryCardState();
}

class _VendorRegistryCardState extends ConsumerState<VendorRegistryCard> {
  final _macCtrl = TextEditingController();
  MacVendorLookup? _lookupResult;
  bool _lookingUp = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(vendorsProvider.notifier).fetchStatus(),
    );
  }

  @override
  void dispose() {
    _macCtrl.dispose();
    super.dispose();
  }

  Future<void> _testLookup() async {
    setState(() => _lookingUp = true);
    final res = await ref.read(vendorsProvider.notifier).lookup(_macCtrl.text);
    if (mounted) {
      setState(() {
        _lookupResult = res;
        _lookingUp = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(vendorsProvider);
    final status = state.status;
    final canWrite = ref.watch(authProvider).canWrite;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 520;
        final refreshButton = canWrite
            ? FilledButton.icon(
                onPressed: state.refreshing
                    ? null
                    : () =>
                          ref.read(vendorsProvider.notifier).refreshRegistry(),
                icon: state.refreshing
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.sync, size: 18),
                label: Text(
                  state.refreshing ? 'Atualizando...' : 'Atualizar IEEE',
                ),
              )
            : null;

        return SectionCard(
          title: 'Base de Fabricantes (MAC OUI)',
          subtitle:
              'Identificação automática de fabricante e tipo de equipamento pelo MAC',
          icon: Icons.factory_outlined,
          trailing: isCompact ? null : refreshButton,
          footer: isCompact && refreshButton != null
              ? Align(alignment: Alignment.centerRight, child: refreshButton)
              : null,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (state.loading && status == null)
                const LoadingStateView(message: 'Consultando base OUI...')
              else if (status != null) ...<Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    SemanticChip(
                      label: '${formatInteger(status.entries)} registros IEEE',
                      tone: SemanticTone.primary,
                    ),
                    SemanticChip(
                      label:
                          '${formatInteger(status.builtinEntries)} embutidos',
                      tone: SemanticTone.info,
                    ),
                    SemanticChip(
                      label: status.stale ? 'Desatualizada' : 'Atualizada',
                      tone: status.stale
                          ? SemanticTone.warning
                          : SemanticTone.success,
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                KeyValueRow(
                  label: 'Última atualização',
                  value: formatDateTime(
                    status.updatedAt,
                    fallback: 'Nunca atualizada',
                  ),
                ),
                if (status.sources.isNotEmpty)
                  KeyValueRow(
                    label: 'Fontes',
                    value: status.sources.join(', '),
                  ),
              ],
              const SizedBox(height: 14),
              if (constraints.maxWidth < 460)
                Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    TextField(
                      controller: _macCtrl,
                      decoration: const InputDecoration(
                        labelText: 'Testar consulta de MAC',
                        hintText: 'Ex.: DC:2C:6E:00:00:01',
                      ),
                    ),
                    const SizedBox(height: 8),
                    Align(
                      alignment: Alignment.centerRight,
                      child: OutlinedButton.icon(
                        onPressed: _lookingUp ? null : _testLookup,
                        icon: const Icon(Icons.search, size: 18),
                        label: const Text('Consultar'),
                      ),
                    ),
                  ],
                )
              else
                Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: _macCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Testar consulta de MAC',
                          hintText: 'Ex.: DC:2C:6E:00:00:01',
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton.icon(
                      onPressed: _lookingUp ? null : _testLookup,
                      icon: const Icon(Icons.search, size: 18),
                      label: const Text('Consultar'),
                    ),
                  ],
                ),
              if (_lookupResult != null) ...<Widget>[
                const SizedBox(height: 10),
                NestedPanel(
                  tone: _lookupResult!.vendor != null
                      ? SemanticTone.success
                      : SemanticTone.warning,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        _lookupResult!.vendor ?? 'Fabricante não encontrado',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      if (_lookupResult!.organization != null)
                        Text(
                          _lookupResult!.organization!,
                          style: context.textTheme.bodySmall,
                        ),
                      if (_lookupResult!.locallyAdministered)
                        const Text(
                          'MAC aleatório / administrado localmente (privacidade).',
                        ),
                      if (_lookupResult!.hintDeviceType != null)
                        Text(
                          'Sugestão de tipo: ${_lookupResult!.hintDeviceType}',
                        ),
                    ],
                  ),
                ),
              ],
            ],
          ),
        );
      },
    );
  }
}

// ── 2. WifiSettingsCard (Settings -> Wi-Fi) ───────────────────────────────────

class WifiSettingsCard extends ConsumerStatefulWidget {
  const WifiSettingsCard({super.key});

  @override
  ConsumerState<WifiSettingsCard> createState() => _WifiSettingsCardState();
}

class _WifiSettingsCardState extends ConsumerState<WifiSettingsCard> {
  final _liveCtrl = TextEditingController(text: '5');
  final _bgCtrl = TextEditingController(text: '60');
  final _retentionCtrl = TextEditingController(text: '30');
  final _pingPongCountCtrl = TextEditingController(text: '4');
  final _pingPongWindowCtrl = TextEditingController(text: '10');
  String _environment = 'office';
  bool _hydrated = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() async {
      await ref.read(wifiSettingsProvider.notifier).fetch();
      if (!mounted) return;
      _hydrate(ref.read(wifiSettingsProvider).settings);
    });
  }

  void _hydrate(WifiSettingsModel? s) {
    if (s == null) return;
    setState(() {
      _liveCtrl.text = s.liveIntervalS.toString();
      _bgCtrl.text = s.backgroundIntervalS.toString();
      _retentionCtrl.text = s.roamingRetentionDays.toString();
      _pingPongCountCtrl.text = s.pingPongCount.toString();
      _pingPongWindowCtrl.text = s.pingPongWindowMin.toString();
      _environment = s.environment;
      _hydrated = true;
    });
  }

  @override
  void dispose() {
    _liveCtrl.dispose();
    _bgCtrl.dispose();
    _retentionCtrl.dispose();
    _pingPongCountCtrl.dispose();
    _pingPongWindowCtrl.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final current = ref.read(wifiSettingsProvider).settings;
    final model = WifiSettingsModel(
      liveIntervalS: int.tryParse(_liveCtrl.text.trim()) ?? 5,
      backgroundIntervalS: int.tryParse(_bgCtrl.text.trim()) ?? 60,
      environment: _environment,
      roamingRetentionDays: int.tryParse(_retentionCtrl.text.trim()) ?? 30,
      pingPongCount: int.tryParse(_pingPongCountCtrl.text.trim()) ?? 4,
      pingPongWindowMin: int.tryParse(_pingPongWindowCtrl.text.trim()) ?? 10,
      apEnvironments: current?.apEnvironments ?? const <Map<String, dynamic>>[],
    );
    final ok = await ref.read(wifiSettingsProvider.notifier).save(model);
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Configurações de Wi-Fi salvas.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(wifiSettingsProvider);
    final canWrite = ref.watch(authProvider).canWrite;
    if (!_hydrated && state.settings != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_hydrated) _hydrate(state.settings);
      });
    }

    return SectionCard(
      title: 'Configurações de Sinais Wi-Fi',
      subtitle:
          'Cadência de leitura dos APs, ambiente de propagação RF e regras de roaming',
      icon: Icons.wifi,
      iconTone: SemanticTone.plugin,
      child: state.loading && state.settings == null
          ? const LoadingStateView(
              message: 'Carregando configurações de Wi-Fi...',
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (state.error != null) ...<Widget>[
                  NestedPanel(
                    tone: SemanticTone.error,
                    child: Text(state.error!),
                  ),
                  const SizedBox(height: 12),
                ],
                ResponsiveFieldRow(
                  breakpoint: 480,
                  children: <Widget>[
                    TextField(
                      controller: _liveCtrl,
                      enabled: canWrite,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Cadência ao vivo (s)',
                        helperText: 'Com o Mapa Wi-Fi aberto',
                      ),
                    ),
                    TextField(
                      controller: _bgCtrl,
                      enabled: canWrite,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Segundo plano (s)',
                        helperText: 'Sem mapa aberto (alertas e roaming)',
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                AppDropdownFormField<String>(
                  initialValue: _environment,
                  decoration: const InputDecoration(
                    labelText: 'Ambiente RF padrão (estimativa de distância)',
                  ),
                  items: const <DropdownMenuItem<String>>[
                    DropdownMenuItem(
                      value: 'open',
                      child: Text(
                        'Espaço aberto (pouca atenuação)',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    DropdownMenuItem(
                      value: 'office',
                      child: Text(
                        'Escritório / Divisórias (padrão)',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    DropdownMenuItem(
                      value: 'walls',
                      child: Text(
                        'Alvenaria / Muitas paredes',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                  onChanged: canWrite
                      ? (v) => setState(() => _environment = v ?? 'office')
                      : null,
                ),
                const SizedBox(height: 12),
                ResponsiveFieldRow(
                  breakpoint: 640,
                  children: <Widget>[
                    TextField(
                      controller: _retentionCtrl,
                      enabled: canWrite,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Retenção roaming (dias)',
                      ),
                    ),
                    TextField(
                      controller: _pingPongCountCtrl,
                      enabled: canWrite,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Trocas ping-pong',
                      ),
                    ),
                    TextField(
                      controller: _pingPongWindowCtrl,
                      enabled: canWrite,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Janela (min)',
                      ),
                    ),
                  ],
                ),
                if (canWrite) ...<Widget>[
                  const SizedBox(height: 14),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton.icon(
                      onPressed: state.saving ? null : _save,
                      icon: const Icon(Icons.save_outlined),
                      label: Text(
                        state.saving ? 'Salvando...' : 'Salvar configurações',
                      ),
                    ),
                  ),
                ],
              ],
            ),
    );
  }
}

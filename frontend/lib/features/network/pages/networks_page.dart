import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class NetworksPage extends ConsumerStatefulWidget {
  const NetworksPage({super.key});

  @override
  ConsumerState<NetworksPage> createState() => _NetworksPageState();
}

class _NetworksPageState extends ConsumerState<NetworksPage> {
  final TextEditingController _searchCtrl = TextEditingController();
  int? _siteFilter;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() {
      ref.read(networksProvider.notifier).fetchNetworks();
      ref.read(sitesProvider.notifier).fetchSites();
      ref.read(probesProvider.notifier).fetchProbes();
      ref.read(devicesProvider.notifier).fetchDevices();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _openNetworkSheet({NetworkModel? existing}) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing != null ? 'Editar Rede IP' : 'Nova Rede IP',
      subtitle: 'Defina sub-rede CIDR, VLAN, gateway e descoberta automática',
      maxWidth: 620,
      builder: (ctx) => _NetworkFormBody(
        existing: existing,
        onCancel: () => Navigator.of(ctx).pop(),
        onSaved: () => Navigator.of(ctx).pop(),
      ),
    );
  }

  Future<void> _confirmDelete(NetworkModel network) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Excluir rede IP',
      message: 'Remover a rede "${network.name}" (${network.cidr})?',
      confirmLabel: 'Excluir',
    );
    if (ok) {
      await ref.read(networksProvider.notifier).deleteNetwork(network.id);
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(networksProvider);
    final sites = ref.watch(sitesProvider).items;
    final devices = ref.watch(devicesProvider).items;
    final canWrite = ref.watch(authProvider).canWrite;

    final query = _searchCtrl.text.trim().toLowerCase();
    final filtered = state.items
        .where((n) {
          if (_siteFilter != null && n.siteId != _siteFilter) return false;
          if (query.isEmpty) return true;
          return '${n.name} ${n.cidr} ${n.gateway ?? ''} ${n.description ?? ''} ${n.vlanId ?? ''}'
              .toLowerCase()
              .contains(query);
        })
        .toList(growable: false);

    final activeSite = _siteFilter != null
        ? sites.where((s) => s.id == _siteFilter).firstOrNull
        : null;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        return RefreshIndicator(
          onRefresh: () => ref.read(networksProvider.notifier).fetchNetworks(),
          child: ListView(
            children: <Widget>[
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: <Widget>[
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          'Redes IP',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style:
                              (isCompact
                                      ? context.textTheme.titleLarge
                                      : context.textTheme.headlineSmall)
                                  ?.copyWith(fontWeight: FontWeight.w800),
                        ),
                        if (!isCompact) ...<Widget>[
                          const SizedBox(height: 2),
                          Text(
                            'Sub-redes CIDR, utilização IPAM e gatilhos de descoberta',
                            style: context.textTheme.bodySmall?.copyWith(
                              color: context.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                  if (canWrite) ...<Widget>[
                    const SizedBox(width: 8),
                    FilledButton.icon(
                      onPressed: () => _openNetworkSheet(),
                      icon: const Icon(Icons.add, size: 18),
                      label: Text(isCompact ? 'Nova' : 'Nova Rede'),
                    ),
                  ],
                ],
              ),
              const SizedBox(height: 10),
              UnifiedFilterBar(
                searchController: _searchCtrl,
                searchHint: 'Buscar por nome, CIDR, VLAN ou gateway...',
                onSearchChanged: (_) => setState(() {}),
                activeFilterCount: activeSite != null ? 1 : 0,
                filterSheetTitle: 'Filtrar Redes IP',
                filterSheetBuilder: sites.isEmpty
                    ? null
                    : (ctx, setSheet) => AppDropdownFormField<int?>(
                        initialValue: sites.any((s) => s.id == _siteFilter)
                            ? _siteFilter
                            : null,
                        decoration: const InputDecoration(labelText: 'Site'),
                        items: <DropdownMenuItem<int?>>[
                          const DropdownMenuItem<int?>(
                            value: null,
                            child: Text(
                              'Todos os sites',
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          for (final s in sites)
                            DropdownMenuItem<int?>(
                              value: s.id,
                              child: Text(
                                s.name,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                        ],
                        onChanged: (v) {
                          setState(() => _siteFilter = v);
                          setSheet(() {});
                        },
                      ),
                onClearFilters: () => setState(() => _siteFilter = null),
                activeChips: <ActiveFilterChipData>[
                  if (activeSite != null)
                    ActiveFilterChipData(
                      label: 'Site: ${activeSite.name}',
                      onDeleted: () => setState(() => _siteFilter = null),
                    ),
                ],
              ),
              const SizedBox(height: 14),
              if (state.loading && state.items.isEmpty)
                const LoadingStateView(message: 'Carregando redes IP...')
              else if (state.error != null && state.items.isEmpty)
                ErrorStateView(
                  message: state.error!,
                  onRetry: () =>
                      ref.read(networksProvider.notifier).fetchNetworks(),
                )
              else if (filtered.isEmpty)
                EmptyStateView(
                  title: 'Nenhuma rede IP cadastrada',
                  description:
                      'Adicione suas faixas CIDR para habilitar descoberta e IPAM.',
                  icon: Icons.lan_outlined,
                  action: canWrite
                      ? FilledButton.icon(
                          onPressed: () => _openNetworkSheet(),
                          icon: const Icon(Icons.add),
                          label: const Text('Nova Rede'),
                        )
                      : null,
                )
              else
                ResponsiveGrid(
                  xs: 1,
                  sm: 1,
                  md: 2,
                  lg: 3,
                  xl: 3,
                  children: filtered
                      .map((network) {
                        final usedCount = devices
                            .where((d) => d.networkId == network.id)
                            .length;
                        final usable = network.usableHosts ?? 254;
                        final util = usable > 0
                            ? (usedCount / usable).clamp(0.0, 1.0)
                            : 0.0;
                        final utilColor = util > 0.85
                            ? context.semanticColors.error
                            : util > 0.65
                            ? context.semanticColors.warning
                            : context.semanticColors.success;

                        return SectionCard(
                          title: network.name,
                          subtitle: network.cidr,
                          icon: Icons.lan_outlined,
                          stackTrailingOnCompact: false,
                          trailing: canWrite
                              ? Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: <Widget>[
                                    IconButton(
                                      visualDensity: VisualDensity.compact,
                                      tooltip: 'Editar rede',
                                      icon: const Icon(
                                        Icons.edit_outlined,
                                        size: 18,
                                      ),
                                      onPressed: () =>
                                          _openNetworkSheet(existing: network),
                                    ),
                                    IconButton(
                                      visualDensity: VisualDensity.compact,
                                      tooltip: 'Excluir rede',
                                      icon: Icon(
                                        Icons.delete_outline,
                                        size: 18,
                                        color: context.colorScheme.error,
                                      ),
                                      onPressed: () => _confirmDelete(network),
                                    ),
                                  ],
                                )
                              : null,
                          footer: Row(
                            children: <Widget>[
                              Expanded(
                                child: Text(
                                  'Última varredura: ${formatRelativeTime(network.lastScanAt)}',
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: context.textTheme.bodySmall,
                                ),
                              ),
                              if (canWrite && network.scannable) ...<Widget>[
                                const SizedBox(width: 8),
                                FilledButton.tonalIcon(
                                  onPressed: state.scanningId == network.id
                                      ? null
                                      : () => context.go(
                                          '/discovery?networkId=${network.id}&scan=1',
                                        ),
                                  icon: const Icon(Icons.radar, size: 16),
                                  label: const Text('Varrer'),
                                ),
                              ],
                            ],
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: <Widget>[
                              Wrap(
                                spacing: 6,
                                runSpacing: 6,
                                children: <Widget>[
                                  if (network.site != null)
                                    SemanticChip(
                                      label: network.site!.name,
                                      tone: SemanticTone.primary,
                                      icon: Icons.domain_outlined,
                                      compact: true,
                                    ),
                                  if (network.vlanId != null)
                                    SemanticChip(
                                      label: 'VLAN ${network.vlanId}',
                                      tone: SemanticTone.info,
                                      compact: true,
                                    ),
                                  if (network.gateway != null)
                                    SemanticChip(
                                      label: 'GW ${network.gateway}',
                                      tone: SemanticTone.secondary,
                                      compact: true,
                                    ),
                                  SemanticChip(
                                    label: network.scanEnabled
                                        ? 'Auto-descoberta'
                                        : 'Manual',
                                    tone: network.scanEnabled
                                        ? SemanticTone.success
                                        : SemanticTone.warning,
                                    compact: true,
                                  ),
                                ],
                              ),
                              const SizedBox(height: 12),
                              Row(
                                children: <Widget>[
                                  Expanded(
                                    child: Text(
                                      'Utilização IPAM',
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: context.textTheme.labelMedium,
                                    ),
                                  ),
                                  const SizedBox(width: 8),
                                  Text(
                                    '$usedCount / ${formatInteger(usable)} hosts (${formatPercent(util * 100)})',
                                    style: context.textTheme.labelMedium
                                        ?.copyWith(fontWeight: FontWeight.w700),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 6),
                              ClipRRect(
                                borderRadius: BorderRadius.circular(6),
                                child: LinearProgressIndicator(
                                  value: util,
                                  minHeight: 8,
                                  color: utilColor,
                                  backgroundColor: context
                                      .colorScheme
                                      .surfaceContainerHighest,
                                ),
                              ),
                              if (network.description != null &&
                                  network.description!.isNotEmpty) ...<Widget>[
                                const SizedBox(height: 8),
                                Text(
                                  network.description!,
                                  style: context.textTheme.bodySmall,
                                ),
                              ],
                            ],
                          ),
                        );
                      })
                      .toList(growable: false),
                ),
              const SizedBox(height: 24),
            ],
          ),
        );
      },
    );
  }
}

class _NetworkFormBody extends ConsumerStatefulWidget {
  const _NetworkFormBody({
    this.existing,
    required this.onCancel,
    required this.onSaved,
  });

  final NetworkModel? existing;
  final VoidCallback onCancel;
  final VoidCallback onSaved;

  @override
  ConsumerState<_NetworkFormBody> createState() => _NetworkFormBodyState();
}

class _NetworkFormBodyState extends ConsumerState<_NetworkFormBody> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _cidrCtrl;
  late final TextEditingController _gatewayCtrl;
  late final TextEditingController _vlanCtrl;
  late final TextEditingController _intervalCtrl;
  late final TextEditingController _descCtrl;
  int? _siteId;
  int? _probeId;
  bool _scanEnabled = false;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final existing = widget.existing;
    _nameCtrl = TextEditingController(text: existing?.name ?? '');
    _cidrCtrl = TextEditingController(text: existing?.cidr ?? '');
    _gatewayCtrl = TextEditingController(text: existing?.gateway ?? '');
    _vlanCtrl = TextEditingController(text: existing?.vlanId?.toString() ?? '');
    _intervalCtrl = TextEditingController(
      text: (existing?.scanInterval ?? 3600).toString(),
    );
    _descCtrl = TextEditingController(text: existing?.description ?? '');
    _siteId = existing?.siteId;
    _probeId = existing?.probeId;
    _scanEnabled = existing?.scanEnabled ?? false;
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _cidrCtrl.dispose();
    _gatewayCtrl.dispose();
    _vlanCtrl.dispose();
    _intervalCtrl.dispose();
    _descCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_saving) return;
    if (!_formKey.currentState!.validate()) return;
    setState(() => _saving = true);
    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'cidr': _cidrCtrl.text.trim(),
      'siteId': _siteId,
      'probeId': _probeId,
      if (_gatewayCtrl.text.trim().isNotEmpty)
        'gateway': _gatewayCtrl.text.trim(),
      if (_vlanCtrl.text.trim().isNotEmpty)
        'vlan': int.tryParse(_vlanCtrl.text.trim()),
      'scanEnabled': _scanEnabled,
      if (_scanEnabled)
        'scanInterval': int.tryParse(_intervalCtrl.text.trim()) ?? 3600,
      if (_descCtrl.text.trim().isNotEmpty)
        'description': _descCtrl.text.trim(),
    };
    final ok = widget.existing != null
        ? await ref
              .read(networksProvider.notifier)
              .updateNetwork(widget.existing!.id, payload)
        : await ref.read(networksProvider.notifier).createNetwork(payload);
    if (!mounted) return;
    setState(() => _saving = false);
    if (ok) {
      widget.onSaved();
    } else {
      final err = ref.read(networksProvider).error;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(err ?? 'Erro ao salvar rede.')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final sites = ref.watch(sitesProvider).items;
    final probes = ref.watch(probesProvider).items;

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextFormField(
            controller: _nameCtrl,
            decoration: const InputDecoration(labelText: 'Nome da rede *'),
            validator: FormValidators.required('Informe o nome da rede.'),
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 480,
            children: <Widget>[
              TextFormField(
                controller: _cidrCtrl,
                decoration: const InputDecoration(
                  labelText: 'Bloco CIDR *',
                  hintText: '192.168.1.0/24',
                ),
                validator: FormValidators.compose(<FormFieldValidator<String>>[
                  FormValidators.required('Informe o bloco CIDR.'),
                  FormValidators.cidr(),
                ]),
              ),
              TextFormField(
                controller: _vlanCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'VLAN ID'),
                validator: FormValidators.integerRange(1, 4094),
              ),
            ],
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _gatewayCtrl,
            decoration: const InputDecoration(
              labelText: 'Gateway padrão',
              hintText: '192.168.1.1',
            ),
            validator: FormValidators.ipv4(),
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 480,
            children: <Widget>[
              AppDropdownFormField<int?>(
                initialValue: sites.any((s) => s.id == _siteId)
                    ? _siteId
                    : null,
                decoration: const InputDecoration(labelText: 'Site'),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Nenhum', overflow: TextOverflow.ellipsis),
                  ),
                  for (final s in sites)
                    DropdownMenuItem<int?>(
                      value: s.id,
                      child: Text(s.name, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (v) => setState(() => _siteId = v),
              ),
              AppDropdownFormField<int?>(
                initialValue: probes.any((p) => p.id == _probeId)
                    ? _probeId
                    : null,
                decoration: const InputDecoration(labelText: 'Sonda remota'),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text(
                      'Central local',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  for (final p in probes)
                    DropdownMenuItem<int?>(
                      value: p.id,
                      child: Text(p.name, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (v) => setState(() => _probeId = v),
              ),
            ],
          ),
          const SizedBox(height: 10),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Descoberta periódica automática'),
            value: _scanEnabled,
            onChanged: (v) => setState(() => _scanEnabled = v),
          ),
          if (_scanEnabled)
            TextFormField(
              controller: _intervalCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: 'Intervalo de varredura (segundos)',
              ),
            ),
          const SizedBox(height: 10),
          TextFormField(
            controller: _descCtrl,
            maxLines: 2,
            decoration: const InputDecoration(labelText: 'Descrição'),
          ),
          const SizedBox(height: 16),
          SymmetricalActionBar(
            children: <Widget>[
              OutlinedButton(
                onPressed: _saving ? null : widget.onCancel,
                child: const Text('Cancelar'),
              ),
              FilledButton.icon(
                onPressed: _saving ? null : _submit,
                icon: const Icon(Icons.check),
                label: Text(widget.existing != null ? 'Salvar' : 'Criar rede'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/diagnostics_dialogs.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class DevicesPage extends ConsumerStatefulWidget {
  const DevicesPage({super.key, this.initialSiteId});

  final int? initialSiteId;

  @override
  ConsumerState<DevicesPage> createState() => _DevicesPageState();
}

class _DevicesPageState extends ConsumerState<DevicesPage> {
  final TextEditingController _searchCtrl = TextEditingController();
  String _statusFilter = 'all';
  String _typeFilter = 'all';
  int? _siteFilter;
  int? _networkFilter;
  String _vendorFilter = 'all';
  String _sortBy = 'name';
  final Set<int> _selectedIds = <int>{};

  @override
  void initState() {
    super.initState();
    _siteFilter = widget.initialSiteId;
    Future<void>.microtask(() {
      ref.read(devicesProvider.notifier).fetchDevices();
      ref.read(sitesProvider.notifier).fetchSites();
      ref.read(networksProvider.notifier).fetchNetworks();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _openBatchParentSheet(List<DeviceModel> allDevices) async {
    int? chosenParentId;
    final confirmed = await showAdaptiveFormSheet<bool>(
      context: context,
      title: 'Definir dispositivo pai em lote',
      subtitle: '${_selectedIds.length} dispositivos selecionados',
      builder: (ctx) => StatefulBuilder(
        builder: (context, setModal) {
          final candidates = allDevices
              .where((d) => !_selectedIds.contains(d.id))
              .toList(growable: false);
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              AppDropdownFormField<int?>(
                initialValue: chosenParentId,
                decoration: const InputDecoration(labelText: 'Dispositivo pai'),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text(
                      'Nenhum (raiz)',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  for (final d in candidates)
                    DropdownMenuItem<int?>(
                      value: d.id,
                      child: Text(d.name, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (v) => setModal(() => chosenParentId = v),
              ),
              const SizedBox(height: 16),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton.icon(
                  onPressed: () => Navigator.of(ctx).pop(true),
                  icon: const Icon(Icons.account_tree_outlined),
                  label: const Text('Aplicar hierarquia'),
                ),
              ),
            ],
          );
        },
      ),
    );
    if (confirmed == true) {
      final ok = await ref
          .read(devicesProvider.notifier)
          .batchSetParent(_selectedIds.toList(growable: false), chosenParentId);
      if (ok && mounted) {
        setState(() => _selectedIds.clear());
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Hierarquia atualizada em lote.')),
        );
      }
    }
  }

  Future<void> _confirmDelete(DeviceModel device) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Excluir dispositivo',
      message:
          'Deseja remover "${device.name}" e todos os seus monitores associados?',
      confirmLabel: 'Excluir',
    );
    if (!ok) return;
    final deleted = await ref
        .read(devicesProvider.notifier)
        .deleteDevice(device.id);
    if (deleted && mounted) {
      _selectedIds.remove(device.id);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('"${device.name}" removido.')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(devicesProvider);
    final sites = ref.watch(sitesProvider).items;
    final networks = ref.watch(networksProvider).items;
    final canWrite = ref.watch(authProvider).canWrite;

    final vendors =
        state.items
            .map((d) => d.vendor?.trim())
            .whereType<String>()
            .where((v) => v.isNotEmpty)
            .toSet()
            .toList()
          ..sort();

    final query = _searchCtrl.text.trim().toLowerCase();
    final filtered = state.items.where((d) {
      if (_statusFilter != 'all' && d.status != _statusFilter) return false;
      if (_typeFilter != 'all' && d.type != _typeFilter) return false;
      if (_siteFilter != null && d.siteId != _siteFilter) return false;
      if (_networkFilter != null && d.networkId != _networkFilter) return false;
      if (_vendorFilter != 'all' && d.vendor != _vendorFilter) return false;
      if (query.isNotEmpty) {
        final haystack = [
          d.name,
          d.ipAddress ?? '',
          d.macAddress ?? '',
          d.vendor ?? '',
          d.model ?? '',
          d.site?.name ?? '',
        ].join(' ').toLowerCase();
        if (!haystack.contains(query)) return false;
      }
      return true;
    }).toList();

    filtered.sort((a, b) {
      return switch (_sortBy) {
        'status' => a.status.compareTo(b.status),
        'ip' => (a.ipAddress ?? '').compareTo(b.ipAddress ?? ''),
        'type' => a.type.compareTo(b.type),
        _ => a.name.toLowerCase().compareTo(b.name.toLowerCase()),
      };
    });

    final activeSecondaryFilters =
        (_typeFilter != 'all' ? 1 : 0) +
        (_siteFilter != null ? 1 : 0) +
        (_networkFilter != null ? 1 : 0) +
        (_vendorFilter != 'all' ? 1 : 0);

    final statusChips = <(String, String)>[
      ('all', 'Todos (${state.totalCount})'),
      ('online', 'Online (${state.onlineCount})'),
      ('offline', 'Offline (${state.offlineCount})'),
      ('warning', 'Atenção (${state.warningCount})'),
      if (state.unknownCount > 0) ('unknown', 'Desc. (${state.unknownCount})'),
    ];

    final activeSiteName = _siteFilter != null
        ? sites.where((s) => s.id == _siteFilter).firstOrNull?.name
        : null;
    final activeNetName = _networkFilter != null
        ? networks.where((n) => n.id == _networkFilter).firstOrNull?.name
        : null;

    final activeChips = <ActiveFilterChipData>[
      if (_typeFilter != 'all')
        ActiveFilterChipData(
          label: deviceTypeLabel(_typeFilter),
          onDeleted: () => setState(() => _typeFilter = 'all'),
        ),
      if (activeSiteName != null)
        ActiveFilterChipData(
          label: 'Site: $activeSiteName',
          onDeleted: () => setState(() => _siteFilter = null),
        ),
      if (activeNetName != null)
        ActiveFilterChipData(
          label: 'Rede: $activeNetName',
          onDeleted: () => setState(() => _networkFilter = null),
        ),
      if (_vendorFilter != 'all')
        ActiveFilterChipData(
          label: _vendorFilter,
          onDeleted: () => setState(() => _vendorFilter = 'all'),
        ),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        return RefreshIndicator(
          onRefresh: () => ref.read(devicesProvider.notifier).fetchDevices(),
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
                          'Dispositivos',
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
                            'Inventário de equipamentos monitorados com status ao vivo via SSE',
                            style: context.textTheme.bodySmall?.copyWith(
                              color: context.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                  if (!isCompact) ...<Widget>[
                    OutlinedButton.icon(
                      onPressed: () => showSnmpProfilesDialog(context),
                      icon: const Icon(Icons.tune, size: 18),
                      label: const Text('Perfis SNMP'),
                    ),
                    if (canWrite) ...<Widget>[
                      const SizedBox(width: 8),
                      FilledButton.icon(
                        onPressed: () => showDeviceFormSheet(context: context),
                        icon: const Icon(Icons.add, size: 18),
                        label: const Text('Novo Dispositivo'),
                      ),
                    ],
                  ] else ...<Widget>[
                    if (canWrite) ...<Widget>[
                      const SizedBox(width: 8),
                      FilledButton.icon(
                        onPressed: () => showDeviceFormSheet(context: context),
                        icon: const Icon(Icons.add, size: 18),
                        label: const Text('Novo'),
                      ),
                    ],
                    PopupMenuButton<String>(
                      tooltip: 'Mais ações',
                      icon: const Icon(Icons.more_vert),
                      onSelected: (value) {
                        if (value == 'snmp_profiles') {
                          showSnmpProfilesDialog(context);
                        }
                      },
                      itemBuilder: (_) => const <PopupMenuEntry<String>>[
                        PopupMenuItem<String>(
                          value: 'snmp_profiles',
                          child: ListTile(
                            dense: true,
                            contentPadding: EdgeInsets.zero,
                            leading: Icon(Icons.tune),
                            title: Text('Perfis SNMP'),
                          ),
                        ),
                      ],
                    ),
                  ],
                ],
              ),
              const SizedBox(height: 10),
              UnifiedFilterBar(
                searchController: _searchCtrl,
                searchHint: 'Buscar nome, IP, MAC, fabricante ou modelo...',
                onSearchChanged: (_) => setState(() {}),
                activeFilterCount: activeSecondaryFilters,
                filterSheetTitle: 'Filtrar Dispositivos',
                filterSheetBuilder: (sheetCtx, setSheet) => Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    AppDropdownFormField<String>(
                      initialValue: _typeFilter,
                      decoration: const InputDecoration(labelText: 'Tipo'),
                      items: <DropdownMenuItem<String>>[
                        const DropdownMenuItem<String>(
                          value: 'all',
                          child: Text(
                            'Todos os tipos',
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        for (final opt in deviceTypeOptions)
                          DropdownMenuItem<String>(
                            value: opt.$1,
                            child: Text(
                              opt.$2,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                      ],
                      onChanged: (v) {
                        setState(() => _typeFilter = v ?? 'all');
                        setSheet(() {});
                      },
                    ),
                    if (sites.isNotEmpty || networks.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 12),
                      ResponsiveFieldRow(
                        breakpoint: 480,
                        children: <Widget>[
                          if (sites.isNotEmpty)
                            AppDropdownFormField<int?>(
                              initialValue:
                                  sites.any((s) => s.id == _siteFilter)
                                  ? _siteFilter
                                  : null,
                              decoration: const InputDecoration(
                                labelText: 'Site',
                              ),
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
                          if (networks.isNotEmpty)
                            AppDropdownFormField<int?>(
                              initialValue:
                                  networks.any((n) => n.id == _networkFilter)
                                  ? _networkFilter
                                  : null,
                              decoration: const InputDecoration(
                                labelText: 'Rede IP',
                              ),
                              items: <DropdownMenuItem<int?>>[
                                const DropdownMenuItem<int?>(
                                  value: null,
                                  child: Text(
                                    'Todas as redes',
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                                for (final n in networks)
                                  DropdownMenuItem<int?>(
                                    value: n.id,
                                    child: Text(
                                      n.name,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                              ],
                              onChanged: (v) {
                                setState(() => _networkFilter = v);
                                setSheet(() {});
                              },
                            ),
                        ],
                      ),
                    ],
                    if (vendors.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 12),
                      AppDropdownFormField<String>(
                        initialValue: vendors.contains(_vendorFilter)
                            ? _vendorFilter
                            : 'all',
                        decoration: const InputDecoration(
                          labelText: 'Fabricante',
                        ),
                        items: <DropdownMenuItem<String>>[
                          const DropdownMenuItem<String>(
                            value: 'all',
                            child: Text(
                              'Todos os fabricantes',
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          for (final v in vendors)
                            DropdownMenuItem<String>(
                              value: v,
                              child: Text(v, overflow: TextOverflow.ellipsis),
                            ),
                        ],
                        onChanged: (v) {
                          setState(() => _vendorFilter = v ?? 'all');
                          setSheet(() {});
                        },
                      ),
                    ],
                  ],
                ),
                onClearFilters: () => setState(() {
                  _typeFilter = 'all';
                  _siteFilter = null;
                  _networkFilter = null;
                  _vendorFilter = 'all';
                }),
                sortOptions: const <FilterSortOption>[
                  FilterSortOption(value: 'name', label: 'Nome'),
                  FilterSortOption(value: 'status', label: 'Status'),
                  FilterSortOption(value: 'ip', label: 'Endereço IP'),
                  FilterSortOption(value: 'type', label: 'Tipo'),
                ],
                activeSort: _sortBy,
                onSortChanged: (v) => setState(() => _sortBy = v),
                quickChips: statusChips
                    .map(
                      (opt) => ChoiceChip(
                        label: Text(opt.$2),
                        selected: _statusFilter == opt.$1,
                        onSelected: (_) =>
                            setState(() => _statusFilter = opt.$1),
                      ),
                    )
                    .toList(growable: false),
                activeChips: activeChips,
              ),
              if (!isCompact) ...<Widget>[
                const SizedBox(height: 14),
                ResponsiveGrid(
                  xs: 2,
                  sm: 2,
                  md: 4,
                  lg: 4,
                  xl: 4,
                  children: <Widget>[
                    KpiCard(
                      label: 'Total',
                      value: formatInteger(state.totalCount),
                      icon: Icons.devices_outlined,
                      tone: SemanticTone.primary,
                      onTap: () => setState(() => _statusFilter = 'all'),
                    ),
                    KpiCard(
                      label: 'Online',
                      value: formatInteger(state.onlineCount),
                      icon: Icons.check_circle_outline,
                      tone: SemanticTone.success,
                      onTap: () => setState(() => _statusFilter = 'online'),
                    ),
                    KpiCard(
                      label: 'Offline',
                      value: formatInteger(state.offlineCount),
                      icon: Icons.error_outline,
                      tone: SemanticTone.error,
                      onTap: () => setState(() => _statusFilter = 'offline'),
                    ),
                    KpiCard(
                      label: 'Atenção / Desc.',
                      value: formatInteger(
                        state.warningCount + state.unknownCount,
                      ),
                      icon: Icons.warning_amber_outlined,
                      tone: SemanticTone.warning,
                      onTap: () => setState(() => _statusFilter = 'warning'),
                    ),
                  ],
                ),
              ],
              if (_selectedIds.isNotEmpty && canWrite) ...<Widget>[
                const SizedBox(height: 12),
                NestedPanel(
                  tone: SemanticTone.primary,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 14,
                    vertical: 10,
                  ),
                  child: Wrap(
                    spacing: 10,
                    runSpacing: 8,
                    alignment: WrapAlignment.spaceBetween,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      Text(
                        countNoun(
                          _selectedIds.length,
                          'dispositivo selecionado',
                          'dispositivos selecionados',
                        ),
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      Wrap(
                        spacing: 8,
                        runSpacing: 6,
                        children: <Widget>[
                          FilledButton.icon(
                            onPressed: () => _openBatchParentSheet(state.items),
                            icon: const Icon(
                              Icons.account_tree_outlined,
                              size: 18,
                            ),
                            label: const Text('Definir pai em lote'),
                          ),
                          TextButton(
                            onPressed: () =>
                                setState(() => _selectedIds.clear()),
                            child: const Text('Limpar seleção'),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
              const SizedBox(height: 12),
              if (state.loading && state.items.isEmpty)
                const LoadingStateView(message: 'Carregando dispositivos...')
              else if (state.error != null && state.items.isEmpty)
                ErrorStateView(
                  message: state.error!,
                  onRetry: () =>
                      ref.read(devicesProvider.notifier).fetchDevices(),
                )
              else if (filtered.isEmpty)
                EmptyStateView(
                  title: 'Nenhum dispositivo encontrado',
                  description: state.items.isEmpty
                      ? 'Cadastre seu primeiro equipamento ou execute uma descoberta de rede.'
                      : 'Ajuste os filtros para visualizar outros dispositivos.',
                  icon: Icons.devices_other_outlined,
                  action: canWrite
                      ? FilledButton.icon(
                          onPressed: () =>
                              showDeviceFormSheet(context: context),
                          icon: const Icon(Icons.add),
                          label: const Text('Novo Dispositivo'),
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
                      .map((device) {
                        final selected = _selectedIds.contains(device.id);
                        return _DeviceCard(
                          device: device,
                          canWrite: canWrite,
                          selected: selected,
                          onToggleSelect: canWrite
                              ? (v) {
                                  setState(() {
                                    if (v == true) {
                                      _selectedIds.add(device.id);
                                    } else {
                                      _selectedIds.remove(device.id);
                                    }
                                  });
                                }
                              : null,
                          onOpen: () => context.go('/devices/${device.id}'),
                          onEdit: () => showDeviceFormSheet(
                            context: context,
                            existing: device,
                          ),
                          onDelete: () => _confirmDelete(device),
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

class _DeviceCard extends StatelessWidget {
  const _DeviceCard({
    required this.device,
    required this.canWrite,
    required this.selected,
    required this.onToggleSelect,
    required this.onOpen,
    required this.onEdit,
    required this.onDelete,
  });

  final DeviceModel device;
  final bool canWrite;
  final bool selected;
  final ValueChanged<bool?>? onToggleSelect;
  final VoidCallback onOpen;
  final VoidCallback onEdit;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: selected
            ? BorderSide(color: scheme.primary, width: 2)
            : BorderSide.none,
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onOpen,
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  if (onToggleSelect != null)
                    SizedBox(
                      width: 28,
                      height: 28,
                      child: Checkbox(
                        value: selected,
                        onChanged: onToggleSelect,
                        visualDensity: VisualDensity.compact,
                      ),
                    ),
                  Container(
                    width: 40,
                    height: 40,
                    decoration: BoxDecoration(
                      color: scheme.primaryContainer,
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Icon(
                      deviceTypeIcon(device.type),
                      color: scheme.onPrimaryContainer,
                      size: 22,
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          device.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          device.ipAddress ?? 'Sem IP cadastrado',
                          style: context.textTheme.bodySmall?.copyWith(
                            fontFamily: 'monospace',
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 8),
                  StatusIndicator.fromStatus(device.status, compact: true),
                ],
              ),
              const SizedBox(height: 10),
              Wrap(
                spacing: 6,
                runSpacing: 6,
                children: <Widget>[
                  SemanticChip(
                    label: deviceTypeLabel(device.type),
                    tone: SemanticTone.info,
                    compact: true,
                  ),
                  if (device.site != null)
                    SemanticChip(
                      label: device.site!.name,
                      tone: SemanticTone.secondary,
                      icon: Icons.domain_outlined,
                      compact: true,
                    ),
                  if (device.snmpEnabled)
                    const SemanticChip(
                      label: 'SNMP',
                      tone: SemanticTone.success,
                      compact: true,
                    ),
                  if (device.isSystem)
                    const SemanticChip(
                      label: 'Sistema',
                      tone: SemanticTone.primary,
                      compact: true,
                    ),
                ],
              ),
              if (device.vendor != null ||
                  device.model != null ||
                  device.macAddress != null) ...<Widget>[
                const SizedBox(height: 8),
                Text(
                  [
                    if (device.vendor != null) device.vendor!,
                    if (device.model != null) device.model!,
                    if (device.macAddress != null) device.macAddress!,
                  ].join(' · '),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
              const SizedBox(height: 10),
              const Divider(height: 1),
              const SizedBox(height: 6),
              Row(
                children: <Widget>[
                  Expanded(
                    child: Wrap(
                      spacing: 2,
                      children: <Widget>[
                        if (device.hasIp && !device.isSystem)
                          IconButton(
                            tooltip: 'Ping rápido',
                            visualDensity: VisualDensity.compact,
                            icon: const Icon(Icons.network_ping, size: 18),
                            onPressed: () => showQuickPingDialog(
                              context,
                              deviceId: device.id,
                              deviceName: device.name,
                            ),
                          ),
                        if (device.hasIp) ...<Widget>[
                          IconButton(
                            tooltip: 'Traceroute',
                            visualDensity: VisualDensity.compact,
                            icon: const Icon(Icons.alt_route, size: 18),
                            onPressed: () => showTracerouteDialog(
                              context,
                              initialHost: device.ipAddress,
                            ),
                          ),
                          IconButton(
                            tooltip: 'Port Scan',
                            visualDensity: VisualDensity.compact,
                            icon: const Icon(Icons.radar, size: 18),
                            onPressed: () => showPortScanDialog(
                              context,
                              initialHost: device.ipAddress,
                            ),
                          ),
                        ],
                        IconButton(
                          tooltip: 'Playbook de diagnóstico',
                          visualDensity: VisualDensity.compact,
                          icon: const Icon(Icons.playlist_play, size: 18),
                          onPressed: () => showDiagnosticPlaybookDialog(
                            context,
                            device: device,
                          ),
                        ),
                      ],
                    ),
                  ),
                  if (canWrite)
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        IconButton(
                          tooltip: 'Editar',
                          visualDensity: VisualDensity.compact,
                          icon: const Icon(Icons.edit_outlined, size: 18),
                          onPressed: onEdit,
                        ),
                        if (!device.isSystem)
                          IconButton(
                            tooltip: 'Excluir',
                            visualDensity: VisualDensity.compact,
                            icon: Icon(
                              Icons.delete_outline,
                              size: 18,
                              color: scheme.error,
                            ),
                            onPressed: onDelete,
                          ),
                      ],
                    ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

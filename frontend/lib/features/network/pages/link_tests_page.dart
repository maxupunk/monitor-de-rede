import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

String _linkTestKindLabel(String kind) => switch (kind.trim().toLowerCase()) {
  'link' => 'Enlace',
  'server' => 'Servidor',
  'end_to_end' => 'Ponta a ponta',
  'path' => 'Caminho',
  _ => kind,
};

String _linkTestDirectionLabel(String direction) =>
    switch (direction.trim().toLowerCase()) {
      'both' => 'Ida e volta',
      'forward' => 'Somente ida',
      'reverse' => 'Somente volta',
      _ => direction,
    };

String _linkTestStatusLabel(String status) =>
    switch (status.trim().toLowerCase()) {
      'completed' || 'succeeded' => 'Concluído',
      'running' => 'Em execução',
      'preparing' => 'Preparando',
      'queued' || 'pending' => 'Na fila',
      'failed' || 'error' => 'Falhou',
      'cancelled' || 'canceled' => 'Cancelado',
      _ => status,
    };

StatusIndicator _buildLinkTestStatus(String status, {bool compact = true}) {
  final normalized = status.trim().toLowerCase();
  return switch (normalized) {
    'completed' || 'succeeded' => StatusIndicator(
      label: 'Concluído',
      tone: SemanticTone.success,
      compact: compact,
    ),
    'running' => StatusIndicator(
      label: 'Em execução',
      tone: SemanticTone.info,
      compact: compact,
    ),
    'preparing' => StatusIndicator(
      label: 'Preparando',
      tone: SemanticTone.info,
      compact: compact,
    ),
    'queued' || 'pending' => StatusIndicator(
      label: 'Na fila',
      tone: SemanticTone.secondary,
      compact: compact,
    ),
    'failed' || 'error' => StatusIndicator(
      label: 'Falhou',
      tone: SemanticTone.error,
      compact: compact,
    ),
    'cancelled' || 'canceled' => StatusIndicator(
      label: 'Cancelado',
      tone: SemanticTone.warning,
      compact: compact,
    ),
    _ => StatusIndicator.fromStatus(status, compact: compact),
  };
}

class LinkTestsPage extends ConsumerStatefulWidget {
  const LinkTestsPage({
    super.key,
    this.initialTab,
    this.initialTestId,
    this.initialDeviceId,
  });

  final String? initialTab;
  final int? initialTestId;
  final int? initialDeviceId;

  @override
  ConsumerState<LinkTestsPage> createState() => _LinkTestsPageState();
}

class _LinkTestsPageState extends ConsumerState<LinkTestsPage>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  String? _statusFilter;
  String? _kindFilter;
  int? _deviceFilter;
  LinkTestModel? _compareA;
  LinkTestModel? _compareB;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(
      length: 2,
      vsync: this,
      initialIndex: widget.initialTab == 'schedules' ? 1 : 0,
    );
    _deviceFilter = widget.initialDeviceId;
    Future<void>.microtask(_bootstrap);
  }

  Future<void> _bootstrap() async {
    await Future.wait(<Future<void>>[
      ref.read(devicesProvider.notifier).fetchDevices(),
      ref.read(topologyProvider.notifier).fetchTopology(showLoading: false),
      ref.read(linkTestsProvider.notifier).fetchPage(deviceId: _deviceFilter),
      ref.read(linkTestsProvider.notifier).fetchSchedules(),
    ]);
    if (!mounted) return;
    if (widget.initialTestId != null) {
      await ref
          .read(linkTestsProvider.notifier)
          .fetchDetail(widget.initialTestId!);
      ref.read(linkTestsProvider.notifier).focus(widget.initialTestId);
    }
    final draft = ref.read(pendingLinkTestDraftProvider);
    if (draft != null) {
      ref.read(pendingLinkTestDraftProvider.notifier).state = null;
      _openWizard(draft: draft);
    }
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  Future<void> _openWizard({LinkTestRequestDraft? draft}) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Novo Teste de Velocidade / Enlace',
      subtitle:
          'Planeje pernas, resolva credenciais e acompanhe a medição em tempo real',
      maxWidth: 680,
      builder: (ctx) => _LinkTestWizardBody(
        initialDraft: draft,
        onStarted: (id) {
          Navigator.of(ctx).pop();
          ref.read(linkTestsProvider.notifier).focus(id);
        },
      ),
    );
  }

  Future<void> _openScheduleSheet({LinkTestScheduleModel? existing}) async {
    final nameCtrl = TextEditingController(text: existing?.name ?? '');
    final intervalCtrl = TextEditingController(
      text: (existing?.intervalMinutes ?? 60).toString(),
    );
    final durationCtrl = TextEditingController(
      text: (existing?.spec.durationS ?? 10).toString(),
    );
    final windowStartCtrl = TextEditingController(
      text: existing?.windowStart ?? '',
    );
    final windowEndCtrl = TextEditingController(
      text: existing?.windowEnd ?? '',
    );
    String kind = existing?.kind ?? 'server';
    int? linkId = existing?.linkId;
    int? sourceId = existing?.sourceDeviceId;
    int? targetId = existing?.targetDeviceId;
    String direction = existing?.spec.direction ?? 'both';
    bool enabled = existing?.enabled ?? true;
    bool saving = false;

    final devices = ref.read(devicesProvider).items;
    final edges = ref.read(topologyProvider).edges;

    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing != null
          ? 'Editar Agendamento'
          : 'Novo Agendamento de Teste',
      maxWidth: 600,
      builder: (ctx) => StatefulBuilder(
        builder: (context, setModal) => Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            TextField(
              controller: nameCtrl,
              decoration: const InputDecoration(
                labelText: 'Nome do agendamento *',
              ),
            ),
            const SizedBox(height: 10),
            AppDropdownFormField<String>(
              initialValue: kind,
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
              onChanged: (v) => setModal(() => kind = v ?? 'server'),
            ),
            const SizedBox(height: 10),
            if (kind == 'link')
              AppDropdownFormField<int?>(
                initialValue: edges.any((e) => e.id == linkId) ? linkId : null,
                decoration: const InputDecoration(labelText: 'Enlace'),
                items: edges
                    .where((e) => !e.isVirtualHierarchy)
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
                onChanged: (v) => setModal(() => linkId = v),
              )
            else ...<Widget>[
              if (kind != 'server') ...<Widget>[
                AppDropdownFormField<int?>(
                  initialValue: devices.any((d) => d.id == sourceId)
                      ? sourceId
                      : null,
                  decoration: const InputDecoration(labelText: 'Origem'),
                  items: devices
                      .map(
                        (d) => DropdownMenuItem<int?>(
                          value: d.id,
                          child: Text(d.name, overflow: TextOverflow.ellipsis),
                        ),
                      )
                      .toList(growable: false),
                  onChanged: (v) => setModal(() => sourceId = v),
                ),
                const SizedBox(height: 10),
              ],
              AppDropdownFormField<int?>(
                initialValue: devices.any((d) => d.id == targetId)
                    ? targetId
                    : null,
                decoration: const InputDecoration(labelText: 'Destino'),
                items: devices
                    .map(
                      (d) => DropdownMenuItem<int?>(
                        value: d.id,
                        child: Text(d.name, overflow: TextOverflow.ellipsis),
                      ),
                    )
                    .toList(growable: false),
                onChanged: (v) => setModal(() => targetId = v),
              ),
            ],
            const SizedBox(height: 10),
            ResponsiveFieldRow(
              breakpoint: 640,
              children: <Widget>[
                TextField(
                  controller: intervalCtrl,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: 'Intervalo (min >= 15)',
                  ),
                ),
                TextField(
                  controller: durationCtrl,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(labelText: 'Duração (s)'),
                ),
                AppDropdownFormField<String>(
                  initialValue: direction,
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
                  onChanged: (v) => setModal(() => direction = v ?? 'both'),
                ),
              ],
            ),
            const SizedBox(height: 10),
            ResponsiveFieldRow(
              breakpoint: 480,
              children: <Widget>[
                TextField(
                  controller: windowStartCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Janela início (HH:MM)',
                  ),
                ),
                TextField(
                  controller: windowEndCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Janela fim (HH:MM)',
                  ),
                ),
              ],
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Agendamento ativo'),
              value: enabled,
              onChanged: (v) => setModal(() => enabled = v),
            ),
            const SizedBox(height: 12),
            SymmetricalActionBar(
              children: <Widget>[
                OutlinedButton(
                  onPressed: saving ? null : () => Navigator.of(ctx).pop(),
                  child: const Text('Cancelar'),
                ),
                FilledButton.icon(
                  onPressed: saving
                      ? null
                      : () async {
                          if (nameCtrl.text.trim().isEmpty) return;
                          setModal(() => saving = true);
                          final payload = <String, dynamic>{
                            'name': nameCtrl.text.trim(),
                            'kind': kind,
                            'linkId': ?linkId,
                            'sourceDeviceId': ?sourceId,
                            'targetDeviceId': ?targetId,
                            'spec': <String, dynamic>{
                              'durationS':
                                  int.tryParse(durationCtrl.text.trim()) ?? 10,
                              'direction': direction,
                              'streams': 4,
                            },
                            'intervalMinutes':
                                (int.tryParse(intervalCtrl.text.trim()) ?? 60)
                                    .clamp(15, 43200),
                            if (windowStartCtrl.text.trim().isNotEmpty)
                              'windowStart': windowStartCtrl.text.trim(),
                            if (windowEndCtrl.text.trim().isNotEmpty)
                              'windowEnd': windowEndCtrl.text.trim(),
                            'enabled': enabled,
                          };
                          final ok = existing != null
                              ? await ref
                                    .read(linkTestsProvider.notifier)
                                    .updateSchedule(existing.id, payload)
                              : await ref
                                    .read(linkTestsProvider.notifier)
                                    .createSchedule(payload);
                          if (ctx.mounted && ok) Navigator.of(ctx).pop();
                          if (context.mounted) setModal(() => saving = false);
                        },
                  icon: const Icon(Icons.check),
                  label: Text(
                    existing != null ? 'Salvar' : 'Criar agendamento',
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
    nameCtrl.dispose();
    intervalCtrl.dispose();
    durationCtrl.dispose();
    windowStartCtrl.dispose();
    windowEndCtrl.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(linkTestsProvider);
    final devices = ref.watch(devicesProvider).items;
    final canWrite = ref.watch(authProvider).canWrite;
    final focused = state.focused ?? state.activeTests.firstOrNull;

    final activeFilterCount =
        (_deviceFilter != null ? 1 : 0) +
        (_statusFilter != null ? 1 : 0) +
        (_kindFilter != null ? 1 : 0);

    final activeDeviceName = _deviceFilter != null
        ? devices.where((d) => d.id == _deviceFilter).firstOrNull?.name
        : null;

    final activeChips = <ActiveFilterChipData>[
      if (activeDeviceName != null)
        ActiveFilterChipData(
          label: activeDeviceName,
          onDeleted: () {
            setState(() => _deviceFilter = null);
            ref
                .read(linkTestsProvider.notifier)
                .fetchPage(page: 1, status: _statusFilter, kind: _kindFilter);
          },
        ),
      if (_statusFilter != null)
        ActiveFilterChipData(
          label: 'Status: ${_linkTestStatusLabel(_statusFilter!)}',
          onDeleted: () {
            setState(() => _statusFilter = null);
            ref
                .read(linkTestsProvider.notifier)
                .fetchPage(page: 1, deviceId: _deviceFilter, kind: _kindFilter);
          },
        ),
      if (_kindFilter != null)
        ActiveFilterChipData(
          label: 'Modo: ${_linkTestKindLabel(_kindFilter!)}',
          onDeleted: () {
            setState(() => _kindFilter = null);
            ref
                .read(linkTestsProvider.notifier)
                .fetchPage(
                  page: 1,
                  deviceId: _deviceFilter,
                  status: _statusFilter,
                );
          },
        ),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        return NestedScrollView(
          headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
            SliverToBoxAdapter(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  if (isCompact)
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            'Testes de Enlace',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                            ),
                          ),
                        ),
                        if (canWrite) ...<Widget>[
                          const SizedBox(width: 8),
                          FilledButton.icon(
                            onPressed: () => _openWizard(),
                            icon: const Icon(Icons.speed, size: 18),
                            label: const Text('Novo'),
                          ),
                          PopupMenuButton<String>(
                            tooltip: 'Mais ações',
                            icon: const Icon(Icons.more_vert),
                            onSelected: (v) {
                              if (v == 'schedule') _openScheduleSheet();
                            },
                            itemBuilder: (_) => const <PopupMenuEntry<String>>[
                              PopupMenuItem<String>(
                                value: 'schedule',
                                child: ListTile(
                                  dense: true,
                                  contentPadding: EdgeInsets.zero,
                                  leading: Icon(Icons.schedule),
                                  title: Text('Novo agendamento'),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ],
                    )
                  else
                    SearchFilterHeader(
                      title: 'Testes de Velocidade e Enlaces',
                      subtitle:
                          'Medição ativa entre equipamentos via iPerf3/BTest com telemetria ao vivo via SSE',
                      actions: <Widget>[
                        if (canWrite) ...<Widget>[
                          OutlinedButton.icon(
                            onPressed: () => _openScheduleSheet(),
                            icon: const Icon(Icons.schedule, size: 18),
                            label: const Text('Agendar'),
                          ),
                          FilledButton.icon(
                            onPressed: () => _openWizard(),
                            icon: const Icon(Icons.speed, size: 18),
                            label: const Text('Novo Teste'),
                          ),
                        ],
                      ],
                    ),
                  if (focused != null) ...<Widget>[
                    const SizedBox(height: 10),
                    _FocusedLinkTestCard(
                      test: focused,
                      state: state,
                      canWrite: canWrite,
                      onClose: () =>
                          ref.read(linkTestsProvider.notifier).focus(null),
                      onCancel: () => ref
                          .read(linkTestsProvider.notifier)
                          .cancel(focused.id),
                    ),
                  ],
                  if (_compareA != null && _compareB != null) ...<Widget>[
                    const SizedBox(height: 10),
                    _LinkTestComparisonCard(
                      a: _compareA!,
                      b: _compareB!,
                      onClear: () => setState(() {
                        _compareA = null;
                        _compareB = null;
                      }),
                    ),
                  ],
                  const SizedBox(height: 8),
                ],
              ),
            ),
            SliverPersistentHeader(
              pinned: true,
              delegate: _LinkTestsTabBarDelegate(
                backgroundColor: Theme.of(context).scaffoldBackgroundColor,
                tabBar: TabBar(
                  controller: _tabController,
                  isScrollable: isCompact,
                  tabAlignment: isCompact
                      ? TabAlignment.start
                      : TabAlignment.fill,
                  labelPadding: const EdgeInsets.symmetric(horizontal: 14),
                  tabs: <Widget>[
                    Tab(text: 'Histórico (${state.total})'),
                    Tab(text: 'Agendamentos (${state.schedules.length})'),
                  ],
                ),
              ),
            ),
          ],
          body: TabBarView(
            controller: _tabController,
            children: <Widget>[
              // ── Tab 1: Histórico ──
              RefreshIndicator(
                onRefresh: () => ref
                    .read(linkTestsProvider.notifier)
                    .fetchPage(
                      page: state.page,
                      deviceId: _deviceFilter,
                      status: _statusFilter,
                      kind: _kindFilter,
                    ),
                child: ListView(
                  padding: const EdgeInsets.only(top: 10, bottom: 20),
                  children: <Widget>[
                    UnifiedFilterBar(
                      activeFilterCount: activeFilterCount,
                      filterSheetTitle: 'Filtrar Histórico de Testes',
                      filterSheetBuilder: (sheetCtx, setSheet) => Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          AppDropdownFormField<int?>(
                            initialValue:
                                devices.any((d) => d.id == _deviceFilter)
                                ? _deviceFilter
                                : null,
                            decoration: const InputDecoration(
                              labelText: 'Equipamento',
                            ),
                            items: <DropdownMenuItem<int?>>[
                              const DropdownMenuItem<int?>(
                                value: null,
                                child: Text(
                                  'Todos',
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                              for (final d in devices)
                                DropdownMenuItem<int?>(
                                  value: d.id,
                                  child: Text(
                                    d.name,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                            ],
                            onChanged: (v) {
                              setState(() => _deviceFilter = v);
                              setSheet(() {});
                            },
                          ),
                          const SizedBox(height: 12),
                          ResponsiveFieldRow(
                            breakpoint: 480,
                            children: <Widget>[
                              AppDropdownFormField<String?>(
                                initialValue: _statusFilter,
                                decoration: const InputDecoration(
                                  labelText: 'Status',
                                ),
                                items: const <DropdownMenuItem<String?>>[
                                  DropdownMenuItem<String?>(
                                    value: null,
                                    child: Text(
                                      'Todos',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'succeeded',
                                    child: Text(
                                      'Concluído',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'running',
                                    child: Text(
                                      'Executando',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'failed',
                                    child: Text(
                                      'Falhou',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'cancelled',
                                    child: Text(
                                      'Cancelado',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                ],
                                onChanged: (v) {
                                  setState(() => _statusFilter = v);
                                  setSheet(() {});
                                },
                              ),
                              AppDropdownFormField<String?>(
                                initialValue: _kindFilter,
                                decoration: const InputDecoration(
                                  labelText: 'Modo',
                                ),
                                items: const <DropdownMenuItem<String?>>[
                                  DropdownMenuItem<String?>(
                                    value: null,
                                    child: Text(
                                      'Todos',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'link',
                                    child: Text(
                                      'Enlace',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'path',
                                    child: Text(
                                      'Caminho',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'server',
                                    child: Text(
                                      'Servidor',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  DropdownMenuItem<String?>(
                                    value: 'end_to_end',
                                    child: Text(
                                      'Ponta a ponta',
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                ],
                                onChanged: (v) {
                                  setState(() => _kindFilter = v);
                                  setSheet(() {});
                                },
                              ),
                            ],
                          ),
                        ],
                      ),
                      onApplyFilters: () => ref
                          .read(linkTestsProvider.notifier)
                          .fetchPage(
                            page: 1,
                            deviceId: _deviceFilter,
                            status: _statusFilter,
                            kind: _kindFilter,
                          ),
                      onClearFilters: () {
                        setState(() {
                          _deviceFilter = null;
                          _statusFilter = null;
                          _kindFilter = null;
                        });
                        ref.read(linkTestsProvider.notifier).fetchPage(page: 1);
                      },
                      activeChips: activeChips,
                    ),
                    const SizedBox(height: 12),
                    if (state.pageLoading && state.pageItems.isEmpty)
                      const LoadingStateView(
                        message: 'Carregando histórico de testes...',
                      )
                    else if (state.pageItems.isEmpty)
                      EmptyStateView(
                        title: 'Nenhum teste de velocidade registrado',
                        description:
                            'Clique em "Novo Teste" para medir a capacidade real de um enlace.',
                        icon: Icons.speed,
                        action: canWrite
                            ? FilledButton.icon(
                                onPressed: () => _openWizard(),
                                icon: const Icon(Icons.speed),
                                label: const Text('Novo Teste'),
                              )
                            : null,
                      )
                    else ...<Widget>[
                      for (final test in state.pageItems)
                        Card(
                          elevation: 2,
                          margin: const EdgeInsets.only(bottom: 8),
                          child: InkWell(
                            borderRadius: AppTheme.borderRadius,
                            onTap: () async {
                              await ref
                                  .read(linkTestsProvider.notifier)
                                  .fetchDetail(test.id);
                              ref
                                  .read(linkTestsProvider.notifier)
                                  .focus(test.id);
                            },
                            child: Padding(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 12,
                                vertical: 10,
                              ),
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.stretch,
                                children: <Widget>[
                                  Row(
                                    children: <Widget>[
                                      _buildLinkTestStatus(test.status),
                                      const SizedBox(width: 10),
                                      Expanded(
                                        child: Column(
                                          crossAxisAlignment:
                                              CrossAxisAlignment.start,
                                          children: <Widget>[
                                            Text(
                                              test.title,
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: context
                                                  .textTheme
                                                  .titleSmall
                                                  ?.copyWith(
                                                    fontWeight: FontWeight.w700,
                                                  ),
                                            ),
                                            const SizedBox(height: 2),
                                            Text(
                                              '${_linkTestKindLabel(test.kind)} · ${countNoun(test.legs.length, 'perna', 'pernas')} · ${formatShortDateTime(test.createdAt)}',
                                              maxLines: 1,
                                              overflow: TextOverflow.ellipsis,
                                              style: context.textTheme.bodySmall
                                                  ?.copyWith(
                                                    color: context
                                                        .colorScheme
                                                        .onSurfaceVariant,
                                                  ),
                                            ),
                                          ],
                                        ),
                                      ),
                                      IconButton(
                                        visualDensity: VisualDensity.compact,
                                        tooltip: 'Comparar resultado',
                                        icon: Icon(
                                          (_compareA?.id == test.id ||
                                                  _compareB?.id == test.id)
                                              ? Icons.compare
                                              : Icons.compare_arrows,
                                          size: 18,
                                        ),
                                        onPressed: () {
                                          setState(() {
                                            if (_compareA == null ||
                                                _compareA!.id == test.id) {
                                              _compareA = test;
                                            } else {
                                              _compareB = test;
                                            }
                                          });
                                        },
                                      ),
                                    ],
                                  ),
                                  if (test.summary?.minForwardBps != null ||
                                      test.summary?.minReverseBps !=
                                          null) ...<Widget>[
                                    const SizedBox(height: 8),
                                    Wrap(
                                      spacing: 6,
                                      runSpacing: 6,
                                      children: <Widget>[
                                        if (test.summary?.minForwardBps != null)
                                          SemanticChip(
                                            label: formatBps(
                                              test.summary!.minForwardBps,
                                            ),
                                            icon: Icons.arrow_downward_rounded,
                                            tone: SemanticTone.success,
                                            compact: true,
                                          ),
                                        if (test.summary?.minReverseBps != null)
                                          SemanticChip(
                                            label: formatBps(
                                              test.summary!.minReverseBps,
                                            ),
                                            icon: Icons.arrow_upward_rounded,
                                            tone: SemanticTone.info,
                                            compact: true,
                                          ),
                                      ],
                                    ),
                                  ],
                                ],
                              ),
                            ),
                          ),
                        ),
                      if (state.total > linkTestHistoryPerPage)
                        Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: <Widget>[
                            IconButton(
                              onPressed: state.page > 1
                                  ? () => ref
                                        .read(linkTestsProvider.notifier)
                                        .fetchPage(
                                          page: state.page - 1,
                                          deviceId: _deviceFilter,
                                          status: _statusFilter,
                                          kind: _kindFilter,
                                        )
                                  : null,
                              icon: const Icon(Icons.chevron_left),
                            ),
                            Text('Página ${state.page}'),
                            IconButton(
                              onPressed:
                                  state.page * linkTestHistoryPerPage <
                                      state.total
                                  ? () => ref
                                        .read(linkTestsProvider.notifier)
                                        .fetchPage(
                                          page: state.page + 1,
                                          deviceId: _deviceFilter,
                                          status: _statusFilter,
                                          kind: _kindFilter,
                                        )
                                  : null,
                              icon: const Icon(Icons.chevron_right),
                            ),
                          ],
                        ),
                    ],
                  ],
                ),
              ),
              // ── Tab 2: Agendamentos ──
              state.schedulesLoading && state.schedules.isEmpty
                  ? const LoadingStateView(
                      message: 'Carregando agendamentos...',
                    )
                  : state.schedules.isEmpty
                  ? ListView(
                      children: <Widget>[
                        const SizedBox(height: 24),
                        EmptyStateView(
                          title: 'Nenhum agendamento configurado',
                          description:
                              'Programe medições recorrentes em janelas de baixa utilização.',
                          icon: Icons.schedule,
                          action: canWrite
                              ? FilledButton.icon(
                                  onPressed: () => _openScheduleSheet(),
                                  icon: const Icon(Icons.add),
                                  label: const Text('Criar agendamento'),
                                )
                              : null,
                        ),
                      ],
                    )
                  : ListView.builder(
                      padding: const EdgeInsets.only(top: 10, bottom: 20),
                      itemCount: state.schedules.length,
                      itemBuilder: (context, idx) {
                        final sched = state.schedules[idx];
                        return Card(
                          elevation: 2,
                          margin: const EdgeInsets.only(bottom: 8),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 12,
                              vertical: 10,
                            ),
                            child: Row(
                              crossAxisAlignment: CrossAxisAlignment.center,
                              children: <Widget>[
                                Icon(
                                  Icons.event_repeat,
                                  color: sched.enabled
                                      ? context.semanticColors.success
                                      : context.colorScheme.onSurfaceVariant,
                                ),
                                const SizedBox(width: 10),
                                Expanded(
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: <Widget>[
                                      Text(
                                        sched.name,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: context.textTheme.titleSmall
                                            ?.copyWith(
                                              fontWeight: FontWeight.w700,
                                            ),
                                      ),
                                      const SizedBox(height: 2),
                                      Text(
                                        'A cada ${sched.intervalMinutes} min · ${_linkTestKindLabel(sched.kind)}'
                                        '${sched.windowStart != null ? ' · Janela ${sched.windowStart}–${sched.windowEnd ?? ''}' : ''}'
                                        '${sched.nextRunAt != null ? ' · Próxima: ${formatTimeUntil(sched.nextRunAt)}' : ''}',
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis,
                                        style: context.textTheme.bodySmall
                                            ?.copyWith(
                                              color: context
                                                  .colorScheme
                                                  .onSurfaceVariant,
                                            ),
                                      ),
                                    ],
                                  ),
                                ),
                                if (canWrite)
                                  if (isCompact)
                                    PopupMenuButton<String>(
                                      tooltip: 'Ações do agendamento',
                                      icon: const Icon(Icons.more_vert),
                                      onSelected: (action) async {
                                        if (action == 'run') {
                                          await ref
                                              .read(linkTestsProvider.notifier)
                                              .runScheduleNow(sched);
                                        } else if (action == 'edit') {
                                          await _openScheduleSheet(
                                            existing: sched,
                                          );
                                        } else if (action == 'delete') {
                                          final ok =
                                              await showConfirmActionDialog(
                                                context: context,
                                                title: 'Excluir agendamento',
                                                message:
                                                    'Remover "${sched.name}"?',
                                                confirmLabel: 'Excluir',
                                              );
                                          if (ok) {
                                            await ref
                                                .read(
                                                  linkTestsProvider.notifier,
                                                )
                                                .deleteSchedule(sched.id);
                                          }
                                        }
                                      },
                                      itemBuilder: (_) =>
                                          <PopupMenuEntry<String>>[
                                            const PopupMenuItem<String>(
                                              value: 'run',
                                              child: ListTile(
                                                dense: true,
                                                contentPadding: EdgeInsets.zero,
                                                leading: Icon(Icons.play_arrow),
                                                title: Text('Executar agora'),
                                              ),
                                            ),
                                            const PopupMenuItem<String>(
                                              value: 'edit',
                                              child: ListTile(
                                                dense: true,
                                                contentPadding: EdgeInsets.zero,
                                                leading: Icon(
                                                  Icons.edit_outlined,
                                                ),
                                                title: Text('Editar'),
                                              ),
                                            ),
                                            PopupMenuItem<String>(
                                              value: 'delete',
                                              child: ListTile(
                                                dense: true,
                                                contentPadding: EdgeInsets.zero,
                                                leading: Icon(
                                                  Icons.delete_outline,
                                                  color:
                                                      context.colorScheme.error,
                                                ),
                                                title: Text(
                                                  'Excluir',
                                                  style: TextStyle(
                                                    color: context
                                                        .colorScheme
                                                        .error,
                                                  ),
                                                ),
                                              ),
                                            ),
                                          ],
                                    )
                                  else
                                    Row(
                                      mainAxisSize: MainAxisSize.min,
                                      children: <Widget>[
                                        IconButton(
                                          tooltip: 'Executar agora',
                                          icon: const Icon(Icons.play_arrow),
                                          onPressed: () => ref
                                              .read(linkTestsProvider.notifier)
                                              .runScheduleNow(sched),
                                        ),
                                        IconButton(
                                          tooltip: 'Editar',
                                          icon: const Icon(
                                            Icons.edit_outlined,
                                            size: 18,
                                          ),
                                          onPressed: () => _openScheduleSheet(
                                            existing: sched,
                                          ),
                                        ),
                                        IconButton(
                                          tooltip: 'Excluir',
                                          icon: Icon(
                                            Icons.delete_outline,
                                            size: 18,
                                            color: context.colorScheme.error,
                                          ),
                                          onPressed: () async {
                                            final ok =
                                                await showConfirmActionDialog(
                                                  context: context,
                                                  title: 'Excluir agendamento',
                                                  message:
                                                      'Remover "${sched.name}"?',
                                                  confirmLabel: 'Excluir',
                                                );
                                            if (ok) {
                                              await ref
                                                  .read(
                                                    linkTestsProvider.notifier,
                                                  )
                                                  .deleteSchedule(sched.id);
                                            }
                                          },
                                        ),
                                      ],
                                    ),
                              ],
                            ),
                          ),
                        );
                      },
                    ),
            ],
          ),
        );
      },
    );
  }
}

class _LinkTestsTabBarDelegate extends SliverPersistentHeaderDelegate {
  const _LinkTestsTabBarDelegate({
    required this.tabBar,
    required this.backgroundColor,
  });

  final TabBar tabBar;
  final Color backgroundColor;

  @override
  double get minExtent => tabBar.preferredSize.height + 1;

  @override
  double get maxExtent => tabBar.preferredSize.height + 1;

  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) {
    return ColoredBox(
      color: backgroundColor,
      child: Column(children: <Widget>[tabBar, const Divider(height: 1)]),
    );
  }

  @override
  bool shouldRebuild(covariant _LinkTestsTabBarDelegate oldDelegate) {
    return oldDelegate.tabBar != tabBar ||
        oldDelegate.backgroundColor != backgroundColor;
  }
}

class _FocusedLinkTestCard extends StatelessWidget {
  const _FocusedLinkTestCard({
    required this.test,
    required this.state,
    required this.canWrite,
    required this.onClose,
    required this.onCancel,
  });

  final LinkTestModel test;
  final LinkTestsState state;
  final bool canWrite;
  final VoidCallback onClose;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final firstLeg = test.legs.firstOrNull;
    final fwdSamples = firstLeg != null
        ? state.samplesFor(test.id, firstLeg.seq, 'forward')
        : const <LinkTestSampleModel>[];
    final revSamples = firstLeg != null
        ? state.samplesFor(test.id, firstLeg.seq, 'reverse')
        : const <LinkTestSampleModel>[];
    final healthSamples = firstLeg != null
        ? state.healthFor(test.id, firstLeg.seq)
        : const <LinkTestHealthSampleModel>[];

    final fwdPts = fwdSamples
        .map((s) => MetricSeriesPoint(x: s.tMs / 1000.0, y: s.bps / 1e6))
        .toList(growable: false);
    final revPts = revSamples
        .map((s) => MetricSeriesPoint(x: s.tMs / 1000.0, y: s.bps / 1e6))
        .toList(growable: false);
    final cpuPts = healthSamples
        .where((h) => h.cpu != null)
        .map((h) => MetricSeriesPoint(x: h.atMs / 1000.0, y: h.cpu!))
        .toList(growable: false);

    return SectionCard(
      title: 'Teste #${test.id} · ${test.title}',
      subtitle:
          '${_linkTestKindLabel(test.kind)} · ${_linkTestDirectionLabel(test.spec.direction)} · ${test.spec.durationS}s (${countNoun(test.spec.streams, 'fluxo', 'fluxos')})',
      icon: Icons.speed,
      iconTone: test.isActive ? SemanticTone.info : SemanticTone.success,
      trailing: IconButton(
        tooltip: 'Fechar painel',
        icon: const Icon(Icons.close),
        onPressed: onClose,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              _buildLinkTestStatus(test.status),
              if (test.isActive && canWrite)
                OutlinedButton.icon(
                  onPressed: onCancel,
                  icon: const Icon(Icons.stop, size: 16),
                  label: const Text('Cancelar'),
                ),
            ],
          ),
          const SizedBox(height: 10),
          // Path Map visual das pernas
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: test.legs
                  .map((leg) {
                    final isBottleneck =
                        test.summary?.bottleneckLegSeq == leg.seq;
                    return Padding(
                      padding: const EdgeInsets.only(right: 10),
                      child: NestedPanel(
                        tone: isBottleneck
                            ? SemanticTone.warning
                            : (leg.status == 'succeeded' ||
                                  leg.status == 'completed')
                            ? SemanticTone.success
                            : SemanticTone.info,
                        padding: const EdgeInsets.all(10),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Row(
                              mainAxisSize: MainAxisSize.min,
                              children: <Widget>[
                                Text(
                                  leg.a.name,
                                  style: context.textTheme.labelLarge?.copyWith(
                                    fontWeight: FontWeight.w800,
                                  ),
                                ),
                                const Padding(
                                  padding: EdgeInsets.symmetric(horizontal: 6),
                                  child: Icon(
                                    Icons.arrow_forward_rounded,
                                    size: 15,
                                  ),
                                ),
                                Text(
                                  leg.b.name,
                                  style: context.textTheme.labelLarge?.copyWith(
                                    fontWeight: FontWeight.w800,
                                  ),
                                ),
                              ],
                            ),
                            const SizedBox(height: 4),
                            Text(
                              'Ida: ${formatBps(leg.forwardBps)} · Volta: ${formatBps(leg.reverseBps)}'
                              '${leg.engine != null ? ' · ${leg.engine}' : ''}',
                              style: context.textTheme.bodySmall,
                            ),
                            if (isBottleneck)
                              const Padding(
                                padding: EdgeInsets.only(top: 4),
                                child: SemanticChip(
                                  label: 'Gargalo do caminho',
                                  tone: SemanticTone.warning,
                                  compact: true,
                                ),
                              ),
                          ],
                        ),
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
          ),
          if (fwdPts.isNotEmpty || revPts.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            MetricLineChart(
              height: 180,
              yFormatter: (v) => '${v.toStringAsFixed(0)} Mbps',
              xFormatter: (v) => '${v.round()}s',
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'Ida (Mbps)',
                  points: fwdPts,
                  tone: SemanticTone.success,
                ),
                MetricLineSeries(
                  name: 'Volta (Mbps)',
                  points: revPts,
                  tone: SemanticTone.info,
                ),
              ],
            ),
          ],
          if (cpuPts.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            Text(
              'Saúde das pontas durante a medição (CPU %)',
              style: context.textTheme.labelMedium,
            ),
            const SizedBox(height: 4),
            MetricLineChart(
              height: 110,
              minY: 0,
              maxY: 100,
              yFormatter: (v) => '${v.round()}%',
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'CPU %',
                  points: cpuPts,
                  tone: SemanticTone.warning,
                ),
              ],
            ),
          ],
          if (test.error != null) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(tone: SemanticTone.error, child: Text(test.error!)),
          ],
        ],
      ),
    );
  }
}

class _LinkTestComparisonCard extends StatelessWidget {
  const _LinkTestComparisonCard({
    required this.a,
    required this.b,
    required this.onClear,
  });

  final LinkTestModel a;
  final LinkTestModel b;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    return NestedPanel(
      tone: SemanticTone.primary,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  'Comparação: #${a.id} vs #${b.id}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              IconButton(
                visualDensity: VisualDensity.compact,
                onPressed: onClear,
                icon: const Icon(Icons.close, size: 18),
              ),
            ],
          ),
          KeyValueRow(
            label: 'Ida mínima',
            value:
                '${formatBps(a.summary?.minForwardBps)}  vs  ${formatBps(b.summary?.minForwardBps)}',
          ),
          KeyValueRow(
            label: 'Volta mínima',
            value:
                '${formatBps(a.summary?.minReverseBps)}  vs  ${formatBps(b.summary?.minReverseBps)}',
          ),
        ],
      ),
    );
  }
}

class _LinkTestWizardBody extends ConsumerStatefulWidget {
  const _LinkTestWizardBody({this.initialDraft, required this.onStarted});

  final LinkTestRequestDraft? initialDraft;
  final ValueChanged<int> onStarted;

  @override
  ConsumerState<_LinkTestWizardBody> createState() =>
      _LinkTestWizardBodyState();
}

class _LinkTestWizardBodyState extends ConsumerState<_LinkTestWizardBody> {
  late String _kind;
  int? _linkId;
  int? _sourceDeviceId;
  int? _targetDeviceId;
  String _direction = 'both';
  int _durationS = 10;
  int _streams = 4;
  final TextEditingController _limitCtrl = TextEditingController();

  bool _planning = false;
  bool _starting = false;
  LinkTestPlanModel? _plan;
  String? _error;

  @override
  void initState() {
    super.initState();
    _kind = widget.initialDraft?.kind ?? 'server';
    _linkId = widget.initialDraft?.linkId;
    _sourceDeviceId = widget.initialDraft?.sourceDeviceId;
    _targetDeviceId = widget.initialDraft?.targetDeviceId;
  }

  @override
  void dispose() {
    _limitCtrl.dispose();
    super.dispose();
  }

  Map<String, dynamic> _buildRequest() => <String, dynamic>{
    'kind': _kind,
    if (_kind == 'link' && _linkId != null) 'linkId': _linkId,
    if (_kind != 'link' && _kind != 'server' && _sourceDeviceId != null)
      'sourceDeviceId': _sourceDeviceId,
    if (_kind != 'link' && _targetDeviceId != null)
      'targetDeviceId': _targetDeviceId,
    'spec': <String, dynamic>{
      'durationS': _durationS,
      'direction': _direction,
      'streams': _streams,
      if (double.tryParse(_limitCtrl.text.trim()) != null)
        'limitMbps': double.parse(_limitCtrl.text.trim()),
    },
  };

  Future<void> _runPlan() async {
    setState(() {
      _planning = true;
      _error = null;
    });
    try {
      final plan = await ref
          .read(linkTestsProvider.notifier)
          .plan(_buildRequest());
      if (mounted) setState(() => _plan = plan);
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _planning = false);
    }
  }

  Future<void> _startTest() async {
    setState(() {
      _starting = true;
      _error = null;
    });
    try {
      final id = await ref
          .read(linkTestsProvider.notifier)
          .start(_buildRequest());
      if (mounted) widget.onStarted(id);
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _starting = false);
    }
  }

  Future<void> _resolveFix(LinkTestFixModel fix) async {
    if (fix.kind == 'add_credential') {
      final saved = await ref
          .read(deviceAccessProvider.notifier)
          .requestAccess(
            deviceId: fix.deviceId,
            transport: fix.transport ?? 'ssh',
            deviceName: fix.deviceName,
            message: fix.message,
          );
      if (saved && mounted) await _runPlan();
    } else if (fix.kind == 'install_plugin' && fix.pluginId != null) {
      await withDeviceAccess(
        context: context,
        ref: ref,
        action: () async {
          await ref.read(devicesProvider.notifier).fetchDevices();
        },
      );
      await _runPlan();
    }
  }

  @override
  Widget build(BuildContext context) {
    final devices = ref.watch(devicesProvider).items;
    final edges = ref
        .watch(topologyProvider)
        .edges
        .where((TopologyEdgeModel e) => !e.isVirtualHierarchy)
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        LayoutBuilder(
          builder: (context, constraints) {
            final isNarrow = constraints.maxWidth < 460;
            return SegmentedButton<String>(
              showSelectedIcon: false,
              style: SegmentedButton.styleFrom(
                visualDensity: isNarrow
                    ? VisualDensity.compact
                    : VisualDensity.standard,
                padding: EdgeInsets.symmetric(
                  horizontal: isNarrow ? 6 : 12,
                  vertical: 8,
                ),
              ),
              segments: const <ButtonSegment<String>>[
                ButtonSegment(
                  value: 'server',
                  label: Text(
                    'Servidor',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment(
                  value: 'link',
                  label: Text(
                    'Enlace',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment(
                  value: 'end_to_end',
                  label: Text(
                    'A -> B',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment(
                  value: 'path',
                  label: Text(
                    'Caminho',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
              selected: <String>{_kind},
              onSelectionChanged: (s) => setState(() {
                _kind = s.first;
                _plan = null;
              }),
            );
          },
        ),
        const SizedBox(height: 12),
        if (_kind == 'link')
          AppDropdownFormField<int?>(
            initialValue: edges.any((e) => e.id == _linkId) ? _linkId : null,
            decoration: const InputDecoration(labelText: 'Enlace da topologia'),
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
            onChanged: (v) => setState(() => _linkId = v),
          )
        else ...<Widget>[
          if (_kind != 'server') ...<Widget>[
            AppDropdownFormField<int?>(
              initialValue: devices.any((d) => d.id == _sourceDeviceId)
                  ? _sourceDeviceId
                  : null,
              decoration: const InputDecoration(
                labelText: 'Equipamento origem',
              ),
              items: devices
                  .map(
                    (DeviceModel d) => DropdownMenuItem<int?>(
                      value: d.id,
                      child: Text(d.name, overflow: TextOverflow.ellipsis),
                    ),
                  )
                  .toList(growable: false),
              onChanged: (v) => setState(() => _sourceDeviceId = v),
            ),
            const SizedBox(height: 10),
          ],
          AppDropdownFormField<int?>(
            initialValue: devices.any((d) => d.id == _targetDeviceId)
                ? _targetDeviceId
                : null,
            decoration: const InputDecoration(labelText: 'Equipamento destino'),
            items: devices
                .map(
                  (DeviceModel d) => DropdownMenuItem<int?>(
                    value: d.id,
                    child: Text(d.name, overflow: TextOverflow.ellipsis),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) => setState(() => _targetDeviceId = v),
          ),
        ],
        const SizedBox(height: 12),
        LayoutBuilder(
          builder: (context, constraints) {
            final directionField = AppDropdownFormField<String>(
              initialValue: _direction,
              decoration: const InputDecoration(labelText: 'Sentido'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'both',
                  child: Text('Ida e Volta', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'forward',
                  child: Text('Somente Ida', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'reverse',
                  child: Text('Somente Volta', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) => setState(() => _direction = v ?? 'both'),
            );
            final durationField = AppDropdownFormField<int>(
              initialValue: _durationS,
              decoration: const InputDecoration(labelText: 'Duração'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(
                  value: 5,
                  child: Text('5 s', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 10,
                  child: Text('10 s', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 20,
                  child: Text('20 s', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 30,
                  child: Text('30 s', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) => setState(() => _durationS = v ?? 10),
            );
            final streamsField = AppDropdownFormField<int>(
              initialValue: _streams,
              decoration: const InputDecoration(labelText: 'Streams'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(
                  value: 1,
                  child: Text('1', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 4,
                  child: Text('4', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 8,
                  child: Text('8', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) => setState(() => _streams = v ?? 4),
            );

            if (constraints.maxWidth < 620) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  directionField,
                  const SizedBox(height: 12),
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Expanded(child: durationField),
                      const SizedBox(width: 10),
                      Expanded(child: streamsField),
                    ],
                  ),
                ],
              );
            }

            return ResponsiveFieldRow(
              breakpoint: 620,
              children: <Widget>[directionField, durationField, streamsField],
            );
          },
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _limitCtrl,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(
            labelText: 'Limite de banda opcional (Mbps)',
            hintText: 'Vazio = sem limite',
          ),
        ),
        const SizedBox(height: 12),
        SymmetricalActionBar(
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: _planning ? null : _runPlan,
              icon: _planning
                  ? const SizedBox(
                      width: 15,
                      height: 15,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.fact_check_outlined),
              label: const Text('Planejar (Dry-Run)'),
            ),
            if (_plan?.isReady == true)
              FilledButton.icon(
                onPressed: _starting ? null : _startTest,
                icon: const Icon(Icons.play_arrow),
                label: Text(
                  _starting ? 'Iniciando...' : 'Confirmar e Executar',
                ),
              ),
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (_plan != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: _plan!.isReady ? SemanticTone.success : SemanticTone.warning,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(
                  'Plano: ${countNoun(_plan!.legs.length, 'perna', 'pernas')} · ~${_plan!.estimatedSeconds}s estimados',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                for (final w in _plan!.warnings)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Icon(
                          Icons.warning_amber_rounded,
                          size: 16,
                          color: context.semanticColors.warning,
                        ),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(w, style: context.textTheme.bodySmall),
                        ),
                      ],
                    ),
                  ),
                for (final b in _plan!.blocking)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Icon(
                          Icons.error_outline,
                          size: 16,
                          color: context.colorScheme.error,
                        ),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            b,
                            style: TextStyle(color: context.colorScheme.error),
                          ),
                        ),
                      ],
                    ),
                  ),
                if (_plan!.fixes.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  for (final fix in _plan!.fixes)
                    Padding(
                      padding: const EdgeInsets.only(top: 6),
                      child: Row(
                        children: <Widget>[
                          Expanded(
                            child: Text(
                              fix.message,
                              style: context.textTheme.bodySmall,
                            ),
                          ),
                          const SizedBox(width: 8),
                          FilledButton.tonal(
                            onPressed: () => _resolveFix(fix),
                            child: Text(
                              fix.kind == 'add_credential'
                                  ? 'Cadastrar acesso'
                                  : 'Resolver',
                            ),
                          ),
                        ],
                      ),
                    ),
                ],
              ],
            ),
          ),
        ],
      ],
    );
  }
}

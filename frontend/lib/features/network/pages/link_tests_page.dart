import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/widgets/link_tests/link_test_schedule_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/link_tests/link_test_telemetry_cards.dart';
import 'package:netmonitor_app/features/network/widgets/link_tests/link_test_wizard_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/network/widgets/link_tests/link_test_schedule_sheet.dart';
export 'package:netmonitor_app/features/network/widgets/link_tests/link_test_telemetry_cards.dart';
export 'package:netmonitor_app/features/network/widgets/link_tests/link_test_wizard_sheet.dart';

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
      if (!mounted) return;
      ref.read(linkTestsProvider.notifier).focus(widget.initialTestId);
    }
    final draft = ref.read(pendingLinkTestDraftProvider);
    if (draft != null) {
      ref.read(pendingLinkTestDraftProvider.notifier).state = null;
      if (ref.read(authProvider).canWriteIn(ProductArea.network)) {
        _openWizard(draft: draft);
      }
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
          'Configure a rota, prepare os equipamentos e acompanhe a medição em tempo real',
      maxWidth: 680,
      builder: (ctx) => LinkTestWizardBody(
        initialDraft: draft,
        onStarted: (id) {
          Navigator.of(ctx).pop();
          ref.read(linkTestsProvider.notifier).focus(id);
        },
      ),
    );
  }

  Future<void> _cancelFocusedTest(int testId) async {
    try {
      await ref.read(linkTestsProvider.notifier).cancel(testId);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Solicitação de cancelamento enviada.')),
        );
      }
    } catch (err) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              'Falha ao cancelar teste: ${formatLinkTestApiError(err)}',
            ),
          ),
        );
      }
    }
  }

  Future<void> _runScheduleNow(LinkTestScheduleModel sched) async {
    try {
      final id = await ref
          .read(linkTestsProvider.notifier)
          .runScheduleNow(sched);
      if (!mounted) return;
      if (id != null && id > 0) {
        _tabController.animateTo(0);
      }
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Execução iniciada para "${sched.name}".')),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Falha ao executar "${sched.name}": ${formatLinkTestApiError(err)}',
          ),
        ),
      );
    }
  }

  Future<void> _confirmDeleteSchedule(LinkTestScheduleModel sched) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Excluir agendamento',
      message: 'Remover "${sched.name}"?',
      confirmLabel: 'Excluir',
    );
    if (!ok || !mounted) return;
    final deleted = await ref
        .read(linkTestsProvider.notifier)
        .deleteSchedule(sched.id);
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          deleted
              ? 'Agendamento "${sched.name}" removido.'
              : (ref.read(linkTestsProvider).schedulesError ??
                    'Não foi possível excluir o agendamento.'),
        ),
      ),
    );
  }

  Future<void> _openScheduleSheet({LinkTestScheduleModel? existing}) {
    return showLinkTestScheduleSheet(context, existing: existing);
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
          label: 'Status: ${linkTestStatusLabel(_statusFilter!)}',
          onDeleted: () {
            setState(() => _statusFilter = null);
            ref
                .read(linkTestsProvider.notifier)
                .fetchPage(page: 1, deviceId: _deviceFilter, kind: _kindFilter);
          },
        ),
      if (_kindFilter != null)
        ActiveFilterChipData(
          label: 'Modo: ${linkTestKindLabel(_kindFilter!)}',
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
        final tabBar = TabBar(
          controller: _tabController,
          isScrollable: isCompact,
          tabAlignment: isCompact ? TabAlignment.start : TabAlignment.fill,
          labelPadding: const EdgeInsets.symmetric(horizontal: 14),
          tabs: <Widget>[
            Tab(text: 'Histórico (${state.total})'),
            Tab(text: 'Agendamentos (${state.schedules.length})'),
          ],
        );

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
                    FocusedLinkTestCard(
                      test: focused,
                      state: state,
                      canWrite: canWrite,
                      onClose: () =>
                          ref.read(linkTestsProvider.notifier).focus(null),
                      onCancel: () => _cancelFocusedTest(focused.id),
                    ),
                  ],
                  if (_compareA != null && _compareB != null) ...<Widget>[
                    const SizedBox(height: 10),
                    LinkTestComparisonCard(
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
              delegate: PinnedSliverHeaderDelegate(
                height: tabBar.preferredSize.height + 1,
                backgroundColor: Theme.of(context).scaffoldBackgroundColor,
                bottomBorder: true,
                child: tabBar,
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
                                      buildLinkTestStatus(test.status),
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
                                              '${linkTestKindLabel(test.kind)} · ${countNoun(test.legs.length, 'trecho', 'trechos')} · ${formatShortDateTime(test.createdAt)}',
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
                                        'A cada ${sched.intervalMinutes} min · ${linkTestKindLabel(sched.kind)}'
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
                                          await _runScheduleNow(sched);
                                        } else if (action == 'edit') {
                                          await _openScheduleSheet(
                                            existing: sched,
                                          );
                                        } else if (action == 'delete') {
                                          await _confirmDeleteSchedule(sched);
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
                                          onPressed: () =>
                                              _runScheduleNow(sched),
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
                                          onPressed: () =>
                                              _confirmDeleteSchedule(sched),
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

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
      subtitle: 'Planeje pernas, resolva credenciais e acompanhe a medição em tempo real',
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
            DropdownButtonFormField<String>(
              initialValue: kind,
              decoration: const InputDecoration(labelText: 'Modo do teste'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'server',
                  child: Text('Central → Equipamento'),
                ),
                DropdownMenuItem(
                  value: 'link',
                  child: Text('Enlace da Topologia'),
                ),
                DropdownMenuItem(
                  value: 'end_to_end',
                  child: Text('Ponta a ponta (A → B)'),
                ),
                DropdownMenuItem(
                  value: 'path',
                  child: Text('Caminho completo (salto a salto)'),
                ),
              ],
              onChanged: (v) => setModal(() => kind = v ?? 'server'),
            ),
            const SizedBox(height: 10),
            if (kind == 'link')
              DropdownButtonFormField<int?>(
                initialValue: edges.any((e) => e.id == linkId) ? linkId : null,
                decoration: const InputDecoration(labelText: 'Enlace'),
                items: edges
                    .where((e) => !e.isVirtualHierarchy)
                    .map(
                      (e) => DropdownMenuItem<int?>(
                        value: e.id,
                        child: Text(
                          '${e.sourceDeviceName ?? '#${e.sourceDeviceId}'} ↔ ${e.targetDeviceName ?? '#${e.targetDeviceId}'}',
                        ),
                      ),
                    )
                    .toList(growable: false),
                onChanged: (v) => setModal(() => linkId = v),
              )
            else ...<Widget>[
              if (kind != 'server') ...<Widget>[
                DropdownButtonFormField<int?>(
                  initialValue: devices.any((d) => d.id == sourceId)
                      ? sourceId
                      : null,
                  decoration: const InputDecoration(labelText: 'Origem'),
                  items: devices
                      .map(
                        (d) => DropdownMenuItem<int?>(
                          value: d.id,
                          child: Text(d.name),
                        ),
                      )
                      .toList(growable: false),
                  onChanged: (v) => setModal(() => sourceId = v),
                ),
                const SizedBox(height: 10),
              ],
              DropdownButtonFormField<int?>(
                initialValue: devices.any((d) => d.id == targetId)
                    ? targetId
                    : null,
                decoration: const InputDecoration(labelText: 'Destino'),
                items: devices
                    .map(
                      (d) => DropdownMenuItem<int?>(
                        value: d.id,
                        child: Text(d.name),
                      ),
                    )
                    .toList(growable: false),
                onChanged: (v) => setModal(() => targetId = v),
              ),
            ],
            const SizedBox(height: 10),
            Row(
              children: <Widget>[
                Expanded(
                  child: TextField(
                    controller: intervalCtrl,
                    keyboardType: TextInputType.number,
                    decoration: const InputDecoration(
                      labelText: 'Intervalo (min >= 15)',
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    controller: durationCtrl,
                    keyboardType: TextInputType.number,
                    decoration: const InputDecoration(labelText: 'Duração (s)'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: DropdownButtonFormField<String>(
                    initialValue: direction,
                    decoration: const InputDecoration(labelText: 'Sentido'),
                    items: const <DropdownMenuItem<String>>[
                      DropdownMenuItem(value: 'both', child: Text('Ambos')),
                      DropdownMenuItem(value: 'forward', child: Text('Ida')),
                      DropdownMenuItem(value: 'reverse', child: Text('Volta')),
                    ],
                    onChanged: (v) => setModal(() => direction = v ?? 'both'),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 10),
            Row(
              children: <Widget>[
                Expanded(
                  child: TextField(
                    controller: windowStartCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Janela início (HH:MM)',
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    controller: windowEndCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Janela fim (HH:MM)',
                    ),
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
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
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
                label: Text(existing != null ? 'Salvar' : 'Criar agendamento'),
              ),
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          title: 'Testes de Velocidade e Enlaces',
          subtitle: 'Medição ativa entre equipamentos via iPerf3/BTest com telemetria ao vivo via SSE',
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
          const SizedBox(height: 12),
          _FocusedLinkTestCard(
            test: focused,
            state: state,
            canWrite: canWrite,
            onClose: () => ref.read(linkTestsProvider.notifier).focus(null),
            onCancel: () =>
                ref.read(linkTestsProvider.notifier).cancel(focused.id),
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
        const SizedBox(height: 10),
        TabBar(
          controller: _tabController,
          tabs: <Widget>[
            Tab(text: 'Histórico (${state.total})'),
            Tab(text: 'Agendamentos (${state.schedules.length})'),
          ],
        ),
        const SizedBox(height: 10),
        Expanded(
          child: TabBarView(
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
                  children: <Widget>[
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: <Widget>[
                        ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 200),
                          child: DropdownButtonFormField<int?>(
                            initialValue:
                                devices.any((d) => d.id == _deviceFilter)
                                ? _deviceFilter
                                : null,
                            isDense: true,
                            decoration: const InputDecoration(
                              labelText: 'Equipamento',
                            ),
                            items: <DropdownMenuItem<int?>>[
                              const DropdownMenuItem<int?>(
                                value: null,
                                child: Text('Todos'),
                              ),
                              for (final d in devices)
                                DropdownMenuItem<int?>(
                                  value: d.id,
                                  child: Text(d.name),
                                ),
                            ],
                            onChanged: (v) {
                              setState(() => _deviceFilter = v);
                              ref
                                  .read(linkTestsProvider.notifier)
                                  .fetchPage(
                                    page: 1,
                                    deviceId: v,
                                    status: _statusFilter,
                                    kind: _kindFilter,
                                  );
                            },
                          ),
                        ),
                        ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 160),
                          child: DropdownButtonFormField<String?>(
                            initialValue: _statusFilter,
                            isDense: true,
                            decoration: const InputDecoration(
                              labelText: 'Status',
                            ),
                            items: const <DropdownMenuItem<String?>>[
                              DropdownMenuItem<String?>(
                                value: null,
                                child: Text('Todos'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'succeeded',
                                child: Text('Concluído'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'running',
                                child: Text('Executando'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'failed',
                                child: Text('Falhou'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'cancelled',
                                child: Text('Cancelado'),
                              ),
                            ],
                            onChanged: (v) {
                              setState(() => _statusFilter = v);
                              ref
                                  .read(linkTestsProvider.notifier)
                                  .fetchPage(
                                    page: 1,
                                    deviceId: _deviceFilter,
                                    status: v,
                                    kind: _kindFilter,
                                  );
                            },
                          ),
                        ),
                        ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 170),
                          child: DropdownButtonFormField<String?>(
                            initialValue: _kindFilter,
                            isDense: true,
                            decoration: const InputDecoration(
                              labelText: 'Modo',
                            ),
                            items: const <DropdownMenuItem<String?>>[
                              DropdownMenuItem<String?>(
                                value: null,
                                child: Text('Todos'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'link',
                                child: Text('Enlace'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'path',
                                child: Text('Caminho'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'server',
                                child: Text('Servidor'),
                              ),
                              DropdownMenuItem<String?>(
                                value: 'end_to_end',
                                child: Text('Ponta a ponta'),
                              ),
                            ],
                            onChanged: (v) {
                              setState(() => _kindFilter = v);
                              ref
                                  .read(linkTestsProvider.notifier)
                                  .fetchPage(
                                    page: 1,
                                    deviceId: _deviceFilter,
                                    status: _statusFilter,
                                    kind: v,
                                  );
                            },
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 12),
                    if (state.pageLoading && state.pageItems.isEmpty)
                      const LoadingStateView(
                        message: 'Carregando histórico de testes...',
                      )
                    else if (state.pageItems.isEmpty)
                      EmptyStateView(
                        title: 'Nenhum teste de velocidade registrado',
                        description: 'Clique em "Novo Teste" para medir a capacidade real de um enlace.',
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
                          child: ListTile(
                            onTap: () async {
                              await ref
                                  .read(linkTestsProvider.notifier)
                                  .fetchDetail(test.id);
                              ref
                                  .read(linkTestsProvider.notifier)
                                  .focus(test.id);
                            },
                            leading: StatusIndicator.fromStatus(
                              test.status,
                              compact: true,
                            ),
                            title: Text(
                              test.title,
                              style: const TextStyle(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            subtitle: Text(
                              '${test.kind} · ${countNoun(test.legs.length, 'perna', 'pernas')} · ${formatShortDateTime(test.createdAt)}',
                            ),
                            trailing: Wrap(
                              spacing: 6,
                              crossAxisAlignment: WrapCrossAlignment.center,
                              children: <Widget>[
                                if (test.summary?.minForwardBps != null)
                                  SemanticChip(
                                    label:
                                        '↓ ${formatBps(test.summary!.minForwardBps)}',
                                    tone: SemanticTone.success,
                                    compact: true,
                                  ),
                                if (test.summary?.minReverseBps != null)
                                  SemanticChip(
                                    label:
                                        '↑ ${formatBps(test.summary!.minReverseBps)}',
                                    tone: SemanticTone.info,
                                    compact: true,
                                  ),
                                IconButton(
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
                  ? EmptyStateView(
                      title: 'Nenhum agendamento configurado',
                      description: 'Programe medições recorrentes em janelas de baixa utilização.',
                      icon: Icons.schedule,
                      action: canWrite
                          ? FilledButton.icon(
                              onPressed: () => _openScheduleSheet(),
                              icon: const Icon(Icons.add),
                              label: const Text('Criar agendamento'),
                            )
                          : null,
                    )
                  : ListView.builder(
                      itemCount: state.schedules.length,
                      itemBuilder: (context, idx) {
                        final sched = state.schedules[idx];
                        return Card(
                          elevation: 2,
                          margin: const EdgeInsets.only(bottom: 8),
                          child: ListTile(
                            leading: Icon(
                              Icons.event_repeat,
                              color: sched.enabled
                                  ? context.semanticColors.success
                                  : context.colorScheme.onSurfaceVariant,
                            ),
                            title: Text(
                              sched.name,
                              style: const TextStyle(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            subtitle: Text(
                              'A cada ${sched.intervalMinutes} min · ${sched.kind}'
                              '${sched.windowStart != null ? ' · Janela ${sched.windowStart}–${sched.windowEnd ?? ''}' : ''}'
                              '${sched.nextRunAt != null ? ' · Próxima: ${formatTimeUntil(sched.nextRunAt)}' : ''}',
                            ),
                            trailing: canWrite
                                ? Wrap(
                                    spacing: 4,
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
                                        onPressed: () =>
                                            _openScheduleSheet(existing: sched),
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
                                  )
                                : null,
                          ),
                        );
                      },
                    ),
            ],
          ),
        ),
      ],
    );
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
          '${test.kind} · ${test.spec.direction} · ${test.spec.durationS}s (${test.spec.streams} streams)',
      icon: Icons.speed,
      iconTone: test.isActive ? SemanticTone.info : SemanticTone.success,
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          StatusIndicator.fromStatus(test.status, compact: true),
          if (test.isActive && canWrite) ...<Widget>[
            const SizedBox(width: 8),
            OutlinedButton.icon(
              onPressed: onCancel,
              icon: const Icon(Icons.stop, size: 16),
              label: const Text('Cancelar'),
            ),
          ],
          IconButton(
            tooltip: 'Fechar painel',
            icon: const Icon(Icons.close),
            onPressed: onClose,
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
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
                            : leg.status == 'succeeded'
                            ? SemanticTone.success
                            : SemanticTone.info,
                        padding: const EdgeInsets.all(10),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Text(
                              '${leg.a.name} → ${leg.b.name}',
                              style: context.textTheme.labelLarge?.copyWith(
                                fontWeight: FontWeight.w800,
                              ),
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
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: <Widget>[
              Text(
                'Comparação: #${a.id} vs #${b.id}',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
              IconButton(
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
        SegmentedButton<String>(
          segments: const <ButtonSegment<String>>[
            ButtonSegment(value: 'server', label: Text('Servidor')),
            ButtonSegment(value: 'link', label: Text('Enlace')),
            ButtonSegment(value: 'end_to_end', label: Text('A → B')),
            ButtonSegment(value: 'path', label: Text('Caminho')),
          ],
          selected: <String>{_kind},
          onSelectionChanged: (s) => setState(() {
            _kind = s.first;
            _plan = null;
          }),
        ),
        const SizedBox(height: 12),
        if (_kind == 'link')
          DropdownButtonFormField<int?>(
            initialValue: edges.any((e) => e.id == _linkId) ? _linkId : null,
            decoration: const InputDecoration(labelText: 'Enlace da topologia'),
            items: edges
                .map(
                  (e) => DropdownMenuItem<int?>(
                    value: e.id,
                    child: Text(
                      '${e.sourceDeviceName ?? '#${e.sourceDeviceId}'} ↔ ${e.targetDeviceName ?? '#${e.targetDeviceId}'}',
                    ),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) => setState(() => _linkId = v),
          )
        else ...<Widget>[
          if (_kind != 'server') ...<Widget>[
            DropdownButtonFormField<int?>(
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
                      child: Text(d.name),
                    ),
                  )
                  .toList(growable: false),
              onChanged: (v) => setState(() => _sourceDeviceId = v),
            ),
            const SizedBox(height: 10),
          ],
          DropdownButtonFormField<int?>(
            initialValue: devices.any((d) => d.id == _targetDeviceId)
                ? _targetDeviceId
                : null,
            decoration: const InputDecoration(labelText: 'Equipamento destino'),
            items: devices
                .map(
                  (DeviceModel d) =>
                      DropdownMenuItem<int?>(value: d.id, child: Text(d.name)),
                )
                .toList(growable: false),
            onChanged: (v) => setState(() => _targetDeviceId = v),
          ),
        ],
        const SizedBox(height: 12),
        Row(
          children: <Widget>[
            Expanded(
              child: DropdownButtonFormField<String>(
                initialValue: _direction,
                decoration: const InputDecoration(labelText: 'Sentido'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: 'both', child: Text('Ida e Volta')),
                  DropdownMenuItem(
                    value: 'forward',
                    child: Text('Somente Ida'),
                  ),
                  DropdownMenuItem(
                    value: 'reverse',
                    child: Text('Somente Volta'),
                  ),
                ],
                onChanged: (v) => setState(() => _direction = v ?? 'both'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: DropdownButtonFormField<int>(
                initialValue: _durationS,
                decoration: const InputDecoration(labelText: 'Duração'),
                items: const <DropdownMenuItem<int>>[
                  DropdownMenuItem(value: 5, child: Text('5 s')),
                  DropdownMenuItem(value: 10, child: Text('10 s')),
                  DropdownMenuItem(value: 20, child: Text('20 s')),
                  DropdownMenuItem(value: 30, child: Text('30 s')),
                ],
                onChanged: (v) => setState(() => _durationS = v ?? 10),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: DropdownButtonFormField<int>(
                initialValue: _streams,
                decoration: const InputDecoration(labelText: 'Streams'),
                items: const <DropdownMenuItem<int>>[
                  DropdownMenuItem(value: 1, child: Text('1')),
                  DropdownMenuItem(value: 4, child: Text('4')),
                  DropdownMenuItem(value: 8, child: Text('8')),
                ],
                onChanged: (v) => setState(() => _streams = v ?? 4),
              ),
            ),
          ],
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
        Row(
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
            const SizedBox(width: 8),
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
                    child: Text('⚠ $w', style: context.textTheme.bodySmall),
                  ),
                for (final b in _plan!.blocking)
                  Padding(
                    padding: const EdgeInsets.only(top: 4),
                    child: Text(
                      '✖ $b',
                      style: TextStyle(color: context.colorScheme.error),
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

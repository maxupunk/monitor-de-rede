import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class SaasServicesWidget extends ConsumerStatefulWidget {
  const SaasServicesWidget({
    super.key,
    this.config = const <String, dynamic>{},
  });

  final Map<String, dynamic> config;

  @override
  ConsumerState<SaasServicesWidget> createState() => _SaasServicesWidgetState();
}

class _SaasServicesWidgetState extends ConsumerState<SaasServicesWidget> {
  List<DeviceMonitorSummary> _monitors = const <DeviceMonitorSummary>[];
  bool _loading = true;
  VoidCallback? _unsub;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(
      const <String>['monitor:result'],
      (data, _) {
        final id = parseIntOrNull(data['monitorId'] ?? data['id']);
        if (id == null || !mounted) return;
        setState(() {
          _monitors = _monitors
              .map((m) {
                if (m.id != id) return m;
                return m.copyWith(
                  status: data['status'] as String?,
                  latencyMs: parseDoubleOrNull(data['latencyMs']),
                  lastCheckedAt: data['finishedAt'] as String?,
                );
              })
              .toList(growable: false);
        });
      },
    );
  }

  @override
  void dispose() {
    _unsub?.call();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final list = await ref
          .read(diagnosticsServiceProvider)
          .fetchSaasMonitors();
      if (mounted) {
        setState(() {
          _monitors = list;
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 520;
        final catalogBtn = FilledButton.tonalIcon(
          onPressed: () async {
            await showSaasPresetsDialog(context);
            _load();
          },
          icon: const Icon(Icons.add, size: 18),
          label: const Text('Catálogo SaaS'),
        );
        return SectionCard(
          title: 'Serviços SaaS e Nuvem',
          subtitle: 'Monitoramento de experiência (QoE) de serviços externos',
          icon: Icons.cloud_done_outlined,
          iconTone: SemanticTone.secondary,
          trailing: isCompact ? null : catalogBtn,
          footer: isCompact
              ? Align(alignment: Alignment.centerRight, child: catalogBtn)
              : null,
          child: _loading
              ? const LoadingStateView(message: 'Carregando serviços SaaS...')
              : _monitors.isEmpty
              ? EmptyStateView(
                  title: 'Nenhum serviço SaaS provisionado',
                  description:
                      'Adicione bancos, nuvens e ferramentas de produtividade em um clique.',
                  icon: Icons.cloud_outlined,
                  action: FilledButton.icon(
                    onPressed: () async {
                      await showSaasPresetsDialog(context);
                      _load();
                    },
                    icon: const Icon(Icons.add),
                    label: const Text('Abrir catálogo SaaS'),
                  ),
                )
              : Column(
                  children: _monitors
                      .map((m) {
                        return ListTile(
                          dense: true,
                          contentPadding: EdgeInsets.zero,
                          leading: StatusIndicator.fromStatus(
                            m.status,
                            compact: true,
                          ),
                          title: Text(
                            m.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          subtitle: Text(
                            m.target ?? m.type,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          trailing: Text(
                            formatLatency(m.latencyMs),
                            style: context.textTheme.labelLarge?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        );
                      })
                      .toList(growable: false),
                ),
        );
      },
    );
  }
}

Future<void> showSaasPresetsDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Catálogo de Serviços SaaS',
    subtitle: 'Provisione monitores prontos por categoria',
    maxWidth: 680,
    builder: (ctx) => _SaasPresetsBody(onDone: () => Navigator.of(ctx).pop()),
  );
}

class _SaasPresetsBody extends ConsumerStatefulWidget {
  const _SaasPresetsBody({required this.onDone});

  final VoidCallback onDone;

  @override
  ConsumerState<_SaasPresetsBody> createState() => _SaasPresetsBodyState();
}

class _SaasPresetsBodyState extends ConsumerState<_SaasPresetsBody> {
  static const List<(String, String)> _categories = <(String, String)>[
    ('all', 'Todos'),
    ('finance', 'Bancos'),
    ('productivity', 'Produtividade'),
    ('cloud', 'Nuvem'),
    ('communication', 'Comunicação'),
    ('developer', 'Dev'),
    ('streaming', 'Streaming'),
    ('government', 'Governo'),
  ];

  List<SaasPresetItem> _presets = const <SaasPresetItem>[];
  final Set<String> _selected = <String>{};
  String _category = 'all';
  String _search = '';
  bool _loading = true;
  bool _provisioning = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final list = await ref
          .read(diagnosticsServiceProvider)
          .fetchSaasPresets();
      if (mounted) {
        setState(() {
          _presets = list;
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _provisionIds(List<String> ids) async {
    if (ids.isEmpty) return;
    setState(() => _provisioning = true);
    try {
      await ref.read(diagnosticsServiceProvider).provisionSaasPresets(ids);
      if (mounted) widget.onDone();
    } finally {
      if (mounted) setState(() => _provisioning = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Carregando catálogo SaaS...');
    }
    final filtered = _presets
        .where((p) {
          if (_category != 'all' && p.category != _category) return false;
          if (_search.isNotEmpty) {
            final q = _search.toLowerCase();
            return p.name.toLowerCase().contains(q) ||
                p.target.toLowerCase().contains(q);
          }
          return true;
        })
        .toList(growable: false);

    final recommended = _presets
        .where((p) => p.recommended && !p.alreadyProvisioned)
        .take(6)
        .map((p) => p.id)
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          decoration: const InputDecoration(
            hintText: 'Buscar serviço SaaS...',
            prefixIcon: Icon(Icons.search),
          ),
          onChanged: (v) => setState(() => _search = v),
        ),
        const SizedBox(height: 8),
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            children: _categories
                .map((cat) {
                  return Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: ChoiceChip(
                      label: Text(cat.$2),
                      selected: _category == cat.$1,
                      onSelected: (_) => setState(() => _category = cat.$1),
                    ),
                  );
                })
                .toList(growable: false),
          ),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            if (recommended.isNotEmpty)
              OutlinedButton.icon(
                onPressed: _provisioning
                    ? null
                    : () => _provisionIds(recommended),
                icon: const Icon(Icons.auto_awesome, size: 18),
                label: Text('Provisionar Recomendados (${recommended.length})'),
              ),
            FilledButton.icon(
              onPressed: _provisioning || _selected.isEmpty
                  ? null
                  : () => _provisionIds(_selected.toList(growable: false)),
              icon: const Icon(Icons.check, size: 18),
              label: Text('Provisionar selecionados (${_selected.length})'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        for (final preset in filtered)
          CheckboxListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            title: Text(preset.name),
            subtitle: Text('${preset.target} · ${preset.category}'),
            secondary: preset.alreadyProvisioned
                ? const SemanticChip(
                    label: 'Ativo',
                    tone: SemanticTone.success,
                    compact: true,
                  )
                : null,
            value: preset.alreadyProvisioned || _selected.contains(preset.id),
            onChanged: preset.alreadyProvisioned
                ? null
                : (v) {
                    setState(() {
                      if (v == true) {
                        _selected.add(preset.id);
                      } else {
                        _selected.remove(preset.id);
                      }
                    });
                  },
          ),
      ],
    );
  }
}

class SaasLatencyHeatmapWidget extends ConsumerStatefulWidget {
  const SaasLatencyHeatmapWidget({
    super.key,
    this.config = const <String, dynamic>{},
  });

  final Map<String, dynamic> config;

  @override
  ConsumerState<SaasLatencyHeatmapWidget> createState() =>
      _SaasLatencyHeatmapWidgetState();
}

class _SaasLatencyHeatmapWidgetState
    extends ConsumerState<SaasLatencyHeatmapWidget> {
  int _days = 7;
  bool _loading = true;
  List<HeatmapCell> _cells = const <HeatmapCell>[];

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final cells = await ref
          .read(diagnosticsServiceProvider)
          .fetchHourlyHeatmap(
            monitorId: parseIntOrNull(widget.config['monitorId']),
            days: _days,
          );
      if (mounted) {
        setState(() {
          _cells = cells;
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Color _cellColor(BuildContext context, HeatmapCell? cell) {
    final scheme = context.colorScheme;
    final sem = context.semanticColors;
    if (cell == null || cell.checks == 0) return scheme.surfaceContainerHighest;
    if (cell.downChecks > 0) return sem.error;
    final ms = cell.avgLatencyMs ?? 0;
    if (ms < 50) return sem.success;
    if (ms < 150) return sem.info;
    if (ms < 300) return sem.warning;
    return sem.caution;
  }

  @override
  Widget build(BuildContext context) {
    final days = _cells.map((c) => c.day).toSet().toList()..sort();
    final lookup = <String, HeatmapCell>{
      for (final c in _cells) '${c.day}:${c.hour}': c,
    };

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 520;
        final daysToggle = SegmentedButton<int>(
          showSelectedIcon: !isCompact,
          segments: const <ButtonSegment<int>>[
            ButtonSegment(value: 7, label: Text('7d')),
            ButtonSegment(value: 14, label: Text('14d')),
          ],
          selected: <int>{_days},
          onSelectionChanged: (s) {
            setState(() => _days = s.first);
            _load();
          },
        );
        return SectionCard(
          title: 'Mapa de Calor · Latência por Hora',
          subtitle: 'Média horária dos últimos $_days dias',
          icon: Icons.grid_on_outlined,
          iconTone: SemanticTone.warning,
          trailing: isCompact ? null : daysToggle,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (isCompact) ...<Widget>[
                Align(alignment: Alignment.centerLeft, child: daysToggle),
                const SizedBox(height: 10),
              ],
              if (_loading)
                const LoadingStateView(message: 'Gerando mapa de calor...')
              else if (days.isEmpty)
                const EmptyStateView(
                  title: 'Sem histórico horário suficiente',
                  icon: Icons.grid_off_outlined,
                )
              else
                SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          const SizedBox(width: 72),
                          for (var h = 0; h < 24; h += 3)
                            SizedBox(
                              width: 48,
                              child: Text(
                                formatHourOfDay(h),
                                style: context.textTheme.labelSmall,
                              ),
                            ),
                        ],
                      ),
                      const SizedBox(height: 4),
                      for (final day in days)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 4),
                          child: Row(
                            children: <Widget>[
                              SizedBox(
                                width: 72,
                                child: Text(
                                  day.length >= 10 ? day.substring(5, 10) : day,
                                  style: context.textTheme.labelSmall,
                                ),
                              ),
                              for (var h = 0; h < 24; h++)
                                Tooltip(
                                  message: () {
                                    final c = lookup['$day:$h'];
                                    if (c == null || c.checks == 0) {
                                      return '$day ${formatHourOfDay(h)}: sem medições';
                                    }
                                    return '$day ${formatHourOfDay(h)} · Média ${formatLatency(c.avgLatencyMs)}'
                                        '${c.p95LatencyMs != null ? ' · P95 ${formatLatency(c.p95LatencyMs)}' : ''}'
                                        ' (${c.checks} checks)';
                                  }(),
                                  child: Container(
                                    width: 14,
                                    height: 14,
                                    margin: const EdgeInsets.only(right: 2),
                                    decoration: BoxDecoration(
                                      color: _cellColor(
                                        context,
                                        lookup['$day:$h'],
                                      ),
                                      borderRadius: BorderRadius.circular(3),
                                    ),
                                  ),
                                ),
                            ],
                          ),
                        ),
                    ],
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

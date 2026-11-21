import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── 1. DnsServersCard (Settings -> Conectividade) ─────────────────────────────

class DnsServersCard extends ConsumerStatefulWidget {
  const DnsServersCard({super.key});

  @override
  ConsumerState<DnsServersCard> createState() => _DnsServersCardState();
}

class _DnsServersCardState extends ConsumerState<DnsServersCard> {
  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(dnsServersProvider.notifier).fetchServers(),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(dnsServersProvider);
    final canWrite = ref.watch(authProvider).canWrite;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 560;
        final actions = canWrite
            ? Wrap(
                spacing: 8,
                runSpacing: 8,
                children: <Widget>[
                  OutlinedButton.icon(
                    onPressed: () => showDnsBatchMonitorDialog(context),
                    icon: const Icon(Icons.playlist_add_check, size: 18),
                    label: const Text('Monitorar em lote'),
                  ),
                  FilledButton.icon(
                    onPressed: () => showDnsServersDialog(context),
                    icon: const Icon(Icons.add, size: 18),
                    label: const Text('Gerenciar'),
                  ),
                ],
              )
            : null;

        return SectionCard(
          title: 'Servidores DNS',
          subtitle:
              'Resolvers UDP, TCP e DoH usados no benchmark e monitoramento em lote',
          icon: Icons.dns_outlined,
          trailing: isCompact ? null : actions,
          footer: isCompact && actions != null ? actions : null,
          child: state.loading && state.items.isEmpty
              ? const LoadingStateView(message: 'Carregando servidores DNS...')
              : state.items.isEmpty
              ? const EmptyStateView(
                  title: 'Nenhum servidor DNS cadastrado',
                  description:
                      'Adicione resolvers como 1.1.1.1, 8.8.8.8 ou seu DNS interno.',
                  icon: Icons.dns_outlined,
                )
              : Column(
                  children: state.items
                      .map((server) {
                        return ListTile(
                          contentPadding: EdgeInsets.zero,
                          leading: SemanticChip(
                            label: server.protocol.toUpperCase(),
                            tone: server.protocol == 'doh'
                                ? SemanticTone.success
                                : SemanticTone.info,
                            compact: true,
                          ),
                          title: Text(
                            server.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          subtitle: Text(
                            server.address,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontFamily: 'monospace'),
                          ),
                          trailing: server.isDefault
                              ? const SemanticChip(
                                  label: 'Benchmark',
                                  tone: SemanticTone.primary,
                                  compact: true,
                                )
                              : null,
                        );
                      })
                      .toList(growable: false),
                ),
        );
      },
    );
  }
}

// ── 2. DnsServersDialog (CRUD) ────────────────────────────────────────────────

Future<void> showDnsServersDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Servidores DNS',
    subtitle: 'Cadastre e organize resolvers para comparação e monitoramento',
    maxWidth: 640,
    builder: (_) => const _DnsServersDialogBody(),
  );
}

class _DnsServersDialogBody extends ConsumerStatefulWidget {
  const _DnsServersDialogBody();

  @override
  ConsumerState<_DnsServersDialogBody> createState() =>
      _DnsServersDialogBodyState();
}

class _DnsServersDialogBodyState extends ConsumerState<_DnsServersDialogBody> {
  final _nameCtrl = TextEditingController();
  final _addressCtrl = TextEditingController();
  final _descCtrl = TextEditingController();
  String _protocol = 'udp';
  bool _isDefault = true;
  int? _editingId;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(dnsServersProvider.notifier).fetchServers(),
    );
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _addressCtrl.dispose();
    _descCtrl.dispose();
    super.dispose();
  }

  void _startEdit(DnsServerModel s) {
    setState(() {
      _editingId = s.id;
      _nameCtrl.text = s.name;
      _addressCtrl.text = s.address;
      _descCtrl.text = s.description ?? '';
      _protocol = s.protocol;
      _isDefault = s.isDefault;
    });
  }

  void _resetForm() {
    setState(() {
      _editingId = null;
      _nameCtrl.clear();
      _addressCtrl.clear();
      _descCtrl.clear();
      _protocol = 'udp';
      _isDefault = true;
    });
  }

  Future<void> _save() async {
    if (_nameCtrl.text.trim().isEmpty || _addressCtrl.text.trim().isEmpty) {
      return;
    }
    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'address': _addressCtrl.text.trim(),
      'protocol': _protocol,
      'isDefault': _isDefault,
      'description': _descCtrl.text.trim().isEmpty
          ? null
          : _descCtrl.text.trim(),
    };
    final notifier = ref.read(dnsServersProvider.notifier);
    final ok = _editingId != null
        ? await notifier.updateServer(_editingId!, payload)
        : await notifier.createServer(payload);
    if (ok && mounted) _resetForm();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(dnsServersProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              ResponsiveFieldRow(
                breakpoint: 480,
                children: <Widget>[
                  TextField(
                    controller: _nameCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Nome (ex: Cloudflare)',
                    ),
                  ),
                  AppDropdownFormField<String>(
                    initialValue: _protocol,
                    decoration: const InputDecoration(labelText: 'Protocolo'),
                    items: const <DropdownMenuItem<String>>[
                      DropdownMenuItem(
                        value: 'udp',
                        child: Text('UDP', overflow: TextOverflow.ellipsis),
                      ),
                      DropdownMenuItem(
                        value: 'tcp',
                        child: Text('TCP', overflow: TextOverflow.ellipsis),
                      ),
                      DropdownMenuItem(
                        value: 'doh',
                        child: Text(
                          'DoH (HTTPS)',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                    onChanged: (v) => setState(() => _protocol = v ?? 'udp'),
                  ),
                ],
              ),
              const SizedBox(height: 10),
              TextField(
                controller: _addressCtrl,
                decoration: const InputDecoration(
                  labelText: 'Endereço (IP, IP:porta ou URL DoH)',
                  hintText: '1.1.1.1 ou https://cloudflare-dns.com/dns-query',
                ),
              ),
              const SizedBox(height: 8),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Incluir no benchmark padrão do dashboard'),
                value: _isDefault,
                onChanged: (v) => setState(() => _isDefault = v),
              ),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: 8,
                runSpacing: 8,
                children: <Widget>[
                  if (_editingId != null)
                    TextButton(
                      onPressed: _resetForm,
                      child: const Text('Cancelar edição'),
                    ),
                  FilledButton.icon(
                    onPressed: state.saving ? null : _save,
                    icon: const Icon(Icons.save_outlined, size: 18),
                    label: Text(
                      _editingId != null ? 'Atualizar' : 'Adicionar DNS',
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 14),
        for (final server in state.items) ...<Widget>[
          NestedPanel(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        server.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                      Text(
                        '${server.address} · ${server.protocol.toUpperCase()}',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.bodySmall?.copyWith(
                          fontFamily: 'monospace',
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: 'Editar',
                  icon: const Icon(Icons.edit_outlined, size: 18),
                  onPressed: () => _startEdit(server),
                ),
                IconButton(
                  tooltip: 'Excluir',
                  icon: const Icon(Icons.delete_outline, size: 18),
                  onPressed: () => ref
                      .read(dnsServersProvider.notifier)
                      .deleteServer(server.id),
                ),
              ],
            ),
          ),
          const SizedBox(height: 6),
        ],
      ],
    );
  }
}

// ── 3. DnsBatchMonitorDialog ──────────────────────────────────────────────────

Future<void> showDnsBatchMonitorDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Criar monitores DNS em lote',
    subtitle:
        'Provisione monitores de consulta DNS e ping para múltiplos resolvers',
    maxWidth: 600,
    builder: (ctx) =>
        _DnsBatchMonitorBody(onDone: () => Navigator.of(ctx).pop()),
  );
}

class _DnsBatchMonitorBody extends ConsumerStatefulWidget {
  const _DnsBatchMonitorBody({required this.onDone});

  final VoidCallback onDone;

  @override
  ConsumerState<_DnsBatchMonitorBody> createState() =>
      _DnsBatchMonitorBodyState();
}

class _DnsBatchMonitorBodyState extends ConsumerState<_DnsBatchMonitorBody> {
  final Set<int> _selectedIds = <int>{};
  final _domainsCtrl = TextEditingController(
    text: 'google.com, cloudflare.com',
  );
  String _recordType = 'A';
  int _intervalSeconds = 60;
  bool _includePing = true;
  bool _executeNow = true;
  DnsBatchProvisionResponse? _result;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() async {
      await ref.read(dnsServersProvider.notifier).fetchServers();
      if (!mounted) return;
      setState(() {
        _selectedIds.addAll(
          ref
              .read(dnsServersProvider)
              .items
              .where((s) => s.isDefault)
              .map((s) => s.id),
        );
      });
    });
  }

  @override
  void dispose() {
    _domainsCtrl.dispose();
    super.dispose();
  }

  Future<void> _provision() async {
    final servers = ref
        .read(dnsServersProvider)
        .items
        .where((s) => _selectedIds.contains(s.id))
        .map(
          (s) => <String, dynamic>{
            'server': s.address,
            'name': s.name,
            'protocol': s.protocol,
            if (s.protocol == 'doh') 'dohUrl': s.address,
          },
        )
        .toList(growable: false);
    if (servers.isEmpty) return;

    final domains = _domainsCtrl.text
        .split(RegExp(r'[\s,;]+'))
        .map((e) => e.trim())
        .where((e) => e.isNotEmpty)
        .toList(growable: false);

    final res = await ref
        .read(dnsPerformanceProvider.notifier)
        .provisionMonitors(<String, dynamic>{
          'servers': servers,
          'domains': domains.isEmpty ? <String>['google.com'] : domains,
          'recordType': _recordType,
          'intervalSeconds': _intervalSeconds,
          'includePing': _includePing,
          'executeNow': _executeNow,
        });
    if (mounted && res != null) {
      setState(() => _result = res);
    }
  }

  @override
  Widget build(BuildContext context) {
    final servers = ref.watch(dnsServersProvider).items;
    final perf = ref.watch(dnsPerformanceProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          'Selecione os servidores DNS (${_selectedIds.length})',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 6),
        for (final s in servers)
          CheckboxListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            title: Text(s.name),
            subtitle: Text('${s.address} (${s.protocol.toUpperCase()})'),
            value: _selectedIds.contains(s.id),
            onChanged: (v) {
              setState(() {
                if (v == true) {
                  _selectedIds.add(s.id);
                } else {
                  _selectedIds.remove(s.id);
                }
              });
            },
          ),
        const SizedBox(height: 10),
        TextField(
          controller: _domainsCtrl,
          decoration: const InputDecoration(
            labelText: 'Domínios de consulta (separados por vírgula)',
          ),
        ),
        const SizedBox(height: 10),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            AppDropdownFormField<String>(
              initialValue: _recordType,
              decoration: const InputDecoration(labelText: 'Registro'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'A',
                  child: Text('A', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'AAAA',
                  child: Text('AAAA', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'MX',
                  child: Text('MX', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'TXT',
                  child: Text('TXT', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) => setState(() => _recordType = v ?? 'A'),
            ),
            AppDropdownFormField<int>(
              initialValue: _intervalSeconds,
              decoration: const InputDecoration(labelText: 'Intervalo'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(
                  value: 30,
                  child: Text('30 s', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 60,
                  child: Text('1 min', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 300,
                  child: Text('5 min', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) => setState(() => _intervalSeconds = v ?? 60),
            ),
          ],
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Incluir monitores ICMP Ping acompanhantes'),
          value: _includePing,
          onChanged: (v) => setState(() => _includePing = v),
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Executar primeira medição imediatamente'),
          value: _executeNow,
          onChanged: (v) => setState(() => _executeNow = v),
        ),
        if (_result != null) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.success,
            child: Text(
              '${_result!.createdCount} monitores criados · ${_result!.alreadyMonitoredCount} já existiam.',
            ),
          ),
          const SizedBox(height: 10),
        ],
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: perf.provisioning || _selectedIds.isEmpty
                ? null
                : _provision,
            icon: const Icon(Icons.playlist_add_check),
            label: Text(
              perf.provisioning ? 'Provisionando...' : 'Provisionar monitores',
            ),
          ),
        ),
      ],
    );
  }
}

// ── 4. DnsLatencyCard (Dashboard Widget `dns_latency`) ────────────────────────

class DnsLatencyCard extends ConsumerStatefulWidget {
  const DnsLatencyCard({super.key, this.config = const <String, dynamic>{}});

  final Map<String, dynamic> config;

  @override
  ConsumerState<DnsLatencyCard> createState() => _DnsLatencyCardState();
}

class _DnsLatencyCardState extends ConsumerState<DnsLatencyCard> {
  int _hours = 24;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref
          .read(dnsPerformanceProvider.notifier)
          .fetchPerformance(hours: _hours),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(dnsPerformanceProvider);
    final slowest = state.slowestLatency;

    final chartSeries = state.series
        .take(4)
        .toList()
        .asMap()
        .entries
        .map((entry) {
          final tones = <SemanticTone>[
            SemanticTone.primary,
            SemanticTone.success,
            SemanticTone.info,
            SemanticTone.warning,
          ];
          final points = <MetricSeriesPoint>[];
          for (var i = 0; i < entry.value.points.length; i++) {
            final ms = entry.value.points[i].latencyMs;
            if (ms != null) {
              points.add(MetricSeriesPoint(x: i.toDouble(), y: ms));
            }
          }
          return MetricLineSeries(
            name: entry.value.label,
            points: points,
            tone: tones[entry.key % tones.length],
            showArea: false,
          );
        })
        .toList(growable: false);

    return SectionCard(
      title: 'Desempenho DNS',
      subtitle: state.source == 'benchmark'
          ? 'Benchmark ao vivo'
          : 'Ranking das últimas ${_hours}h (${state.monitorCount} monitores)',
      icon: Icons.dns_outlined,
      iconTone: SemanticTone.info,
      trailing: Wrap(
        spacing: 2,
        runSpacing: 2,
        children: <Widget>[
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: 'Benchmark ao vivo',
            icon: state.benchmarking
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.bolt),
            onPressed: state.benchmarking
                ? null
                : () =>
                      ref.read(dnsPerformanceProvider.notifier).runBenchmark(),
          ),
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: 'Configurar servidores DNS',
            icon: const Icon(Icons.tune),
            onPressed: () => showDnsServersDialog(context),
          ),
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: 'Criar monitores DNS em lote',
            icon: const Icon(Icons.playlist_add),
            onPressed: () => showDnsBatchMonitorDialog(context),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 6,
            children: <int>[6, 24, 168]
                .map((h) {
                  return ChoiceChip(
                    label: Text(h == 168 ? '7d' : '${h}h'),
                    selected: state.source == 'history' && _hours == h,
                    onSelected: (_) {
                      setState(() => _hours = h);
                      ref
                          .read(dnsPerformanceProvider.notifier)
                          .fetchPerformance(hours: h);
                    },
                  );
                })
                .toList(growable: false),
          ),
          const SizedBox(height: 12),
          if (state.loading && state.ranking.isEmpty)
            const LoadingStateView(message: 'Medindo latência DNS...')
          else if (state.ranking.isEmpty)
            EmptyStateView(
              title: 'Sem medições DNS ainda',
              description:
                  'Execute o benchmark ao vivo ou provisione monitores DNS.',
              icon: Icons.dns_outlined,
              action: FilledButton.icon(
                onPressed: () =>
                    ref.read(dnsPerformanceProvider.notifier).runBenchmark(),
                icon: const Icon(Icons.bolt),
                label: const Text('Rodar Benchmark Agora'),
              ),
            )
          else ...<Widget>[
            for (var i = 0; i < state.ranking.length; i++) ...<Widget>[
              _DnsRankingBar(
                rank: i + 1,
                entry: state.ranking[i],
                maxLatency: slowest,
              ),
              const SizedBox(height: 8),
            ],
            if (chartSeries.any((s) => s.points.length >= 2)) ...<Widget>[
              const SizedBox(height: 8),
              MetricLineChart(
                series: chartSeries,
                height: 160,
                yFormatter: (v) => formatLatency(v),
              ),
            ],
          ],
        ],
      ),
    );
  }
}

class _DnsRankingBar extends StatelessWidget {
  const _DnsRankingBar({
    required this.rank,
    required this.entry,
    required this.maxLatency,
  });

  final int rank;
  final DnsRankingEntry entry;
  final double maxLatency;

  @override
  Widget build(BuildContext context) {
    final avg = entry.avgLookupTimeMs;
    final ratio = (avg != null && maxLatency > 0)
        ? (avg / maxLatency).clamp(0.08, 1.0)
        : 0.0;
    final tone = rank == 1
        ? SemanticTone.success
        : (avg == null ? SemanticTone.error : SemanticTone.info);
    final colors = context.semanticColors.resolve(tone, context.colorScheme);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              child: Text(
                '#$rank ${entry.label} (${entry.protocol.toUpperCase()})',
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(width: 8),
            Text(
              avg != null
                  ? '${formatLatency(avg)} · ${formatPercent(entry.normalizedSuccessPct, digits: 0)}'
                  : 'Falha',
              style: context.textTheme.labelMedium?.copyWith(
                fontWeight: FontWeight.w700,
                color: colors.foreground,
              ),
            ),
          ],
        ),
        const SizedBox(height: 4),
        ClipRRect(
          borderRadius: BorderRadius.circular(6),
          child: LinearProgressIndicator(
            value: ratio,
            minHeight: 8,
            color: colors.foreground,
            backgroundColor: context.colorScheme.surfaceContainerHighest,
          ),
        ),
      ],
    );
  }
}

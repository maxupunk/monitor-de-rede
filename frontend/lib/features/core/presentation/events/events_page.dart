import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';
import 'package:netmonitor_app/features/core/providers/events_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Tela de Eventos em Tempo Real (SSE) + Histórico Paginado (paridade com `EventsPage.vue`).
class EventsPage extends ConsumerStatefulWidget {
  const EventsPage({super.key});

  @override
  ConsumerState<EventsPage> createState() => _EventsPageState();
}

class _EventsPageState extends ConsumerState<EventsPage>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  final TextEditingController _searchCtrl = TextEditingController();
  String _categoryFilter = 'Todos';
  bool _paused = false;
  List<SseEvent> _frozenSnapshot = const <SseEvent>[];
  DateTime? _clearedAfter;

  static const List<String> _categories = <String>[
    'Todos',
    'Alertas',
    'Monitores',
    'Dispositivos',
    'Operação',
    'Sistema',
  ];

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
    _tabController.addListener(() {
      if (!_tabController.indexIsChanging && _tabController.index == 1) {
        if (ref.read(eventsHistoryProvider).items.isEmpty) {
          ref.read(eventsHistoryProvider.notifier).fetchPage(page: 1);
        }
      }
    });
  }

  @override
  void dispose() {
    _tabController.dispose();
    _searchCtrl.dispose();
    super.dispose();
  }

  void _togglePause(List<SseEvent> recentEvents) {
    setState(() {
      if (_paused) {
        _paused = false;
      } else {
        _frozenSnapshot = List<SseEvent>.from(recentEvents);
        _paused = true;
      }
    });
  }

  Future<void> _showEventDetail({
    required String type,
    required String timestamp,
    required Map<String, dynamic> data,
  }) async {
    final p = EventPresentation.describe(type, data);
    final prettyJson = const JsonEncoder.withIndent('  ').convert(data);

    await showAdaptiveFormSheet<void>(
      context: context,
      title: p.label,
      subtitle: '$type · ${formatDateTime(timestamp)}',
      maxWidth: 640,
      builder: (context) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              SemanticChip(label: p.category, tone: p.tone, icon: p.icon),
              SemanticChip(label: type, tone: SemanticTone.secondary),
            ],
          ),
          const SizedBox(height: 12),
          KeyValueRow(label: 'Resumo', value: p.summary),
          KeyValueRow(label: 'Horário', value: formatDateTime(timestamp)),
          const SizedBox(height: 12),
          CodeViewer(title: 'Payload JSON', code: prettyJson, maxHeight: 340),
        ],
      ),
    );
  }

  Widget _buildHeader(
    BuildContext context,
    RealtimeState realtime, {
    required bool isCompact,
  }) {
    final scheme = context.colorScheme;
    final sem = context.semanticColors;
    final dotColor = _paused
        ? sem.warning
        : (realtime.isConnected ? sem.success : sem.warning);

    final categoryChips = <Widget>[
      for (final cat in _categories)
        FilterChip(
          visualDensity: VisualDensity.compact,
          materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
          selected: _categoryFilter == cat,
          label: Text(cat),
          onSelected: (_) => setState(() => _categoryFilter = cat),
        ),
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (isCompact)
          Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  'Eventos',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.headlineSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                    color: scheme.onSurface,
                  ),
                ),
              ),
              FilledButton.tonal(
                style: FilledButton.styleFrom(
                  visualDensity: VisualDensity.compact,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 8,
                  ),
                ),
                onPressed: () => _togglePause(realtime.recentEvents),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Container(
                      width: 8,
                      height: 8,
                      decoration: BoxDecoration(
                        color: dotColor,
                        shape: BoxShape.circle,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Icon(_paused ? Icons.play_arrow : Icons.pause, size: 16),
                    const SizedBox(width: 4),
                    Text(_paused ? 'Retomar' : 'Pausar'),
                  ],
                ),
              ),
              IconButton(
                tooltip: 'Limpar tela',
                visualDensity: VisualDensity.compact,
                icon: const Icon(Icons.clear_all),
                onPressed: () => setState(() => _clearedAfter = DateTime.now()),
              ),
            ],
          )
        else
          Wrap(
            spacing: 12,
            runSpacing: 10,
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    'Eventos',
                    style: context.textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                      color: scheme.onSurface,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    'Fluxo SSE em tempo real e histórico persistido',
                    style: context.textTheme.bodyMedium?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  SemanticChip(
                    label: realtime.isConnected
                        ? 'SSE Conectado'
                        : 'Reconectando...',
                    tone: realtime.isConnected
                        ? SemanticTone.success
                        : SemanticTone.warning,
                    icon: realtime.isConnected
                        ? Icons.radio_button_checked
                        : Icons.sync,
                  ),
                  OutlinedButton.icon(
                    onPressed: () => _togglePause(realtime.recentEvents),
                    icon: Icon(
                      _paused ? Icons.play_arrow : Icons.pause,
                      size: 18,
                    ),
                    label: Text(_paused ? 'Retomar' : 'Pausar'),
                  ),
                  TextButton.icon(
                    onPressed: () =>
                        setState(() => _clearedAfter = DateTime.now()),
                    icon: const Icon(Icons.clear_all, size: 18),
                    label: const Text('Limpar tela'),
                  ),
                ],
              ),
            ],
          ),
        const SizedBox(height: 10),
        UnifiedFilterBar(
          searchController: _searchCtrl,
          searchHint: 'Filtrar por tipo ou mensagem...',
          onSearchChanged: (_) => setState(() {}),
          quickChips: categoryChips,
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final realtime = ref.watch(sseServiceProvider);
    final history = ref.watch(eventsHistoryProvider);

    final rawLive = _paused ? _frozenSnapshot : realtime.recentEvents;
    final liveEvents = rawLive
        .where((ev) {
          if (_clearedAfter != null) {
            final ts = parseDateOrNull(ev.timestamp);
            if (ts != null && ts.isBefore(_clearedAfter!)) return false;
          }
          final p = EventPresentation.describe(ev.type, ev.data);
          if (_categoryFilter != 'Todos' && p.category != _categoryFilter) {
            return false;
          }
          final q = _searchCtrl.text.trim().toLowerCase();
          if (q.isNotEmpty) {
            final match =
                ev.type.toLowerCase().contains(q) ||
                p.label.toLowerCase().contains(q) ||
                p.summary.toLowerCase().contains(q);
            if (!match) return false;
          }
          return true;
        })
        .toList(growable: false);

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;

        return NestedScrollView(
          headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: _buildHeader(context, realtime, isCompact: isCompact),
              ),
            ),
            SliverPersistentHeader(
              pinned: true,
              delegate: PinnedSliverHeaderDelegate(
                height: 48,
                bottomBorder: true,
                child: TabBar(
                  controller: _tabController,
                  isScrollable: isCompact,
                  tabAlignment: isCompact
                      ? TabAlignment.start
                      : TabAlignment.fill,
                  tabs: <Widget>[
                    Tab(text: 'Tempo Real (${liveEvents.length})'),
                    Tab(
                      text:
                          'Histórico (${history.meta?.total ?? history.items.length})',
                    ),
                  ],
                ),
              ),
            ),
          ],
          body: TabBarView(
            controller: _tabController,
            children: <Widget>[
              _buildLiveTab(liveEvents),
              _buildHistoryTab(history),
            ],
          ),
        );
      },
    );
  }

  Widget _buildLiveTab(List<SseEvent> liveEvents) {
    if (liveEvents.isEmpty) {
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: const <Widget>[
          EmptyStateView(
            title: 'Nenhum evento em tempo real',
            description:
                'Os eventos do barramento SSE aparecerão aqui instantaneamente.',
            icon: Icons.bolt_outlined,
          ),
        ],
      );
    }

    return ListView.separated(
      padding: const EdgeInsets.only(top: 10, bottom: 24),
      itemCount: liveEvents.length,
      separatorBuilder: (_, _) => const SizedBox(height: 8),
      itemBuilder: (context, index) {
        final ev = liveEvents[index];
        final p = EventPresentation.describe(ev.type, ev.data);
        return _EventFeedCard(
          presentation: p,
          title: p.label,
          summary: p.summary,
          timestampLabel: formatClockTime(ev.timestamp),
          onTap: () => _showEventDetail(
            type: ev.type,
            timestamp: ev.timestamp,
            data: ev.data,
          ),
        );
      },
    );
  }

  Widget _buildHistoryTab(EventsHistoryState history) {
    if (history.loading && history.items.isEmpty) {
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: const <Widget>[
          LoadingStateView(message: 'Carregando histórico de eventos...'),
        ],
      );
    }
    if (history.error != null && history.items.isEmpty) {
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: <Widget>[
          ErrorStateView(
            message: history.error!,
            onRetry: () =>
                ref.read(eventsHistoryProvider.notifier).fetchPage(page: 1),
          ),
        ],
      );
    }
    if (history.items.isEmpty) {
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: const <Widget>[
          EmptyStateView(title: 'Sem eventos persistidos', icon: Icons.history),
        ],
      );
    }

    final hasNext = history.meta?.hasNext == true;
    return RefreshIndicator(
      onRefresh: () =>
          ref.read(eventsHistoryProvider.notifier).fetchPage(page: 1),
      child: ListView.separated(
        padding: const EdgeInsets.only(top: 10, bottom: 24),
        physics: const AlwaysScrollableScrollPhysics(),
        itemCount: history.items.length + (hasNext ? 1 : 0),
        separatorBuilder: (_, _) => const SizedBox(height: 8),
        itemBuilder: (context, index) {
          if (index >= history.items.length) {
            return Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: OutlinedButton(
                onPressed: history.loading
                    ? null
                    : () => ref
                          .read(eventsHistoryProvider.notifier)
                          .fetchPage(
                            page: history.meta!.currentPage + 1,
                            append: true,
                          ),
                child: const Text('Carregar mais eventos'),
              ),
            );
          }
          final item = history.items[index];
          final p = EventPresentation.describe(item.type, item.data);
          return _EventFeedCard(
            presentation: p,
            title: item.title ?? p.label,
            summary: item.message ?? p.summary,
            timestampLabel: formatShortDateTime(item.timestamp),
            onTap: () => _showEventDetail(
              type: item.type,
              timestamp: item.timestamp,
              data: item.data,
            ),
          );
        },
      ),
    );
  }
}

class _EventFeedCard extends StatelessWidget {
  const _EventFeedCard({
    required this.presentation,
    required this.title,
    required this.summary,
    required this.timestampLabel,
    required this.onTap,
  });

  final EventPresentation presentation;
  final String title;
  final String summary;
  final String timestampLabel;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(presentation.tone, scheme);

    return Card(
      elevation: 1,
      shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: colors.container,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(color: colors.border),
                ),
                child: Icon(
                  presentation.icon,
                  size: 20,
                  color: colors.onContainer,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      summary,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodySmall?.copyWith(
                        color: scheme.onSurfaceVariant,
                        height: 1.35,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 10),
              Column(
                crossAxisAlignment: CrossAxisAlignment.end,
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  SemanticChip(
                    label: presentation.category,
                    tone: presentation.tone,
                    compact: true,
                  ),
                  const SizedBox(height: 6),
                  Text(
                    timestampLabel,
                    style: context.textTheme.labelSmall?.copyWith(
                      fontFamily: 'monospace',
                      fontWeight: FontWeight.w600,
                      color: scheme.onSurfaceVariant,
                    ),
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

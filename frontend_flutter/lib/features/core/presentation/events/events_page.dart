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

  @override
  Widget build(BuildContext context) {
    final realtime = ref.watch(sseServiceProvider);
    final history = ref.watch(eventsHistoryProvider);

    final rawLive = _paused ? _frozenSnapshot : realtime.recentEvents;
    final liveEvents = rawLive
        .where((ev) {
          if (_clearedAfter != null) {
            final ts = DateTime.tryParse(ev.timestamp);
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          title: 'Eventos',
          subtitle: 'Fluxo SSE em tempo real e histórico persistido',
          searchController: _searchCtrl,
          searchHint: 'Filtrar por tipo ou mensagem...',
          onSearchChanged: (_) => setState(() {}),
          actions: <Widget>[
            SemanticChip(
              label: realtime.isConnected ? 'SSE Conectado' : 'Reconectando...',
              tone: realtime.isConnected
                  ? SemanticTone.success
                  : SemanticTone.warning,
              icon: realtime.isConnected
                  ? Icons.radio_button_checked
                  : Icons.sync,
            ),
            OutlinedButton.icon(
              onPressed: () {
                setState(() {
                  if (_paused) {
                    _paused = false;
                  } else {
                    _frozenSnapshot = List<SseEvent>.from(
                      realtime.recentEvents,
                    );
                    _paused = true;
                  }
                });
              },
              icon: Icon(_paused ? Icons.play_arrow : Icons.pause, size: 18),
              label: Text(_paused ? 'Retomar' : 'Pausar'),
            ),
            TextButton.icon(
              onPressed: () => setState(() => _clearedAfter = DateTime.now()),
              icon: const Icon(Icons.clear_all, size: 18),
              label: const Text('Limpar tela'),
            ),
          ],
          filters: <Widget>[
            for (final cat in const <String>[
              'Todos',
              'Alertas',
              'Monitores',
              'Dispositivos',
              'Operação',
              'Sistema',
            ])
              FilterChip(
                selected: _categoryFilter == cat,
                label: Text(cat),
                onSelected: (_) => setState(() => _categoryFilter = cat),
              ),
          ],
        ),
        const SizedBox(height: 10),
        TabBar(
          controller: _tabController,
          isScrollable: true,
          tabAlignment: TabAlignment.start,
          tabs: <Widget>[
            Tab(text: 'Tempo Real (${liveEvents.length})'),
            Tab(
              text:
                  'Histórico (${history.meta?.total ?? history.items.length})',
            ),
          ],
        ),
        const SizedBox(height: 12),
        Expanded(
          child: TabBarView(
            controller: _tabController,
            children: <Widget>[
              liveEvents.isEmpty
                  ? const EmptyStateView(
                      title: 'Nenhum evento em tempo real',
                      description: 'Os eventos do barramento SSE aparecerão aqui instantaneamente.',
                      icon: Icons.bolt_outlined,
                    )
                  : ListView.separated(
                      itemCount: liveEvents.length,
                      separatorBuilder: (_, _) => const SizedBox(height: 8),
                      itemBuilder: (context, index) {
                        final ev = liveEvents[index];
                        final p = EventPresentation.describe(ev.type, ev.data);
                        return Card(
                          elevation: 1,
                          shape: RoundedRectangleBorder(
                            borderRadius: AppTheme.borderRadius,
                          ),
                          child: ListTile(
                            onTap: () => _showEventDetail(
                              type: ev.type,
                              timestamp: ev.timestamp,
                              data: ev.data,
                            ),
                            leading: Icon(
                              p.icon,
                              color: context.semanticColors
                                  .resolve(p.tone, context.colorScheme)
                                  .foreground,
                            ),
                            title: Wrap(
                              spacing: 6,
                              crossAxisAlignment: WrapCrossAlignment.center,
                              children: <Widget>[
                                Text(
                                  p.label,
                                  style: const TextStyle(
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                                SemanticChip(
                                  label: p.category,
                                  tone: p.tone,
                                  compact: true,
                                ),
                              ],
                            ),
                            subtitle: Text(
                              p.summary,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                            ),
                            trailing: Text(
                              formatClockTime(ev.timestamp),
                              style: context.textTheme.labelSmall,
                            ),
                          ),
                        );
                      },
                    ),
              _buildHistoryTab(history),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildHistoryTab(EventsHistoryState history) {
    if (history.loading && history.items.isEmpty) {
      return const LoadingStateView(
        message: 'Carregando histórico de eventos...',
      );
    }
    if (history.error != null && history.items.isEmpty) {
      return ErrorStateView(
        message: history.error!,
        onRetry: () =>
            ref.read(eventsHistoryProvider.notifier).fetchPage(page: 1),
      );
    }
    if (history.items.isEmpty) {
      return const EmptyStateView(
        title: 'Sem eventos persistidos',
        icon: Icons.history,
      );
    }

    return RefreshIndicator(
      onRefresh: () =>
          ref.read(eventsHistoryProvider.notifier).fetchPage(page: 1),
      child: ListView(
        children: <Widget>[
          for (final item in history.items) ...<Widget>[
            Builder(
              builder: (context) {
                final p = EventPresentation.describe(item.type, item.data);
                return ListTile(
                  onTap: () => _showEventDetail(
                    type: item.type,
                    timestamp: item.timestamp,
                    data: item.data,
                  ),
                  leading: Icon(
                    p.icon,
                    color: context.semanticColors
                        .resolve(p.tone, context.colorScheme)
                        .foreground,
                  ),
                  title: Text(
                    item.title ?? p.label,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  subtitle: Text(
                    item.message ?? p.summary,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Text(
                    formatShortDateTime(item.timestamp),
                    style: context.textTheme.labelSmall,
                  ),
                );
              },
            ),
            const Divider(height: 1),
          ],
          if (history.meta?.hasNext == true)
            Padding(
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
            ),
        ],
      ),
    );
  }
}

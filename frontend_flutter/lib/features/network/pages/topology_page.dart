import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const double _canvasWidth = 2200;
const double _canvasHeight = 1500;
const double _nodeWidth = 176;
const double _nodeHeight = 82;

class TopologyPage extends ConsumerStatefulWidget {
  const TopologyPage({super.key});

  @override
  ConsumerState<TopologyPage> createState() => _TopologyPageState();
}

class _TopologyPageState extends ConsumerState<TopologyPage> {
  final TransformationController _transformCtrl = TransformationController();
  final TextEditingController _searchCtrl = TextEditingController();

  String _statusFilter = 'all';
  String _typeFilter = 'all';
  int? _selectedNodeId;
  int? _selectedEdgeId;
  bool _pathFinderMode = false;
  int? _pathStartId;
  QuickPingReport? _quickPingOverlay;
  String? _quickPingTargetName;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() async {
      await Future.wait(<Future<void>>[
        ref.read(sitesProvider.notifier).fetchSites(),
        ref.read(topologyProvider.notifier).fetchTopology(),
      ]);
      if (mounted) _fitToScreen();
    });
  }

  @override
  void dispose() {
    _transformCtrl.dispose();
    _searchCtrl.dispose();
    super.dispose();
  }

  void _zoomBy(double factor) {
    final current = _transformCtrl.value.clone();
    final nextScale = (current.getMaxScaleOnAxis() * factor).clamp(0.3, 2.5);
    final ratio = nextScale / current.getMaxScaleOnAxis();
    current.scaleByDouble(ratio, ratio, 1.0, 1.0);
    _transformCtrl.value = current;
  }

  void _fitToScreen() {
    final positions = ref.read(topologyProvider).positions;
    if (positions.isEmpty) {
      _transformCtrl.value = Matrix4.identity();
      return;
    }
    var minX = double.infinity;
    var minY = double.infinity;
    var maxX = double.negativeInfinity;
    var maxY = double.negativeInfinity;
    for (final p in positions.values) {
      if (p.x < minX) minX = p.x;
      if (p.y < minY) minY = p.y;
      if (p.x + _nodeWidth > maxX) maxX = p.x + _nodeWidth;
      if (p.y + _nodeHeight > maxY) maxY = p.y + _nodeHeight;
    }
    final size = MediaQuery.sizeOf(context);
    final graphW = math.max(1.0, maxX - minX + 120);
    final graphH = math.max(1.0, maxY - minY + 120);
    final scale = math
        .min((size.width - 40) / graphW, (size.height - 160) / graphH)
        .clamp(0.35, 1.1);
    final tx = -(minX - 40) * scale;
    final ty = -(minY - 40) * scale;
    _transformCtrl.value = Matrix4.identity()
      ..translateByDouble(tx, ty, 0, 1)
      ..scaleByDouble(scale, scale, 1, 1);
  }

  void _jumpToNode(TopologyNodeModel node) {
    final pos = ref.read(topologyProvider).positions[node.id];
    if (pos == null) return;
    final size = MediaQuery.sizeOf(context);
    final tx = size.width / 2 - (pos.x + _nodeWidth / 2);
    final ty = (size.height - 120) / 2 - (pos.y + _nodeHeight / 2);
    _transformCtrl.value = Matrix4.identity()..translateByDouble(tx, ty, 0, 1);
    setState(() {
      _selectedNodeId = node.id;
      _selectedEdgeId = null;
    });
  }

  Future<void> _handleNodeTap(TopologyNodeModel node) async {
    if (_pathFinderMode) {
      if (_pathStartId == null) {
        setState(() => _pathStartId = node.id);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              'Origem: ${node.name}. Toque no equipamento de destino.',
            ),
          ),
        );
      } else {
        await ref
            .read(topologyProvider.notifier)
            .fetchPath(_pathStartId!, node.id);
        setState(() {
          _pathFinderMode = false;
          _pathStartId = null;
        });
      }
      return;
    }

    setState(() {
      _selectedNodeId = node.id;
      _selectedEdgeId = null;
    });
    if (MediaQuery.sizeOf(context).width < 960) {
      _openNodeInspectorSheet(node);
    }
  }

  void _openNodeInspectorSheet(TopologyNodeModel node) {
    showAdaptiveFormSheet<void>(
      context: context,
      title: node.name,
      subtitle:
          '${deviceTypeLabel(node.type)}${node.ipAddress != null ? ' · ${node.ipAddress}' : ''}',
      builder: (_) => _NodeInspectorContent(
        node: node,
        onQuickPing: () => _runQuickPing(node),
        onStartLinkTest: () {
          ref.read(pendingLinkTestDraftProvider.notifier).state =
              LinkTestRequestDraft(kind: 'server', targetDeviceId: node.id);
          context.go('/link-tests?deviceId=${node.id}');
        },
      ),
    );
  }

  void _openEdgeInspectorSheet(TopologyEdgeModel edge) {
    final canWrite = ref.read(authProvider).canWrite;
    showAdaptiveFormSheet<void>(
      context: context,
      title: 'Enlace #${edge.id}',
      subtitle:
          '${edge.sourceDeviceName ?? '#${edge.sourceDeviceId}'} ↔ ${edge.targetDeviceName ?? '#${edge.targetDeviceId}'}',
      builder: (ctx) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          KeyValueRow(label: 'Status', value: edge.status.toUpperCase()),
          KeyValueRow(label: 'Tipo', value: edge.linkType),
          KeyValueRow(label: 'Descoberta', value: edge.discoveryMethod),
          KeyValueRow(
            label: 'Velocidade',
            value: formatLinkSpeed(edge.speedBps),
          ),
          KeyValueRow(
            label: 'Tráfego',
            value: edge.formattedTraffic.isEmpty ? '—' : edge.formattedTraffic,
          ),
          if (edge.utilization != null)
            KeyValueRow(
              label: 'Utilização',
              value: formatPercent(edge.utilization! * 100),
            ),
          if (edge.sourceInterfaceName != null)
            KeyValueRow(
              label: 'Porta origem',
              value: edge.sourceInterfaceName!,
            ),
          if (edge.targetInterfaceName != null)
            KeyValueRow(
              label: 'Porta destino',
              value: edge.targetInterfaceName!,
            ),
          const SizedBox(height: 14),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              if (!edge.isVirtualHierarchy)
                FilledButton.tonalIcon(
                  onPressed: () {
                    Navigator.of(ctx).pop();
                    ref.read(pendingLinkTestDraftProvider.notifier).state =
                        LinkTestRequestDraft(kind: 'link', linkId: edge.id);
                    context.go('/link-tests');
                  },
                  icon: const Icon(Icons.speed, size: 18),
                  label: const Text('Testar enlace'),
                ),
              if (canWrite && !edge.isVirtualHierarchy && !edge.confirmed)
                OutlinedButton.icon(
                  onPressed: () async {
                    await ref.read(topologyProvider.notifier).updateLink(
                      edge.id,
                      <String, dynamic>{
                        'sourceInterfaceId': edge.sourceInterfaceId,
                        'targetInterfaceId': edge.targetInterfaceId,
                        'linkType': edge.linkType,
                      },
                    );
                    if (ctx.mounted) Navigator.of(ctx).pop();
                  },
                  icon: const Icon(Icons.verified_outlined, size: 18),
                  label: const Text('Confirmar enlace'),
                ),
              if (canWrite && !edge.isVirtualHierarchy)
                TextButton.icon(
                  onPressed: () async {
                    final ok = await showConfirmActionDialog(
                      context: context,
                      title: 'Remover enlace',
                      message: 'Remover este enlace da topologia?',
                      confirmLabel: 'Remover',
                    );
                    if (ok) {
                      await ref
                          .read(topologyProvider.notifier)
                          .deleteLink(edge.id);
                      if (ctx.mounted) Navigator.of(ctx).pop();
                    }
                  },
                  icon: Icon(
                    Icons.delete_outline,
                    color: context.colorScheme.error,
                    size: 18,
                  ),
                  label: Text(
                    'Excluir',
                    style: TextStyle(color: context.colorScheme.error),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _runQuickPing(TopologyNodeModel node) async {
    try {
      final report = await ref
          .read(diagnosticsServiceProvider)
          .quickPing(node.id);
      if (mounted) {
        setState(() {
          _quickPingOverlay = report;
          _quickPingTargetName = node.name;
        });
      }
    } catch (err) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(err.toString())));
      }
    }
  }

  Future<void> _openAddLinkSheet() async {
    final nodes = ref.read(topologyProvider).nodes;
    int? srcId = nodes.firstOrNull?.id;
    int? tgtId = nodes.length > 1 ? nodes[1].id : null;
    String linkType = 'ethernet';

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Adicionar Enlace Manual',
      builder: (ctx) => StatefulBuilder(
        builder: (context, setModal) => Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            DropdownButtonFormField<int?>(
              initialValue: srcId,
              decoration: const InputDecoration(labelText: 'Equipamento A'),
              items: nodes
                  .map(
                    (n) => DropdownMenuItem<int?>(
                      value: n.id,
                      child: Text(n.name),
                    ),
                  )
                  .toList(growable: false),
              onChanged: (v) => setModal(() => srcId = v),
            ),
            const SizedBox(height: 10),
            DropdownButtonFormField<int?>(
              initialValue: tgtId,
              decoration: const InputDecoration(labelText: 'Equipamento B'),
              items: nodes
                  .map(
                    (n) => DropdownMenuItem<int?>(
                      value: n.id,
                      child: Text(n.name),
                    ),
                  )
                  .toList(growable: false),
              onChanged: (v) => setModal(() => tgtId = v),
            ),
            const SizedBox(height: 10),
            DropdownButtonFormField<String>(
              initialValue: linkType,
              decoration: const InputDecoration(labelText: 'Tipo de enlace'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(value: 'ethernet', child: Text('Ethernet')),
                DropdownMenuItem(value: 'fiber', child: Text('Fibra óptica')),
                DropdownMenuItem(
                  value: 'wireless',
                  child: Text('Wireless / Rádio'),
                ),
                DropdownMenuItem(value: 'vpn', child: Text('Túnel VPN')),
              ],
              onChanged: (v) => setModal(() => linkType = v ?? 'ethernet'),
            ),
            const SizedBox(height: 14),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
                onPressed: srcId == null || tgtId == null || srcId == tgtId
                    ? null
                    : () async {
                        final ok = await ref
                            .read(topologyProvider.notifier)
                            .addLink(<String, dynamic>{
                              'sourceDeviceId': srcId,
                              'targetDeviceId': tgtId,
                              'linkType': linkType,
                            });
                        if (ctx.mounted && ok) Navigator.of(ctx).pop();
                      },
                icon: const Icon(Icons.add_link),
                label: const Text('Criar enlace'),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _openUnmanagedSwitchSheet() async {
    final nameCtrl = TextEditingController();
    final vendorCtrl = TextEditingController();
    final portsCtrl = TextEditingController(text: '8');
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Novo Switch Não Gerenciável',
      builder: (ctx) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextField(
            controller: nameCtrl,
            decoration: const InputDecoration(labelText: 'Nome do switch *'),
          ),
          const SizedBox(height: 10),
          Row(
            children: <Widget>[
              Expanded(
                child: TextField(
                  controller: vendorCtrl,
                  decoration: const InputDecoration(labelText: 'Fabricante'),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: TextField(
                  controller: portsCtrl,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(labelText: 'Portas'),
                ),
              ),
            ],
          ),
          const SizedBox(height: 14),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: () async {
                if (nameCtrl.text.trim().isEmpty) return;
                final ok = await ref
                    .read(topologyProvider.notifier)
                    .createUnmanagedSwitch(<String, dynamic>{
                      'name': nameCtrl.text.trim(),
                      if (vendorCtrl.text.trim().isNotEmpty)
                        'vendor': vendorCtrl.text.trim(),
                      'portCount': int.tryParse(portsCtrl.text.trim()) ?? 8,
                      'siteId': ref.read(topologyProvider).siteId,
                    });
                if (ctx.mounted && ok) Navigator.of(ctx).pop();
              },
              icon: const Icon(Icons.hub_outlined),
              label: const Text('Adicionar switch'),
            ),
          ),
        ],
      ),
    );
    nameCtrl.dispose();
    vendorCtrl.dispose();
    portsCtrl.dispose();
  }

  void _openFilterAndViewSheet() {
    showAdaptiveFormSheet<void>(
      context: context,
      title: 'Filtros e Camadas da Topologia',
      builder: (_) => Consumer(
        builder: (context, ref, _) {
          final prefs = ref.watch(topologyProvider).viewPrefs;
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              DropdownButtonFormField<String>(
                initialValue: _statusFilter,
                decoration: const InputDecoration(labelText: 'Status dos nós'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: 'all', child: Text('Todos')),
                  DropdownMenuItem(value: 'online', child: Text('Online')),
                  DropdownMenuItem(value: 'offline', child: Text('Offline')),
                  DropdownMenuItem(value: 'warning', child: Text('Atenção')),
                ],
                onChanged: (v) => setState(() => _statusFilter = v ?? 'all'),
              ),
              const SizedBox(height: 10),
              DropdownButtonFormField<String>(
                initialValue: _typeFilter,
                decoration: const InputDecoration(
                  labelText: 'Tipo de equipamento',
                ),
                items: <DropdownMenuItem<String>>[
                  const DropdownMenuItem(value: 'all', child: Text('Todos')),
                  for (final opt in deviceTypeOptions)
                    DropdownMenuItem(value: opt.$1, child: Text(opt.$2)),
                ],
                onChanged: (v) => setState(() => _typeFilter = v ?? 'all'),
              ),
              const Divider(height: 24),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Rótulos de tráfego nos enlaces'),
                value: prefs.showTrafficLabels,
                onChanged: (v) => ref
                    .read(topologyProvider.notifier)
                    .setViewPrefs(prefs.copyWith(showTrafficLabels: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Colorir enlaces por utilização'),
                value: prefs.colorByUtilization,
                onChanged: (v) => ref
                    .read(topologyProvider.notifier)
                    .setViewPrefs(prefs.copyWith(colorByUtilization: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Badges ao vivo de CPU/RAM nos nós'),
                value: prefs.showHealthBadges,
                onChanged: (v) => ref
                    .read(topologyProvider.notifier)
                    .setViewPrefs(prefs.copyWith(showHealthBadges: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Mostrar endereço IP'),
                value: prefs.showIpLabels,
                onChanged: (v) => ref
                    .read(topologyProvider.notifier)
                    .setViewPrefs(prefs.copyWith(showIpLabels: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Exibir nós isolados'),
                value: prefs.showIsolated,
                onChanged: (v) => ref
                    .read(topologyProvider.notifier)
                    .setViewPrefs(prefs.copyWith(showIsolated: v)),
              ),
            ],
          );
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final topo = ref.watch(topologyProvider);
    final sites = ref.watch(sitesProvider).items;
    final canWrite = ref.watch(authProvider).canWrite;
    final isWide = MediaQuery.sizeOf(context).width >= 960;

    final connectedNodeIds = <int>{
      for (final e in topo.edges) ...<int>[e.sourceDeviceId, e.targetDeviceId],
    };

    final visibleNodes = topo.nodes
        .where((n) {
          if (_statusFilter != 'all' && n.status != _statusFilter) return false;
          if (_typeFilter != 'all' && n.type != _typeFilter) return false;
          if (!topo.viewPrefs.showIsolated &&
              !connectedNodeIds.contains(n.id)) {
            return false;
          }
          return true;
        })
        .toList(growable: false);

    final visibleIds = visibleNodes.map((n) => n.id).toSet();
    final visibleEdges = topo.edges
        .where(
          (e) =>
              visibleIds.contains(e.sourceDeviceId) &&
              visibleIds.contains(e.targetDeviceId),
        )
        .toList(growable: false);

    final selectedNode = _selectedNodeId != null
        ? topo.nodes.where((n) => n.id == _selectedNodeId).firstOrNull
        : null;

    return Stack(
      children: <Widget>[
        // ── Canvas Interativo com Pinch-Zoom + Pan + CustomPainter + Draggable Material 3 Node Cards ──
        if (topo.loading && topo.nodes.isEmpty)
          const LoadingStateView(message: 'Carregando mapa de topologia...')
        else if (topo.error != null && topo.nodes.isEmpty)
          ErrorStateView(
            message: topo.error!,
            onRetry: () => ref.read(topologyProvider.notifier).fetchTopology(),
          )
        else if (topo.nodes.isEmpty)
          const EmptyStateView(
            title: 'Nenhum dispositivo na topologia',
            description:
                'Cadastre equipamentos ou execute uma descoberta de rede.',
            icon: Icons.account_tree_outlined,
          )
        else
          Positioned.fill(
            child: InteractiveViewer(
              transformationController: _transformCtrl,
              constrained: false,
              boundaryMargin: const EdgeInsets.all(600),
              minScale: 0.3,
              maxScale: 2.5,
              child: SizedBox(
                width: _canvasWidth,
                height: _canvasHeight,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: <Widget>[
                    // Camada de arestas (CustomPainter)
                    Positioned.fill(
                      child: CustomPaint(
                        painter: _TopologyEdgesPainter(
                          edges: visibleEdges,
                          positions: topo.positions,
                          highlightedEdges:
                              (topo.highlightedPath?.edgeIds ?? const <int>[])
                                  .toSet(),
                          selectedEdgeId: _selectedEdgeId,
                          prefs: topo.viewPrefs,
                          scheme: context.colorScheme,
                          semantics: context.semanticColors,
                        ),
                      ),
                    ),
                    // Badges clicáveis de tráfego no ponto médio de cada enlace
                    for (final edge in visibleEdges)
                      if (topo.positions[edge.sourceDeviceId] != null &&
                          topo.positions[edge.targetDeviceId] != null)
                        _buildEdgeMidBadge(context, edge, topo),
                    // Nós Material 3 arrastáveis
                    for (final node in visibleNodes)
                      if (topo.positions[node.id] != null)
                        Positioned(
                          left: topo.positions[node.id]!.x,
                          top: topo.positions[node.id]!.y,
                          child: GestureDetector(
                            onPanUpdate: (details) {
                              final scale = _transformCtrl.value
                                  .getMaxScaleOnAxis();
                              final cur = topo.positions[node.id]!;
                              ref
                                  .read(topologyProvider.notifier)
                                  .moveNode(
                                    node.id,
                                    cur.translate(
                                      details.delta.dx / scale,
                                      details.delta.dy / scale,
                                    ),
                                  );
                            },
                            onPanEnd: (_) {
                              if (canWrite) {
                                ref
                                    .read(topologyProvider.notifier)
                                    .saveLayout(onlyNodeIds: <int>[node.id]);
                              }
                            },
                            child: _TopologyNodeCard(
                              node: node,
                              selected: _selectedNodeId == node.id,
                              highlighted:
                                  topo.highlightedPath?.deviceIds.contains(
                                    node.id,
                                  ) ==
                                  true,
                              prefs: topo.viewPrefs,
                              onTap: () => _handleNodeTap(node),
                            ),
                          ),
                        ),
                  ],
                ),
              ),
            ),
          ),

        // ── Barra superior flutuante (mobile-friendly Wrap) ──
        Positioned(
          top: 12,
          left: 12,
          right: isWide && selectedNode != null ? 364 : 12,
          child: Card(
            elevation: 3,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
              child: Wrap(
                spacing: 8,
                runSpacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  if (sites.isNotEmpty)
                    SizedBox(
                      width: 150,
                      child: DropdownButtonFormField<int?>(
                        initialValue: topo.siteId,
                        isDense: true,
                        decoration: const InputDecoration(
                          contentPadding: EdgeInsets.symmetric(
                            horizontal: 8,
                            vertical: 6,
                          ),
                        ),
                        items: <DropdownMenuItem<int?>>[
                          const DropdownMenuItem<int?>(
                            value: null,
                            child: Text('Todos sites'),
                          ),
                          for (final s in sites)
                            DropdownMenuItem<int?>(
                              value: s.id,
                              child: Text(s.name),
                            ),
                        ],
                        onChanged: (v) => ref
                            .read(topologyProvider.notifier)
                            .fetchTopology(siteId: v, updateSiteFilter: true),
                      ),
                    ),
                  SizedBox(
                    width: 170,
                    child: TextField(
                      controller: _searchCtrl,
                      decoration: const InputDecoration(
                        hintText: 'Localizar nó...',
                        prefixIcon: Icon(Icons.search, size: 18),
                        isDense: true,
                        contentPadding: EdgeInsets.symmetric(
                          horizontal: 8,
                          vertical: 8,
                        ),
                      ),
                      onSubmitted: (q) {
                        final lower = q.trim().toLowerCase();
                        final match = topo.nodes
                            .where(
                              (n) =>
                                  n.name.toLowerCase().contains(lower) ||
                                  (n.ipAddress ?? '').contains(lower),
                            )
                            .firstOrNull;
                        if (match != null) _jumpToNode(match);
                      },
                    ),
                  ),
                  PopupMenuButton<TopologyLayoutMode>(
                    tooltip: 'Layout do grafo',
                    onSelected: (mode) {
                      ref.read(topologyProvider.notifier).applyLayoutMode(mode);
                      _fitToScreen();
                    },
                    itemBuilder: (_) =>
                        const <PopupMenuEntry<TopologyLayoutMode>>[
                          PopupMenuItem(
                            value: TopologyLayoutMode.hierarchical,
                            child: Text('Hierárquico (Árvore)'),
                          ),
                          PopupMenuItem(
                            value: TopologyLayoutMode.force,
                            child: Text('Força (Automático)'),
                          ),
                          PopupMenuItem(
                            value: TopologyLayoutMode.radial,
                            child: Text('Radial'),
                          ),
                          PopupMenuItem(
                            value: TopologyLayoutMode.grid,
                            child: Text('Grade'),
                          ),
                        ],
                    child: Chip(
                      avatar: const Icon(Icons.auto_graph, size: 16),
                      label: Text(topo.layoutMode.name),
                    ),
                  ),
                  IconButton(
                    tooltip: 'Filtros e camadas',
                    icon: const Icon(Icons.tune),
                    onPressed: _openFilterAndViewSheet,
                  ),
                  IconButton(
                    tooltip: _pathFinderMode
                        ? 'Cancelar caminho'
                        : 'Traçar caminho A → B',
                    icon: Icon(
                      Icons.route,
                      color: _pathFinderMode
                          ? context.colorScheme.primary
                          : null,
                    ),
                    onPressed: () {
                      setState(() {
                        _pathFinderMode = !_pathFinderMode;
                        _pathStartId = null;
                      });
                      if (!_pathFinderMode) {
                        ref
                            .read(topologyProvider.notifier)
                            .clearHighlightedPath();
                      }
                    },
                  ),
                  if (canWrite) ...<Widget>[
                    IconButton(
                      tooltip: 'Adicionar enlace',
                      icon: const Icon(Icons.add_link),
                      onPressed: _openAddLinkSheet,
                    ),
                    IconButton(
                      tooltip: 'Switch não gerenciável',
                      icon: const Icon(Icons.hub_outlined),
                      onPressed: _openUnmanagedSwitchSheet,
                    ),
                    IconButton(
                      tooltip: 'Recalcular LLDP/CDP/ARP',
                      icon: topo.recalculating
                          ? const SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.sync),
                      onPressed: topo.recalculating
                          ? null
                          : () => ref
                                .read(topologyProvider.notifier)
                                .recalculateTopology(),
                    ),
                    IconButton(
                      tooltip: 'Salvar layout',
                      icon: const Icon(Icons.save_outlined),
                      onPressed: topo.savingLayout
                          ? null
                          : () async {
                              final ok = await ref
                                  .read(topologyProvider.notifier)
                                  .saveLayout();
                              if (ok && context.mounted) {
                                ScaffoldMessenger.of(context).showSnackBar(
                                  const SnackBar(
                                    content: Text(
                                      'Posições da topologia salvas.',
                                    ),
                                  ),
                                );
                              }
                            },
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),

        // ── Controles de Zoom flutuantes (canto inferior esquerdo) ──
        Positioned(
          left: 12,
          bottom: 12,
          child: Card(
            elevation: 3,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                IconButton(
                  tooltip: 'Aumentar zoom',
                  icon: const Icon(Icons.zoom_in),
                  onPressed: () => _zoomBy(1.2),
                ),
                IconButton(
                  tooltip: 'Diminuir zoom',
                  icon: const Icon(Icons.zoom_out),
                  onPressed: () => _zoomBy(0.8),
                ),
                IconButton(
                  tooltip: 'Ajustar à tela',
                  icon: const Icon(Icons.fit_screen),
                  onPressed: _fitToScreen,
                ),
              ],
            ),
          ),
        ),

        // ── Overlay flutuante de Quick Ping ──
        if (_quickPingOverlay != null)
          Positioned(
            right: isWide && selectedNode != null ? 364 : 12,
            bottom: 12,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 320),
              child: Card(
                elevation: 4,
                child: Padding(
                  padding: const EdgeInsets.all(12),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          const Icon(Icons.network_ping, size: 18),
                          const SizedBox(width: 6),
                          Expanded(
                            child: Text(
                              'Quick Ping · $_quickPingTargetName',
                              style: context.textTheme.titleSmall?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                          ),
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            icon: const Icon(Icons.close, size: 16),
                            onPressed: () =>
                                setState(() => _quickPingOverlay = null),
                          ),
                        ],
                      ),
                      Text(
                        _quickPingOverlay!.verdict,
                        style: context.textTheme.bodySmall,
                      ),
                      const SizedBox(height: 4),
                      Text(
                        'RTT: ${formatLatency(_quickPingOverlay!.small?.avgMs)} · Perda: ${formatPercent(_quickPingOverlay!.lossPct)}',
                        style: context.textTheme.labelMedium?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),

        // ── Painel lateral do Inspetor em desktop (>= 960px) ──
        if (isWide && selectedNode != null)
          Positioned(
            top: 12,
            right: 12,
            bottom: 12,
            width: 340,
            child: Card(
              elevation: 4,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  ListTile(
                    title: Text(
                      selectedNode.name,
                      style: const TextStyle(fontWeight: FontWeight.w800),
                    ),
                    subtitle: Text(
                      selectedNode.ipAddress ??
                          deviceTypeLabel(selectedNode.type),
                    ),
                    trailing: IconButton(
                      icon: const Icon(Icons.close),
                      onPressed: () => setState(() => _selectedNodeId = null),
                    ),
                  ),
                  const Divider(height: 1),
                  Expanded(
                    child: SingleChildScrollView(
                      padding: const EdgeInsets.all(14),
                      child: _NodeInspectorContent(
                        node: selectedNode,
                        onQuickPing: () => _runQuickPing(selectedNode),
                        onStartLinkTest: () {
                          ref
                              .read(pendingLinkTestDraftProvider.notifier)
                              .state = LinkTestRequestDraft(
                            kind: 'server',
                            targetDeviceId: selectedNode.id,
                          );
                          context.go('/link-tests?deviceId=${selectedNode.id}');
                        },
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
      ],
    );
  }

  Widget _buildEdgeMidBadge(
    BuildContext context,
    TopologyEdgeModel edge,
    TopologyState topo,
  ) {
    final p1 = topo.positions[edge.sourceDeviceId]!;
    final p2 = topo.positions[edge.targetDeviceId]!;
    final mx = (p1.x + p2.x) / 2 + _nodeWidth / 2 - 42;
    final my = (p1.y + p2.y) / 2 + _nodeHeight / 2 - 12;
    final label =
        topo.viewPrefs.showTrafficLabels && edge.formattedTraffic.isNotEmpty
        ? edge.formattedTraffic
        : formatLinkSpeed(edge.speedBps);

    return Positioned(
      left: mx,
      top: my,
      child: InkWell(
        onTap: () {
          setState(() {
            _selectedEdgeId = edge.id;
            _selectedNodeId = null;
          });
          _openEdgeInspectorSheet(edge);
        },
        borderRadius: BorderRadius.circular(8),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
          decoration: BoxDecoration(
            color: context.colorScheme.surface,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(
              color: edge.status == 'down'
                  ? context.semanticColors.error
                  : context.colorScheme.outlineVariant,
            ),
          ),
          child: Text(
            label,
            style: context.textTheme.labelSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
        ),
      ),
    );
  }
}

class _TopologyNodeCard extends StatelessWidget {
  const _TopologyNodeCard({
    required this.node,
    required this.selected,
    required this.highlighted,
    required this.prefs,
    required this.onTap,
  });

  final TopologyNodeModel node;
  final bool selected;
  final bool highlighted;
  final TopologyViewPrefs prefs;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final sem = context.semanticColors;
    final borderColor = selected
        ? scheme.primary
        : highlighted
        ? sem.warning
        : node.status == 'offline'
        ? sem.error
        : scheme.outlineVariant;

    return SizedBox(
      width: _nodeWidth,
      height: _nodeHeight,
      child: Material(
        elevation: selected ? 6 : 2,
        color: scheme.surface,
        shape: RoundedRectangleBorder(
          borderRadius: AppTheme.borderRadius,
          side: BorderSide(
            color: borderColor,
            width: selected || highlighted ? 2.5 : 1.2,
          ),
        ),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(
                      deviceTypeIcon(node.type),
                      size: 18,
                      color: scheme.primary,
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        node.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelLarge?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    Container(
                      width: 9,
                      height: 9,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: node.status == 'online'
                            ? sem.success
                            : node.status == 'offline'
                            ? sem.error
                            : sem.warning,
                      ),
                    ),
                  ],
                ),
                if (prefs.showIpLabels && node.ipAddress != null)
                  Text(
                    node.ipAddress!,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.labelSmall?.copyWith(
                      fontFamily: 'monospace',
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                if (prefs.showHealthBadges &&
                    (node.cpuPercent != null || node.memoryPercent != null))
                  Row(
                    children: <Widget>[
                      if (node.cpuPercent != null)
                        Text(
                          'CPU ${node.cpuPercent!.round()}%',
                          style: context.textTheme.labelSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                            color: node.cpuPercent! > 85 ? sem.error : sem.info,
                          ),
                        ),
                      if (node.cpuPercent != null && node.memoryPercent != null)
                        const SizedBox(width: 8),
                      if (node.memoryPercent != null)
                        Text(
                          'RAM ${node.memoryPercent!.round()}%',
                          style: context.textTheme.labelSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                            color: node.memoryPercent! > 85
                                ? sem.warning
                                : scheme.secondary,
                          ),
                        ),
                    ],
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _NodeInspectorContent extends ConsumerWidget {
  const _NodeInspectorContent({
    required this.node,
    required this.onQuickPing,
    required this.onStartLinkTest,
  });

  final TopologyNodeModel node;
  final VoidCallback onQuickPing;
  final VoidCallback onStartLinkTest;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final pluginActions =
        ref.watch(topologyProvider).pluginActions[node.id] ??
        const <TopologyPluginActionModel>[];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        KeyValueRow(label: 'Status', value: node.status.toUpperCase()),
        KeyValueRow(label: 'Tipo', value: deviceTypeLabel(node.type)),
        KeyValueRow(label: 'IP', value: node.ipAddress ?? '—', monospace: true),
        KeyValueRow(label: 'Site', value: node.siteName ?? '—'),
        KeyValueRow(label: 'Interfaces', value: '${node.interfaceCount}'),
        if (node.cpuPercent != null)
          KeyValueRow(label: 'CPU', value: formatPercent(node.cpuPercent)),
        if (node.memoryPercent != null)
          KeyValueRow(
            label: 'Memória',
            value: formatPercent(node.memoryPercent),
          ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            FilledButton.icon(
              onPressed: () => context.go('/devices/${node.id}'),
              icon: const Icon(Icons.open_in_new, size: 16),
              label: const Text('Abrir dispositivo'),
            ),
            if (!node.isSystem && node.ipAddress != null)
              FilledButton.tonalIcon(
                onPressed: onQuickPing,
                icon: const Icon(Icons.network_ping, size: 16),
                label: const Text('Quick Ping'),
              ),
            OutlinedButton.icon(
              onPressed: onStartLinkTest,
              icon: const Icon(Icons.speed, size: 16),
              label: const Text('Testar velocidade'),
            ),
          ],
        ),
        if (pluginActions.isNotEmpty) ...<Widget>[
          const Divider(height: 24),
          Text(
            'Ações de Plugins',
            style: context.textTheme.labelLarge?.copyWith(
              color: context.semanticColors.plugin,
              fontWeight: FontWeight.w800,
            ),
          ),
          const SizedBox(height: 6),
          for (final act in pluginActions)
            ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(
                Icons.extension_outlined,
                color: context.semanticColors.plugin,
                size: 18,
              ),
              title: Text(act.label),
              subtitle: Text(act.pluginName),
              onTap: () => context.go('/devices/${node.id}?tab=plugins'),
            ),
        ],
      ],
    );
  }
}

class _TopologyEdgesPainter extends CustomPainter {
  const _TopologyEdgesPainter({
    required this.edges,
    required this.positions,
    required this.highlightedEdges,
    required this.selectedEdgeId,
    required this.prefs,
    required this.scheme,
    required this.semantics,
  });

  final List<TopologyEdgeModel> edges;
  final Map<int, TopologyPosition> positions;
  final Set<int> highlightedEdges;
  final int? selectedEdgeId;
  final TopologyViewPrefs prefs;
  final ColorScheme scheme;
  final AppSemanticColors semantics;

  @override
  void paint(Canvas canvas, Size size) {
    for (final edge in edges) {
      final p1 = positions[edge.sourceDeviceId];
      final p2 = positions[edge.targetDeviceId];
      if (p1 == null || p2 == null) continue;

      final start = Offset(p1.x + _nodeWidth / 2, p1.y + _nodeHeight / 2);
      final end = Offset(p2.x + _nodeWidth / 2, p2.y + _nodeHeight / 2);

      Color color = scheme.outline;
      if (highlightedEdges.contains(edge.id) || edge.id == selectedEdgeId) {
        color = semantics.warning;
      } else if (edge.status == 'down') {
        color = semantics.error;
      } else if (prefs.colorByUtilization && edge.utilization != null) {
        if (edge.utilization! >= 0.8) {
          color = semantics.error;
        } else if (edge.utilization! >= 0.5) {
          color = semantics.warning;
        } else {
          color = semantics.success;
        }
      } else if (edge.status == 'up') {
        color = semantics.info;
      }

      final paint = Paint()
        ..color = color
        ..strokeWidth =
            highlightedEdges.contains(edge.id) || edge.id == selectedEdgeId
            ? 4.0
            : 2.2
        ..style = PaintingStyle.stroke;

      canvas.drawLine(start, end, paint);
    }
  }

  @override
  bool shouldRepaint(covariant _TopologyEdgesPainter oldDelegate) =>
      oldDelegate.edges != edges ||
      oldDelegate.positions != positions ||
      oldDelegate.highlightedEdges != highlightedEdges ||
      oldDelegate.selectedEdgeId != selectedEdgeId ||
      oldDelegate.prefs != prefs;
}

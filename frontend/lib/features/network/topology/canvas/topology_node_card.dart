import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Limite de escala abaixo do qual os nós entram em modo macro (LOD circular).
const double kTopologyMacroZoomThreshold = 0.55;

/// Metadados ao vivo complementares por nó para micro-badges no cartão.
@immutable
class TopologyNodeExtras {
  const TopologyNodeExtras({
    this.latencyMs,
    this.alertCount = 0,
    this.wifiClientsCount = 0,
    this.hasPluginActions = false,
  });

  final double? latencyMs;
  final int alertCount;
  final int wifiClientsCount;
  final bool hasPluginActions;

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is TopologyNodeExtras &&
        other.latencyMs == latencyMs &&
        other.alertCount == alertCount &&
        other.wifiClientsCount == wifiClientsCount &&
        other.hasPluginActions == hasPluginActions;
  }

  @override
  int get hashCode =>
      Object.hash(latencyMs, alertCount, wifiClientsCount, hasPluginActions);
}

/// Cartão de nó interativo Material Design 3 com Zoom Semântico (LOD):
/// - `scale < 0.55`: nó circular compacto de alto contraste + rótulo abaixo.
/// - `scale >= 0.55`: cartão detalhado (`12px` radius) com barra semântica,
///   IP em monoespaçada, micro-badges de telemetria, switch não gerenciável e
///   suporte a arraste 1:1 quando `editLayoutEnabled` estiver ativo.
class TopologyNodeCard extends StatelessWidget {
  const TopologyNodeCard({
    super.key,
    required this.node,
    this.size = kDefaultTopologyNodeSize,
    this.scale = 1.0,
    this.selected = false,
    this.highlighted = false,
    this.dimmed = false,
    this.isPathStart = false,
    this.showIpLabel = true,
    this.showHealthBadges = true,
    this.editLayoutEnabled = false,
    this.overlayMode = TopologyOverlayMode.status,
    this.extras,
    this.onTap,
    this.onDoubleTap,
    this.onLongPress,
    this.onDragStart,
    this.onDragDelta,
    this.onDragEnd,
  });

  final TopologyNodeModel node;
  final Size size;
  final double scale;
  final bool selected;
  final bool highlighted;
  final bool dimmed;
  final bool isPathStart;
  final bool showIpLabel;
  final bool showHealthBadges;
  final bool editLayoutEnabled;
  final TopologyOverlayMode overlayMode;
  final TopologyNodeExtras? extras;
  final VoidCallback? onTap;
  final VoidCallback? onDoubleTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onDragStart;
  final ValueChanged<Offset>? onDragDelta;
  final VoidCallback? onDragEnd;

  bool get _isMacroLod => scale < kTopologyMacroZoomThreshold;

  int get _effectiveAlertCount {
    final extraAlerts = extras?.alertCount ?? 0;
    return extraAlerts > 0 ? extraAlerts : node.monitorsDown;
  }

  SemanticTone _resolveAccentTone() {
    final status = classifyNodeStatus(node.status);
    if (status == TopologyNodeStatus.offline) return SemanticTone.error;

    if (overlayMode == TopologyOverlayMode.health) {
      final maxPct = _maxHealthPercent();
      if (maxPct != null) return classifyHealthPercent(maxPct).tone;
    }
    if (overlayMode == TopologyOverlayMode.latency &&
        extras?.latencyMs != null) {
      return latencyTone(extras!.latencyMs);
    }
    if (_effectiveAlertCount > 0) return SemanticTone.warning;
    return status.tone;
  }

  double? _maxHealthPercent() {
    final cpu = node.cpuPercent;
    final mem = node.memoryPercent;
    if (cpu == null && mem == null) return null;
    if (cpu == null) return mem;
    if (mem == null) return cpu;
    return cpu >= mem ? cpu : mem;
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;
    final accentPair = semantics.resolve(_resolveAccentTone(), scheme);
    final statusPair = semantics.resolve(
      classifyNodeStatus(node.status).tone,
      scheme,
    );
    final safeScale = scale.clamp(0.15, 4.0);

    final content = _isMacroLod
        ? _buildMacroNode(context, scheme, semantics, accentPair, statusPair)
        : _buildDetailCard(context, scheme, semantics, accentPair, statusPair);

    return AnimatedOpacity(
      duration: const Duration(milliseconds: 180),
      opacity: dimmed ? 0.26 : 1.0,
      child: SizedBox(
        width: size.width,
        height: size.height,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: onTap,
          onDoubleTap: onDoubleTap,
          onLongPress: onLongPress,
          onPanStart: editLayoutEnabled ? (_) => onDragStart?.call() : null,
          onPanUpdate: editLayoutEnabled
              ? (details) => onDragDelta?.call(
                  Offset(
                    details.delta.dx / safeScale,
                    details.delta.dy / safeScale,
                  ),
                )
              : null,
          onPanEnd: editLayoutEnabled ? (_) => onDragEnd?.call() : null,
          child: Semantics(
            button: true,
            selected: selected,
            label:
                '${node.name}, ${topologyNodeTypeLabel(node)}, ${classifyNodeStatus(node.status).label}',
            child: content,
          ),
        ),
      ),
    );
  }

  Widget _buildMacroNode(
    BuildContext context,
    ColorScheme scheme,
    AppSemanticColors semantics,
    SemanticColorPair accentPair,
    SemanticColorPair statusPair,
  ) {
    final ringColor = selected
        ? scheme.primary
        : isPathStart
        ? semantics.info
        : highlighted
        ? semantics.warning
        : accentPair.foreground;
    final alerts = _effectiveAlertCount;

    return Center(
      key: ValueKey<String>('topology-node-macro-${node.id}'),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Stack(
            clipBehavior: Clip.none,
            children: <Widget>[
              Container(
                width: 54,
                height: 54,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: scheme.surface,
                  border: Border.all(
                    color: ringColor,
                    width: selected || highlighted ? 3.5 : 2.6,
                  ),
                  boxShadow: <BoxShadow>[
                    BoxShadow(
                      color:
                          (selected || highlighted ? ringColor : scheme.shadow)
                              .withValues(
                                alpha: selected || highlighted ? 0.34 : 0.2,
                              ),
                      blurRadius: selected || highlighted ? 12 : 6,
                      offset: const Offset(0, 2),
                    ),
                  ],
                ),
                child: Center(
                  child: Icon(
                    topologyNodeIcon(node),
                    size: 26,
                    color: accentPair.foreground,
                  ),
                ),
              ),
              Positioned(
                right: -1,
                bottom: -1,
                child: Container(
                  width: 14,
                  height: 14,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: statusPair.foreground,
                    border: Border.all(color: scheme.surface, width: 2),
                  ),
                ),
              ),
              if (alerts > 0)
                Positioned(
                  right: -4,
                  top: -4,
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 5,
                      vertical: 1,
                    ),
                    decoration: BoxDecoration(
                      color: semantics.error,
                      borderRadius: BorderRadius.circular(10),
                      border: Border.all(color: scheme.surface, width: 1.5),
                    ),
                    child: Text(
                      '$alerts',
                      style: TextStyle(
                        color: semantics.onError,
                        fontSize: 10,
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 5),
          ConstrainedBox(
            constraints: BoxConstraints(maxWidth: size.width - 8),
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              decoration: BoxDecoration(
                color: scheme.surface.withValues(alpha: 0.94),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: selected ? scheme.primary : scheme.outlineVariant,
                ),
              ),
              child: Text(
                node.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.center,
                style: context.textTheme.labelMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                  color: scheme.onSurface,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildDetailCard(
    BuildContext context,
    ColorScheme scheme,
    AppSemanticColors semantics,
    SemanticColorPair accentPair,
    SemanticColorPair statusPair,
  ) {
    final unmanaged = isUnmanagedSwitchNode(node);
    final borderColor = selected
        ? scheme.primary
        : isPathStart
        ? semantics.info
        : highlighted
        ? semantics.warning
        : classifyNodeStatus(node.status) == TopologyNodeStatus.offline
        ? semantics.error
        : scheme.outlineVariant;
    final borderWidth = selected
        ? 2.4
        : highlighted || isPathStart
        ? 2.2
        : 1.2;

    final microBadges = _buildMicroBadges(context, scheme, semantics);
    final subtitleText =
        showIpLabel && node.ipAddress != null && node.ipAddress!.isNotEmpty
        ? node.ipAddress!
        : topologyNodeTypeLabel(node);
    final isMonospaceSubtitle =
        showIpLabel && node.ipAddress != null && node.ipAddress!.isNotEmpty;

    return Container(
      key: ValueKey<String>('topology-node-detail-${node.id}'),
      decoration: BoxDecoration(
        borderRadius: AppTheme.borderRadius,
        boxShadow: selected || highlighted
            ? <BoxShadow>[
                BoxShadow(
                  color: (selected ? scheme.primary : semantics.warning)
                      .withValues(alpha: 0.28),
                  blurRadius: 14,
                  spreadRadius: 1,
                ),
              ]
            : null,
      ),
      child: Material(
        elevation: selected
            ? 6
            : highlighted
            ? 4
            : 2,
        color: scheme.surface,
        surfaceTintColor: Colors.transparent,
        shape: RoundedRectangleBorder(
          borderRadius: AppTheme.borderRadius,
          side: BorderSide(color: borderColor, width: borderWidth),
        ),
        clipBehavior: Clip.antiAlias,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            // Barra lateral semântica de estado.
            Container(width: 4.5, color: accentPair.foreground),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(9, 7, 9, 7),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: <Widget>[
                    // Linha 1: ícone + nome + indicadores de status/arraste.
                    Row(
                      children: <Widget>[
                        Container(
                          width: 26,
                          height: 26,
                          decoration: BoxDecoration(
                            color: accentPair.container,
                            borderRadius: BorderRadius.circular(7),
                          ),
                          child: Icon(
                            topologyNodeIcon(node),
                            size: 15,
                            color: accentPair.onContainer,
                          ),
                        ),
                        const SizedBox(width: 7),
                        Expanded(
                          child: Text(
                            node.name,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.labelLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                              color: scheme.onSurface,
                              height: 1.15,
                            ),
                          ),
                        ),
                        if (editLayoutEnabled) ...<Widget>[
                          const SizedBox(width: 4),
                          Icon(
                            Icons.drag_indicator,
                            size: 15,
                            color: scheme.primary,
                          ),
                        ] else ...<Widget>[
                          const SizedBox(width: 5),
                          Container(
                            width: 9,
                            height: 9,
                            decoration: BoxDecoration(
                              shape: BoxShape.circle,
                              color: statusPair.foreground,
                            ),
                          ),
                        ],
                      ],
                    ),
                    // Linha 2: IP em monoespaçada (ou tipo) + indicador de switch não gerenciável.
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            subtitleText,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.labelSmall?.copyWith(
                              fontFamily: isMonospaceSubtitle
                                  ? 'monospace'
                                  : null,
                              fontWeight: FontWeight.w600,
                              color: scheme.onSurfaceVariant,
                            ),
                          ),
                        ),
                        if (unmanaged)
                          _NodeMicroBadge(
                            label: 'N/G',
                            icon: Icons.hub_outlined,
                            background: semantics.cautionContainer,
                            foreground: semantics.onCautionContainer,
                          ),
                      ],
                    ),
                    // Linha 3: micro-badges ao vivo (latência, CPU, alertas, Wi-Fi/plugin).
                    if (microBadges.isNotEmpty)
                      SizedBox(
                        height: 20,
                        child: SingleChildScrollView(
                          scrollDirection: Axis.horizontal,
                          physics: const NeverScrollableScrollPhysics(),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: microBadges,
                          ),
                        ),
                      )
                    else
                      Text(
                        node.siteName ?? topologyNodeTypeLabel(node),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          fontSize: 10.5,
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  List<Widget> _buildMicroBadges(
    BuildContext context,
    ColorScheme scheme,
    AppSemanticColors semantics,
  ) {
    final items = <Widget>[];
    void addBadge(Widget badge) {
      if (items.isNotEmpty) items.add(const SizedBox(width: 4));
      items.add(badge);
    }

    final alerts = _effectiveAlertCount;
    if (alerts > 0) {
      addBadge(
        _NodeMicroBadge(
          label: '$alerts',
          icon: Icons.warning_amber_rounded,
          background: semantics.errorContainer,
          foreground: semantics.onErrorContainer,
        ),
      );
    }

    final latencyMs = extras?.latencyMs;
    if (latencyMs != null && latencyMs.isFinite) {
      final pair = semantics.resolve(latencyTone(latencyMs), scheme);
      addBadge(
        _NodeMicroBadge(
          label: formatLatency(latencyMs),
          background: pair.container,
          foreground: pair.onContainer,
        ),
      );
    }

    if (showHealthBadges &&
        node.cpuPercent != null &&
        node.cpuPercent!.isFinite) {
      final pair = semantics.resolve(
        classifyHealthPercent(node.cpuPercent!).tone,
        scheme,
      );
      addBadge(
        _NodeMicroBadge(
          label: 'CPU ${node.cpuPercent!.round()}%',
          background: pair.container,
          foreground: pair.onContainer,
        ),
      );
    }

    final wifiClients = extras?.wifiClientsCount ?? 0;
    if (wifiClients > 0) {
      addBadge(
        _NodeMicroBadge(
          label: '$wifiClients',
          icon: Icons.wifi,
          background: semantics.pluginContainer,
          foreground: semantics.onPluginContainer,
          borderColor: semantics.plugin.withValues(alpha: 0.55),
        ),
      );
    } else if (extras?.hasPluginActions == true) {
      addBadge(
        _NodeMicroBadge(
          label: 'Plugin',
          icon: Icons.extension_outlined,
          background: semantics.pluginContainer,
          foreground: semantics.onPluginContainer,
          borderColor: semantics.plugin.withValues(alpha: 0.55),
        ),
      );
    }

    if (showHealthBadges &&
        node.memoryPercent != null &&
        node.memoryPercent!.isFinite &&
        items.length < 3) {
      final pair = semantics.resolve(
        classifyHealthPercent(node.memoryPercent!).tone,
        scheme,
      );
      addBadge(
        _NodeMicroBadge(
          label: 'RAM ${node.memoryPercent!.round()}%',
          background: pair.container,
          foreground: pair.onContainer,
        ),
      );
    }

    return items;
  }
}

class _NodeMicroBadge extends StatelessWidget {
  const _NodeMicroBadge({
    required this.label,
    required this.background,
    required this.foreground,
    this.icon,
    this.borderColor,
  });

  final String label;
  final Color background;
  final Color foreground;
  final IconData? icon;
  final Color? borderColor;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.circular(6),
        border: borderColor != null ? Border.all(color: borderColor!) : null,
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (icon != null) ...<Widget>[
            Icon(icon, size: 11, color: foreground),
            const SizedBox(width: 2.5),
          ],
          Text(
            label,
            maxLines: 1,
            softWrap: false,
            style: TextStyle(
              fontSize: 10,
              fontWeight: FontWeight.w700,
              color: foreground,
              height: 1.1,
            ),
          ),
        ],
      ),
    );
  }
}

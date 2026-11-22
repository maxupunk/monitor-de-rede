import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/network/topology/engine/topology_layouts.dart'
    show
        kTopologyMacroNodeDiameter,
        kTopologyMacroNodeRadius,
        kTopologyMacroZoomThreshold;

/// Reconhecedor de arraste de nó com limiar menor (`computeHitSlop`: ~18px toque / 1px mouse)
/// do que o `ScaleGestureRecognizer` do `InteractiveViewer` (`computePanSlop`: ~36px / 2px).
///
/// Garante que, em modo de edição, arrastar sobre um cartão de nó vença sempre
/// a arena de gestos antes do pan da câmera, preservando taps limpos no nó e
/// pan/pinch no espaço vazio do canvas.
class _NodeDragGestureRecognizer extends PanGestureRecognizer {
  _NodeDragGestureRecognizer({super.debugOwner});

  @override
  bool hasSufficientGlobalDistanceToAccept(
    PointerDeviceKind pointerDeviceKind,
    double? deviceTouchSlop,
  ) {
    final threshold = computeHitSlop(pointerDeviceKind, gestureSettings);
    return globalDistanceMoved.abs() > threshold;
  }

  @override
  String get debugDescription => 'topology node drag';
}

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
    this.isDragging = false,
    this.overlayMode = TopologyOverlayMode.status,
    this.extras,
    this.onTap,
    this.onDoubleTap,
    this.onLongPress,
    this.onDragStart,
    this.onDragUpdate,
    this.onDragEnd,
    this.onDragCancel,
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
  final bool isDragging;
  final TopologyOverlayMode overlayMode;
  final TopologyNodeExtras? extras;
  final VoidCallback? onTap;
  final VoidCallback? onDoubleTap;
  final VoidCallback? onLongPress;
  final ValueChanged<Offset>? onDragStart;
  final ValueChanged<Offset>? onDragUpdate;
  final VoidCallback? onDragEnd;
  final VoidCallback? onDragCancel;

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
    final content = _isMacroLod
        ? _buildMacroNode(context, scheme, semantics, accentPair, statusPair)
        : _buildDetailCard(context, scheme, semantics, accentPair, statusPair);

    final gestures = <Type, GestureRecognizerFactory>{
      if (onTap != null)
        TapGestureRecognizer:
            GestureRecognizerFactoryWithHandlers<TapGestureRecognizer>(
              () => TapGestureRecognizer(debugOwner: this),
              (instance) => instance.onTap = onTap,
            ),
      if (onDoubleTap != null)
        DoubleTapGestureRecognizer:
            GestureRecognizerFactoryWithHandlers<DoubleTapGestureRecognizer>(
              () => DoubleTapGestureRecognizer(debugOwner: this),
              (instance) => instance.onDoubleTap = onDoubleTap,
            ),
      // Em modo de edição desabilitamos long-press no nó para que segurar
      // o dedo antes de arrastar no toque nunca abra o seletor de caminho.
      if (!editLayoutEnabled && onLongPress != null)
        LongPressGestureRecognizer:
            GestureRecognizerFactoryWithHandlers<LongPressGestureRecognizer>(
              () => LongPressGestureRecognizer(debugOwner: this),
              (instance) => instance.onLongPress = onLongPress,
            ),
      if (editLayoutEnabled)
        _NodeDragGestureRecognizer:
            GestureRecognizerFactoryWithHandlers<_NodeDragGestureRecognizer>(
              () => _NodeDragGestureRecognizer(debugOwner: this),
              (instance) {
                instance.dragStartBehavior = DragStartBehavior.down;
                instance.onStart = (details) =>
                    onDragStart?.call(details.globalPosition);
                instance.onUpdate = (details) =>
                    onDragUpdate?.call(details.globalPosition);
                instance.onEnd = (_) => onDragEnd?.call();
                instance.onCancel = () => onDragCancel?.call();
              },
            ),
    };

    final cursor = isDragging
        ? SystemMouseCursors.grabbing
        : editLayoutEnabled
        ? SystemMouseCursors.grab
        : SystemMouseCursors.click;

    final interactiveChild = MouseRegion(
      cursor: cursor,
      child: RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: gestures,
        child: Semantics(
          button: true,
          selected: selected,
          label:
              '${node.name}, ${topologyNodeTypeLabel(node)}, ${classifyNodeStatus(node.status).label}',
          child: content,
        ),
      ),
    );

    return AnimatedOpacity(
      duration: const Duration(milliseconds: 180),
      opacity: dimmed ? 0.26 : 1.0,
      child: AnimatedScale(
        duration: const Duration(milliseconds: 120),
        curve: Curves.easeOutCubic,
        scale: isDragging ? 1.035 : 1.0,
        child: SizedBox(
          width: size.width,
          height: size.height,
          child: _isMacroLod
              ? Center(
                  child: SizedBox(
                    width: math.min(size.width, 136.0),
                    height: size.height,
                    child: interactiveChild,
                  ),
                )
              : interactiveChild,
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
    final ringColor = isDragging || selected
        ? scheme.primary
        : isPathStart
        ? semantics.info
        : highlighted
        ? semantics.warning
        : editLayoutEnabled
        ? scheme.primary.withValues(alpha: 0.78)
        : accentPair.foreground;
    final alerts = _effectiveAlertCount;
    final elevated = isDragging || selected || highlighted;
    const circleDiameter = kTopologyMacroNodeDiameter;
    final circleTop = (size.height - circleDiameter) / 2;

    return Stack(
      key: ValueKey<String>('topology-node-macro-${node.id}'),
      clipBehavior: Clip.none,
      alignment: Alignment.center,
      children: <Widget>[
        Positioned(
          top: circleTop,
          child: Stack(
            clipBehavior: Clip.none,
            children: <Widget>[
              Container(
                width: circleDiameter,
                height: circleDiameter,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: scheme.surface,
                  border: Border.all(
                    color: ringColor,
                    width: isDragging
                        ? 4.0
                        : selected || highlighted
                        ? 3.5
                        : 2.6,
                  ),
                  boxShadow: <BoxShadow>[
                    BoxShadow(
                      color: (elevated ? ringColor : scheme.shadow).withValues(
                        alpha: isDragging
                            ? 0.45
                            : elevated
                            ? 0.34
                            : 0.2,
                      ),
                      blurRadius: isDragging
                          ? 18
                          : elevated
                          ? 12
                          : 6,
                      offset: Offset(0, isDragging ? 6 : 2),
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
              if (editLayoutEnabled)
                Positioned(
                  left: -4,
                  top: -4,
                  child: Container(
                    width: 20,
                    height: 20,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: isDragging
                          ? scheme.primary
                          : scheme.primaryContainer,
                      border: Border.all(color: scheme.surface, width: 1.5),
                    ),
                    child: Icon(
                      Icons.drag_indicator,
                      size: 12,
                      color: isDragging
                          ? scheme.onPrimary
                          : scheme.onPrimaryContainer,
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
        ),
        Positioned(
          top: circleTop + circleDiameter + 5,
          left: -24,
          right: -24,
          child: IgnorePointer(
            child: Center(
              child: ConstrainedBox(
                constraints: BoxConstraints(maxWidth: size.width - 8),
                child: Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 2,
                  ),
                  decoration: BoxDecoration(
                    color: scheme.surface,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(
                      color: isDragging || selected
                          ? scheme.primary
                          : scheme.outlineVariant,
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
            ),
          ),
        ),
      ],
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
    final borderColor = isDragging || selected
        ? scheme.primary
        : isPathStart
        ? semantics.info
        : highlighted
        ? semantics.warning
        : classifyNodeStatus(node.status) == TopologyNodeStatus.offline
        ? semantics.error
        : editLayoutEnabled
        ? scheme.primary.withValues(alpha: 0.55)
        : scheme.outlineVariant;
    final borderWidth = isDragging
        ? 2.8
        : selected
        ? 2.4
        : highlighted || isPathStart
        ? 2.2
        : editLayoutEnabled
        ? 1.6
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
        boxShadow: isDragging
            ? <BoxShadow>[
                BoxShadow(
                  color: scheme.primary.withValues(alpha: 0.34),
                  blurRadius: 20,
                  spreadRadius: 2,
                  offset: const Offset(0, 6),
                ),
              ]
            : selected || highlighted
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
        elevation: isDragging
            ? 10
            : selected
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
                          Container(
                            padding: const EdgeInsets.all(2),
                            decoration: BoxDecoration(
                              color: isDragging
                                  ? scheme.primary
                                  : scheme.primaryContainer,
                              borderRadius: BorderRadius.circular(6),
                            ),
                            child: Icon(
                              Icons.drag_indicator,
                              size: 14,
                              color: isDragging
                                  ? scheme.onPrimary
                                  : scheme.onPrimaryContainer,
                            ),
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
          label: 'CPU ${formatPercent(node.cpuPercent, digits: 0)}',
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
          label: 'RAM ${formatPercent(node.memoryPercent, digits: 0)}',
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

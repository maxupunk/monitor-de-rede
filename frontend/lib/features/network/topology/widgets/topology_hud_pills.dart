import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class TopologyLiveStatusPill extends StatelessWidget {
  const TopologyLiveStatusPill({
    super.key,
    required this.summary,
    required this.onlyProblemsActive,
    required this.offlineFilterActive,
    required this.onTapProblems,
    required this.onTapOffline,
  });

  final TopologyHealthSummary summary;
  final bool onlyProblemsActive;
  final bool offlineFilterActive;
  final VoidCallback onTapProblems;
  final VoidCallback onTapOffline;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final sem = context.semanticColors;

    return Material(
      elevation: 2,
      color: scheme.surface.withValues(alpha: 0.94),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(20),
        side: BorderSide(color: scheme.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        child: Wrap(
          spacing: 8,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            _StatusDotSegment(
              color: sem.success,
              label: '${summary.online} online',
            ),
            InkWell(
              onTap: onTapProblems,
              borderRadius: BorderRadius.circular(12),
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                decoration: onlyProblemsActive
                    ? BoxDecoration(
                        color: sem.warningContainer,
                        borderRadius: BorderRadius.circular(12),
                      )
                    : null,
                child: _StatusDotSegment(
                  color: sem.warning,
                  label: countNoun(summary.warning, 'alerta', 'alertas'),
                  highlighted: onlyProblemsActive || summary.warning > 0,
                  textColor: onlyProblemsActive ? sem.onWarningContainer : null,
                ),
              ),
            ),
            InkWell(
              onTap: onTapOffline,
              borderRadius: BorderRadius.circular(12),
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                decoration: offlineFilterActive
                    ? BoxDecoration(
                        color: sem.errorContainer,
                        borderRadius: BorderRadius.circular(12),
                      )
                    : null,
                child: _StatusDotSegment(
                  color: sem.error,
                  label: '${summary.offline} off',
                  highlighted: offlineFilterActive || summary.offline > 0,
                  textColor: offlineFilterActive ? sem.onErrorContainer : null,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusDotSegment extends StatelessWidget {
  const _StatusDotSegment({
    required this.color,
    required this.label,
    this.highlighted = false,
    this.textColor,
  });

  final Color color;
  final String label;
  final bool highlighted;
  final Color? textColor;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Container(
          width: 7,
          height: 7,
          decoration: BoxDecoration(shape: BoxShape.circle, color: color),
        ),
        const SizedBox(width: 5),
        Text(
          label,
          style: context.textTheme.labelSmall?.copyWith(
            fontWeight: highlighted ? FontWeight.w800 : FontWeight.w600,
            color: textColor ?? context.colorScheme.onSurface,
          ),
        ),
      ],
    );
  }
}

class TopologyActiveOverlayChip extends StatelessWidget {
  const TopologyActiveOverlayChip({
    super.key,
    required this.mode,
    required this.onClear,
  });

  final TopologyOverlayMode mode;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    return SemanticChip(
      label: 'Camada: ${mode.shortLabel}',
      tone: SemanticTone.primary,
      icon: mode.icon,
      compact: true,
      onTap: onClear,
    );
  }
}

class TopologyRemoteLayoutNoticePill extends StatelessWidget {
  const TopologyRemoteLayoutNoticePill({super.key, required this.onReload});

  final VoidCallback onReload;

  @override
  Widget build(BuildContext context) {
    return SemanticChip(
      label: 'Layout atualizado · Recarregar',
      tone: SemanticTone.info,
      icon: Icons.sync,
      compact: true,
      onTap: onReload,
    );
  }
}

/// Banner flutuante unificado para modos interativos da topologia (seleção de
/// enlace no mapa, seleção de caminho A -> B e exibição do caminho traçado).
class TopologyFloatingBanner extends StatelessWidget {
  const TopologyFloatingBanner({
    super.key,
    required this.icon,
    required this.tone,
    required this.label,
    this.actionLabel,
    this.onAction,
  });

  final IconData icon;
  final SemanticTone tone;
  final String label;
  final String? actionLabel;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(tone, scheme);

    return Material(
      elevation: 3,
      color: colors.container,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: colors.border),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(icon, size: 17, color: colors.onContainer),
            const SizedBox(width: 8),
            Flexible(
              child: Text(
                label,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelMedium?.copyWith(
                  color: colors.onContainer,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            if (onAction != null && actionLabel != null) ...<Widget>[
              const SizedBox(width: 8),
              TextButton(
                style: TextButton.styleFrom(
                  foregroundColor: colors.onContainer,
                  visualDensity: VisualDensity.compact,
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                ),
                onPressed: onAction,
                child: Text(actionLabel!),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class TopologyEditModeHelperPill extends StatelessWidget {
  const TopologyEditModeHelperPill({
    super.key,
    required this.snapToGrid,
    this.onToggleSnapToGrid,
    required this.onDone,
  });

  final bool snapToGrid;
  final VoidCallback? onToggleSnapToGrid;
  final VoidCallback onDone;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(SemanticTone.primary, scheme);

    return Material(
      elevation: 3,
      color: scheme.surface.withValues(alpha: 0.96),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(24),
        side: BorderSide(color: colors.foreground, width: 1.4),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
        child: Wrap(
          spacing: 8,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Icon(Icons.open_with, size: 16, color: colors.foreground),
            Text(
              'Arraste os nós para mover',
              style: context.textTheme.labelMedium?.copyWith(
                fontWeight: FontWeight.w700,
                color: scheme.onSurface,
              ),
            ),
            if (onToggleSnapToGrid != null)
              FilterChip(
                visualDensity: VisualDensity.compact,
                materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                avatar: const Icon(Icons.grid_4x4, size: 14),
                label: const Text('Grade'),
                selected: snapToGrid,
                onSelected: (_) => onToggleSnapToGrid!(),
              ),
            TextButton(
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 30),
                padding: const EdgeInsets.symmetric(horizontal: 10),
                visualDensity: VisualDensity.compact,
              ),
              onPressed: onDone,
              child: const Text('Concluir'),
            ),
          ],
        ),
      ),
    );
  }
}

class TopologyUnsavedLayoutPill extends StatelessWidget {
  const TopologyUnsavedLayoutPill({
    super.key,
    required this.saving,
    required this.canUndo,
    required this.movedCount,
    required this.snapToGrid,
    this.onToggleSnapToGrid,
    required this.onSave,
    this.onUndo,
    required this.onDiscard,
  });

  final bool saving;
  final bool canUndo;
  final int movedCount;
  final bool snapToGrid;
  final VoidCallback? onToggleSnapToGrid;
  final VoidCallback onSave;
  final VoidCallback? onUndo;
  final VoidCallback onDiscard;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(SemanticTone.primary, scheme);

    return Material(
      elevation: 3,
      color: scheme.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(24),
        side: BorderSide(color: colors.foreground, width: 1.5),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
        child: Wrap(
          spacing: 8,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Icon(Icons.open_with, size: 16, color: colors.foreground),
            Text(
              'Layout editado',
              style: context.textTheme.labelMedium?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            if (movedCount > 0)
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: colors.container,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text(
                  '$movedCount',
                  style: context.textTheme.labelSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                    color: colors.onContainer,
                  ),
                ),
              ),
            if (onToggleSnapToGrid != null)
              FilterChip(
                visualDensity: VisualDensity.compact,
                materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                avatar: const Icon(Icons.grid_4x4, size: 14),
                label: const Text('Grade'),
                selected: snapToGrid,
                onSelected: saving ? null : (_) => onToggleSnapToGrid!(),
              ),
            FilledButton(
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 32),
                padding: const EdgeInsets.symmetric(horizontal: 12),
                visualDensity: VisualDensity.compact,
              ),
              onPressed: saving ? null : onSave,
              child: saving
                  ? const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Salvar'),
            ),
            if (canUndo && onUndo != null)
              OutlinedButton.icon(
                style: OutlinedButton.styleFrom(
                  minimumSize: const Size(0, 32),
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  visualDensity: VisualDensity.compact,
                ),
                onPressed: saving ? null : onUndo,
                icon: const Icon(Icons.undo, size: 15),
                label: const Text('Desfazer'),
              ),
            TextButton(
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 32),
                padding: const EdgeInsets.symmetric(horizontal: 10),
                visualDensity: VisualDensity.compact,
              ),
              onPressed: saving ? null : onDiscard,
              child: const Text('Descartar'),
            ),
          ],
        ),
      ),
    );
  }
}

class TopologyThumbZoneControls extends StatelessWidget {
  const TopologyThumbZoneControls({
    super.key,
    required this.editLayoutEnabled,
    required this.canWrite,
    required this.overlayMode,
    this.zoomListenable,
    this.readZoomScale,
    required this.onZoomIn,
    required this.onZoomOut,
    required this.onFitToScreen,
    required this.onToggleEditLayout,
    required this.onOverlayChanged,
  });

  final bool editLayoutEnabled;
  final bool canWrite;
  final TopologyOverlayMode overlayMode;
  final Listenable? zoomListenable;
  final double Function()? readZoomScale;
  final VoidCallback onZoomIn;
  final VoidCallback onZoomOut;
  final VoidCallback onFitToScreen;
  final VoidCallback onToggleEditLayout;
  final ValueChanged<TopologyOverlayMode> onOverlayChanged;

  Widget _buildZoomPercentPill(BuildContext context, ColorScheme scheme) {
    final zoomLabel = formatPercent(
      (readZoomScale?.call() ?? 1.0) * 100,
      digits: 0,
    );
    return Tooltip(
      message: 'Zoom atual ($zoomLabel) — toque para ajustar à tela',
      child: InkWell(
        onTap: onFitToScreen,
        child: SizedBox(
          width: 44,
          height: 22,
          child: Center(
            child: Text(
              zoomLabel,
              maxLines: 1,
              softWrap: false,
              style: context.textTheme.labelSmall?.copyWith(
                fontSize: 10,
                fontWeight: FontWeight.w800,
                color: scheme.onSurfaceVariant,
                fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
              ),
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasCustomOverlay = overlayMode != TopologyOverlayMode.status;

    return Material(
      elevation: 3,
      color: scheme.surface.withValues(alpha: 0.96),
      shadowColor: scheme.shadow.withValues(alpha: 0.24),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          IconButton(
            tooltip: 'Aproximar zoom (+)',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.zoom_in, size: 20),
            onPressed: onZoomIn,
          ),
          if (zoomListenable != null && readZoomScale != null)
            ListenableBuilder(
              listenable: zoomListenable!,
              builder: (context, _) => _buildZoomPercentPill(context, scheme),
            )
          else if (readZoomScale != null)
            _buildZoomPercentPill(context, scheme),
          IconButton(
            tooltip: 'Afastar zoom (-)',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.zoom_out, size: 20),
            onPressed: onZoomOut,
          ),
          IconButton(
            tooltip: 'Ajustar à tela (0)',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.fit_screen_outlined, size: 20),
            onPressed: onFitToScreen,
          ),
          const SizedBox(width: 34, child: Divider(height: 1)),
          if (canWrite)
            Tooltip(
              message: editLayoutEnabled
                  ? 'Bloquear posições'
                  : 'Editar posições',
              child: InkWell(
                onTap: onToggleEditLayout,
                child: Container(
                  width: 40,
                  height: 40,
                  color: editLayoutEnabled
                      ? scheme.primaryContainer
                      : Colors.transparent,
                  child: Icon(
                    editLayoutEnabled
                        ? Icons.lock_open_outlined
                        : Icons.lock_outline,
                    size: 20,
                    color: editLayoutEnabled
                        ? scheme.onPrimaryContainer
                        : scheme.onSurface,
                  ),
                ),
              ),
            ),
          PopupMenuButton<TopologyOverlayMode>(
            tooltip: 'Camadas da topologia',
            onSelected: onOverlayChanged,
            itemBuilder: (_) => <PopupMenuEntry<TopologyOverlayMode>>[
              for (final mode in TopologyOverlayMode.values)
                PopupMenuItem<TopologyOverlayMode>(
                  value: mode,
                  child: ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    leading: Icon(
                      mode.icon,
                      color: overlayMode == mode ? scheme.primary : null,
                    ),
                    title: Text(
                      mode.label,
                      style: TextStyle(
                        fontWeight: overlayMode == mode
                            ? FontWeight.w700
                            : FontWeight.w500,
                      ),
                    ),
                  ),
                ),
            ],
            child: Container(
              width: 40,
              height: 40,
              color: hasCustomOverlay
                  ? scheme.secondaryContainer
                  : Colors.transparent,
              child: Icon(
                Icons.layers_outlined,
                size: 20,
                color: hasCustomOverlay
                    ? scheme.onSecondaryContainer
                    : scheme.onSurface,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

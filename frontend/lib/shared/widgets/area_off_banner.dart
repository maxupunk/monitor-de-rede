import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/shared/widgets/nested_panel.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Banner informativo quando uma área está desligada ou sem permissão (ADR 017 / 019).
class AreaOffBanner extends StatelessWidget {
  const AreaOffBanner({
    super.key,
    required this.area,
    this.title,
    this.summary,
    this.noAccess = false,
    this.isAdmin = false,
    this.loading = false,
    this.onEnableArea,
    this.onGoDashboard,
  });

  final ProductArea area;
  final String? title;
  final String? summary;
  final bool noAccess;
  final bool isAdmin;
  final bool loading;
  final VoidCallback? onEnableArea;
  final VoidCallback? onGoDashboard;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final tone = noAccess ? SemanticTone.error : SemanticTone.warning;
    final colors = context.semanticColors.resolve(tone, scheme);
    final effectiveTitle = title ?? area.defaultTitle;

    return NestedPanel(
      tone: tone,
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(
                noAccess ? Icons.lock_outline : Icons.power_off_outlined,
                color: colors.onContainer,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  noAccess
                      ? 'Sem acesso à área $effectiveTitle'
                      : 'A área $effectiveTitle está desligada',
                  style: context.textTheme.titleSmall?.copyWith(
                    color: colors.onContainer,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
            ],
          ),
          if (summary != null && summary!.isNotEmpty) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              summary!,
              style: context.textTheme.bodySmall?.copyWith(
                color: colors.onContainer,
              ),
            ),
          ],
          if (onEnableArea != null || onGoDashboard != null) ...<Widget>[
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                if (!noAccess && isAdmin && onEnableArea != null)
                  FilledButton.icon(
                    onPressed: loading ? null : onEnableArea,
                    icon: loading
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.power_settings_new),
                    label: const Text('Ligar área'),
                  ),
                if (onGoDashboard != null)
                  OutlinedButton(
                    onPressed: onGoDashboard,
                    child: const Text('Ir para o Dashboard'),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

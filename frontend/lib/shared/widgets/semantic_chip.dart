import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Chip semântico de alto contraste (proíbe chips cinzas desbotados — AGENTS.md §12).
class SemanticChip extends StatelessWidget {
  const SemanticChip({
    super.key,
    required this.label,
    required this.tone,
    this.icon,
    this.compact = false,
    this.filled = false,
    this.onTap,
  });

  final String label;
  final SemanticTone tone;
  final IconData? icon;
  final bool compact;
  final bool filled;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(tone, scheme);
    final bg = filled ? colors.foreground : colors.container;
    final fg = filled ? colors.onForeground : colors.onContainer;

    final content = Container(
      padding: EdgeInsets.symmetric(
        horizontal: compact ? 8 : 10,
        vertical: compact ? 3 : 5,
      ),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(10),
        border: filled ? null : Border.all(color: colors.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (icon != null) ...<Widget>[
            Icon(icon, size: compact ? 13 : 15, color: fg),
            SizedBox(width: compact ? 4 : 6),
          ],
          Flexible(
            child: Text(
              label,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: fg,
                fontSize: compact ? 11.5 : 12.5,
                fontWeight: FontWeight.w700,
                height: 1.2,
              ),
            ),
          ),
        ],
      ),
    );

    if (onTap == null) return content;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(10),
      child: content,
    );
  }
}

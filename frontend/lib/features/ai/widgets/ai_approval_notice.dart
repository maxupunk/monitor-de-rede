import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aviso global persistente para aprovações pendentes da IA (`AiApprovalNotice.vue`),
/// exibindo itens de `myAiPendingApprovalsProvider` que não estão visíveis no chat aberto.
class AiApprovalNoticeOverlay extends ConsumerWidget {
  const AiApprovalNoticeOverlay({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final mine = ref.watch(myAiPendingApprovalsProvider);
    if (mine.isEmpty) return const SizedBox.shrink();

    final drawerOpen = ref.watch(aiDrawerOpenProvider);
    final chat = ref.watch(aiChatProvider);
    final approvalsState = ref.watch(aiApprovalsProvider);

    final visible = mine
        .where((item) => !(drawerOpen && chat.hasTool(item.callId)))
        .toList(growable: false);
    if (visible.isEmpty) return const SizedBox.shrink();

    final current = visible.first;
    final busy = approvalsState.resolving.contains(current.callId);
    final inCurrentChat = chat.hasTool(current.callId);

    return Positioned(
      right: 16,
      bottom: 76,
      left: MediaQuery.sizeOf(context).width < 600 ? 16 : null,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: Material(
          elevation: 6,
          borderRadius: AppTheme.borderRadius,
          color: context.colorScheme.surface,
          child: Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              borderRadius: AppTheme.borderRadius,
              border: Border.all(
                color: context.semanticColors
                    .resolve(
                      current.risk?.tone ?? SemanticTone.warning,
                      context.colorScheme,
                    )
                    .foreground,
                width: 1.5,
              ),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(
                      Icons.smart_toy_outlined,
                      size: 18,
                      color: context.semanticColors.ai,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'A IA aguarda confirmação',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    if (visible.length > 1)
                      SemanticChip(
                        label: '+${visible.length - 1} na fila',
                        tone: SemanticTone.info,
                        compact: true,
                      ),
                  ],
                ),
                const SizedBox(height: 6),
                Wrap(
                  spacing: 6,
                  runSpacing: 4,
                  children: <Widget>[
                    if (current.risk != null)
                      SemanticChip(
                        label: current.risk!.label,
                        tone: current.risk!.tone,
                        icon: current.risk!.icon,
                        compact: true,
                      ),
                    if (current.createdAt != null)
                      Text(
                        formatRelativeTime(current.createdAt),
                        style: context.textTheme.labelSmall?.copyWith(
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                  ],
                ),
                const SizedBox(height: 8),
                Text(
                  current.summary.isNotEmpty ? current.summary : current.tool,
                  style: context.textTheme.bodySmall?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                if (current.reason != null &&
                    current.reason!.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 4),
                  Text(
                    current.reason!,
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
                const SizedBox(height: 12),
                Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  alignment: WrapAlignment.end,
                  children: <Widget>[
                    if (inCurrentChat)
                      TextButton(
                        onPressed: () =>
                            ref.read(aiDrawerOpenProvider.notifier).state =
                                true,
                        child: const Text('Ver no chat'),
                      ),
                    OutlinedButton(
                      onPressed: busy
                          ? null
                          : () => ref
                                .read(aiApprovalsProvider.notifier)
                                .dismiss(current.callId),
                      child: const Text('Cancelar'),
                    ),
                    FilledButton(
                      onPressed: busy
                          ? null
                          : () => ref
                                .read(aiApprovalsProvider.notifier)
                                .confirm(current.callId),
                      child: busy
                          ? const SizedBox(
                              width: 14,
                              height: 14,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Text('Confirmar'),
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

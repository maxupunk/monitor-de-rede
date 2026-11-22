import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Overlay modal persistente para aprovações individuais de acesso a equipamento
/// vindas do SSE (`plugin:approval_required` — FIFO `pluginsProvider.nextApproval`).
class PluginApprovalOverlay extends ConsumerStatefulWidget {
  const PluginApprovalOverlay({super.key});

  @override
  ConsumerState<PluginApprovalOverlay> createState() =>
      _PluginApprovalOverlayState();
}

class _PluginApprovalOverlayState extends ConsumerState<PluginApprovalOverlay> {
  bool _submitting = false;

  Future<void> _respond(String key, bool approved) async {
    setState(() => _submitting = true);
    try {
      await ref
          .read(pluginsProvider.notifier)
          .respondApproval(key, approved: approved);
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(pluginsProvider);
    final req = state.nextApproval;
    if (req == null) return const SizedBox.shrink();

    final scheme = context.colorScheme;
    final semantics = context.semanticColors;
    final effect = effectPresentation(req.effect);
    final isWrite = req.isWrite;
    final queueCount = state.pendingApprovals.length;

    return Positioned.fill(
      child: Material(
        color: Colors.black54,
        child: Center(
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 520),
              child: Material(
                elevation: 8,
                borderRadius: AppTheme.borderRadius,
                color: isWrite ? semantics.error : scheme.surface,
                child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Icon(
                            isWrite
                                ? Icons.gpp_maybe_outlined
                                : Icons.terminal_outlined,
                            color: isWrite
                                ? semantics.onError
                                : semantics.plugin,
                          ),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Text(
                              'Aprovar acesso ao equipamento',
                              style: context.textTheme.titleMedium?.copyWith(
                                fontWeight: FontWeight.w800,
                                color: isWrite
                                    ? semantics.onError
                                    : scheme.onSurface,
                              ),
                            ),
                          ),
                          if (queueCount > 1)
                            SemanticChip(
                              label: '1 de $queueCount',
                              tone: SemanticTone.warning,
                              compact: true,
                            ),
                        ],
                      ),
                      const SizedBox(height: 10),
                      Wrap(
                        spacing: 6,
                        runSpacing: 6,
                        children: <Widget>[
                          SemanticChip(
                            label: req.pluginName,
                            tone: SemanticTone.plugin,
                            compact: true,
                            filled: isWrite,
                          ),
                          SemanticChip(
                            label: req.transport.toUpperCase(),
                            tone: SemanticTone.info,
                            compact: true,
                            filled: isWrite,
                          ),
                          SemanticChip(
                            label: effect.label,
                            tone: effect.tone,
                            icon: effect.icon,
                            compact: true,
                            filled: !isWrite,
                          ),
                        ],
                      ),
                      const SizedBox(height: 10),
                      Text(
                        'Equipamento: ${req.deviceName} · Ação: ${req.action}',
                        style: context.textTheme.bodySmall?.copyWith(
                          fontWeight: FontWeight.w700,
                          color: isWrite ? semantics.onError : scheme.onSurface,
                        ),
                      ),
                      if (req.reason != null &&
                          req.reason!.isNotEmpty) ...<Widget>[
                        const SizedBox(height: 4),
                        Text(
                          req.reason!,
                          style: context.textTheme.bodySmall?.copyWith(
                            color: isWrite
                                ? semantics.onError.withValues(alpha: 0.9)
                                : scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                      const SizedBox(height: 10),
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: isWrite
                              ? Colors.black.withValues(alpha: 0.25)
                              : scheme.surfaceContainerLowest,
                          borderRadius: BorderRadius.circular(10),
                          border: Border.all(
                            color: isWrite
                                ? semantics.onError.withValues(alpha: 0.4)
                                : scheme.outlineVariant,
                          ),
                        ),
                        child: SelectableText(
                          req.summary,
                          style: TextStyle(
                            fontFamily: 'monospace',
                            fontSize: 12.5,
                            color: isWrite
                                ? semantics.onError
                                : scheme.onSurface,
                          ),
                        ),
                      ),
                      if (req.expiresAt.isNotEmpty) ...<Widget>[
                        const SizedBox(height: 8),
                        Text(
                          'Expira ${formatTimeUntil(req.expiresAt)}',
                          style: context.textTheme.labelSmall?.copyWith(
                            color: isWrite
                                ? semantics.onError.withValues(alpha: 0.85)
                                : scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                      const SizedBox(height: 16),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: <Widget>[
                          OutlinedButton(
                            style: isWrite
                                ? OutlinedButton.styleFrom(
                                    foregroundColor: semantics.onError,
                                    side: BorderSide(color: semantics.onError),
                                  )
                                : null,
                            onPressed: _submitting
                                ? null
                                : () => _respond(req.key, false),
                            child: const Text('Negar'),
                          ),
                          const SizedBox(width: 8),
                          FilledButton.icon(
                            style: isWrite
                                ? FilledButton.styleFrom(
                                    backgroundColor: semantics.onError,
                                    foregroundColor: semantics.error,
                                  )
                                : null,
                            onPressed: _submitting
                                ? null
                                : () => _respond(req.key, true),
                            icon: _submitting
                                ? const SizedBox(
                                    width: 14,
                                    height: 14,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.check, size: 18),
                            label: const Text('Aprovar'),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Overlay invisível da área `ProductArea.network` que sincroniza os Aplicativos
/// de Frota no menu lateral após autenticação, sem acoplar `features/ai/`.
class PluginAppsBootstrapOverlay extends ConsumerStatefulWidget {
  const PluginAppsBootstrapOverlay({super.key});

  @override
  ConsumerState<PluginAppsBootstrapOverlay> createState() =>
      _PluginAppsBootstrapOverlayState();
}

class _PluginAppsBootstrapOverlayState
    extends ConsumerState<PluginAppsBootstrapOverlay> {
  bool _bootstrapped = false;

  @override
  Widget build(BuildContext context) {
    final isAuthenticated = ref.watch(authProvider).isAuthenticated;
    if (isAuthenticated && !_bootstrapped) {
      _bootstrapped = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        ref.read(pluginAppsProvider.notifier).fetchApps();
      });
    } else if (!isAuthenticated && _bootstrapped) {
      _bootstrapped = false;
    }
    return const SizedBox.shrink();
  }
}

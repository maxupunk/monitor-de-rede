import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Banner no topo do chat quando há um equipamento no contexto (`deviceContextId`).
/// Permite ativar o modo "Aceitar automaticamente" mediante termo explícito (ADR 012)
/// e parar imediatamente com botão vermelho em destaque.
class AiAutoAcceptBanner extends ConsumerWidget {
  const AiAutoAcceptBanner({super.key});

  Future<void> _openTermsDialog(
    BuildContext context,
    WidgetRef ref,
    int deviceId,
    String deviceLabel,
  ) async {
    await ref.read(aiChatProvider.notifier).refreshAutoAccept(deviceId);
    if (!context.mounted) return;
    final state = ref.read(aiChatProvider).autoAccept;
    final termsText = state?.terms.isNotEmpty == true
        ? state!.terms
        : 'Ao ativar o modo "Aceitar automaticamente", os acessos da IA (SSH, HTTP e ações de plugin) '
              'neste equipamento serão aprovados sem confirmação individual por até 2 horas, '
              'restritos a esta conversa e a este dispositivo. A IA pode errar e alterar configurações reais.';

    var accepted = false;
    var submitting = false;

    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (dialogCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            return AlertDialog(
              icon: Icon(
                Icons.warning_amber_rounded,
                size: 36,
                color: context.semanticColors.error,
              ),
              title: const Text('Aceitar acessos automaticamente?'),
              content: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 480),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    NestedPanel(
                      tone: SemanticTone.warning,
                      child: Text(
                        'Só nesta conversa e só para $deviceLabel. Vale por até 2 horas.',
                        style: context.textTheme.bodySmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    ConstrainedBox(
                      constraints: const BoxConstraints(maxHeight: 200),
                      child: SingleChildScrollView(
                        child: Text(
                          termsText,
                          style: context.textTheme.bodySmall,
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    CheckboxListTile(
                      contentPadding: EdgeInsets.zero,
                      controlAffinity: ListTileControlAffinity.leading,
                      value: accepted,
                      onChanged: submitting
                          ? null
                          : (v) => setModalState(() => accepted = v == true),
                      title: const Text(
                        'Li o termo e assumo o risco de a IA acessar e alterar este equipamento sem pedir confirmação a cada comando.',
                        style: TextStyle(fontSize: 13),
                      ),
                    ),
                  ],
                ),
              ),
              actions: <Widget>[
                TextButton(
                  onPressed: submitting
                      ? null
                      : () => Navigator.of(dialogCtx).pop(),
                  child: const Text('Cancelar'),
                ),
                FilledButton(
                  style: FilledButton.styleFrom(
                    backgroundColor: context.semanticColors.error,
                    foregroundColor: context.semanticColors.onError,
                  ),
                  onPressed: accepted && !submitting
                      ? () async {
                          setModalState(() => submitting = true);
                          await ref
                              .read(aiChatProvider.notifier)
                              .enableAutoAccept(deviceId);
                          if (dialogCtx.mounted) {
                            Navigator.of(dialogCtx).pop();
                          }
                        }
                      : null,
                  child: submitting
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Text('Ativar modo automático'),
                ),
              ],
            );
          },
        );
      },
    );
  }

  Future<void> _stopAutoAccept(WidgetRef ref, int deviceId) async {
    await ref.read(aiChatProvider.notifier).disableAutoAccept(deviceId);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final chat = ref.watch(aiChatProvider);
    final deviceId = chat.deviceContextId;
    if (deviceId == null) return const SizedBox.shrink();

    final deviceLabel = chat.context.deviceName ?? 'dispositivo #$deviceId';
    final autoAccept = chat.autoAccept;
    final active = autoAccept?.active == true;
    final semantics = context.semanticColors;

    if (active) {
      return Container(
        margin: const EdgeInsets.only(bottom: 10),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        decoration: BoxDecoration(
          color: semantics.error,
          borderRadius: AppTheme.borderRadius,
        ),
        child: Row(
          children: <Widget>[
            Icon(Icons.bolt, color: semantics.onError, size: 20),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                'Aceitar automaticamente ATIVO para $deviceLabel'
                '${autoAccept?.expiresAt != null ? ' (expira ${formatTimeUntil(autoAccept!.expiresAt)})' : ''}',
                style: context.textTheme.bodySmall?.copyWith(
                  color: semantics.onError,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(width: 8),
            FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: semantics.onError,
                foregroundColor: semantics.error,
              ),
              onPressed: () => _stopAutoAccept(ref, deviceId),
              child: const Text('Parar'),
            ),
          ],
        ),
      );
    }

    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: NestedPanel(
        tone: SemanticTone.warning,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        child: Wrap(
          spacing: 10,
          runSpacing: 8,
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(
                  Icons.shield_outlined,
                  size: 18,
                  color: semantics.onWarningContainer,
                ),
                const SizedBox(width: 8),
                Flexible(
                  child: Text(
                    'Acessos da IA a $deviceLabel pedem sua aprovação um a um',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
            FilledButton.tonal(
              style: FilledButton.styleFrom(
                backgroundColor: semantics.errorContainer,
                foregroundColor: semantics.onErrorContainer,
              ),
              onPressed: () =>
                  _openTermsDialog(context, ref, deviceId, deviceLabel),
              child: const Text('Aceitar automaticamente'),
            ),
          ],
        ),
      ),
    );
  }
}

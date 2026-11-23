import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/device_defaults.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/shared/device_access/device_access_controller.dart';
import 'package:netmonitor_app/shared/device_access/device_access_panel.dart';
import 'package:netmonitor_app/shared/widgets/adaptive_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/confirm_action_dialog.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Indica se o erro retornado pelo backend sinaliza ausência de credencial (`missing_credential`).
bool isMissingDeviceCredentialError(Object? error) {
  return error is ApiException && error.isMissingCredential;
}

/// Indica se o equipamento remoto recusou usuário/senha/chave (HTTP 401/403 de alvo).
bool isDeviceRemoteAuthRejected(Object? error) => isDeviceAuthFailure(error);

/// Formata qualquer falha da API de acesso em mensagem clara em PT-BR (incluindo `fieldErrors`).
String formatDeviceAccessError(Object err, {String? platformOrOs}) {
  if (isDeviceAuthFailure(err)) {
    return formatDeviceAuthFailureMessage(err, platformOrOs: platformOrOs);
  }
  if (err is ApiException) {
    final parts = <String>[];
    if (err.message.trim().isNotEmpty) {
      parts.add(err.message.trim());
    }
    final desc = err.payload?.description?.trim();
    if (desc != null && desc.isNotEmpty && !parts.contains(desc)) {
      parts.add(desc);
    }
    for (final fieldErr in err.fieldErrors) {
      final line = fieldErr.field != null && fieldErr.field!.isNotEmpty
          ? '${fieldErr.field}: ${fieldErr.message}'
          : fieldErr.message;
      if (!parts.contains(line)) parts.add(line);
    }
    if (parts.isNotEmpty) return parts.join('\n');
  }
  return describeError(
    err,
    fallback: 'Não foi possível configurar o acesso ao equipamento.',
  );
}

/// Abre diretamente o modal adaptativo de credenciais / chave SSH dedicada (ADR 016).
Future<bool> showDeviceAccessDialog({
  required BuildContext context,
  WidgetRef? ref,
  required int deviceId,
  String? deviceName,
  String? platformHint,
  String? initialTransport,
  String? lockedTransport,
  String? reason,
  String? bannerMessage,
}) async {
  final effectiveTransport = lockedTransport ?? initialTransport;
  final subtitle = (deviceName != null && deviceName.trim().isNotEmpty)
      ? deviceName.trim()
      : (lockedTransport != null
            ? 'Transporte necessário: ${lockedTransport.toUpperCase()}'
            : 'Dispositivo #$deviceId');

  final saved = await showAdaptiveFormSheet<bool>(
    context: context,
    title: 'Acesso ao equipamento',
    subtitle: subtitle,
    maxWidth: 620,
    builder: (sheetCtx) => DeviceAccessPanel(
      deviceId: deviceId,
      deviceName: deviceName,
      platformHint: platformHint,
      initialTransport: effectiveTransport,
      lockedTransport: lockedTransport,
      bannerMessage: bannerMessage ?? reason,
      onSaved: () => Navigator.of(sheetCtx).pop(true),
    ),
  );
  return saved == true;
}

/// Executa `action` e intercepta `{ code: "missing_credential", deviceId, transport }`:
/// abre o diálogo de credenciais para administradores e repete a chamada após salvar.
///
/// Suporta tanto `confirmRetry` (`false` = reexecuta direto) quanto `retryAuto`
/// (`true` = reexecuta direto), mantendo compatibilidade total entre Rede e Plugins.
Future<T?> withDeviceAccess<T>({
  required BuildContext context,
  required WidgetRef ref,
  required Future<T> Function() action,
  bool? confirmRetry,
  bool? retryAuto,
  String? actionLabel,
  int? fallbackDeviceId,
  String? deviceName,
  VoidCallback? onSaved,
}) async {
  try {
    return await action();
  } on ApiException catch (err) {
    final targetDeviceId = err.deviceId ?? fallbackDeviceId;
    if (!err.isMissingCredential || targetDeviceId == null) {
      rethrow;
    }
    if (!context.mounted) return null;

    final auth = ref.read(authProvider);
    if (!auth.canManagePlugins) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            err.message.isNotEmpty
                ? '${err.message} Peça a um administrador para configurar o acesso.'
                : 'Credencial ausente no equipamento. Apenas administradores podem cadastrá-la.',
          ),
        ),
      );
      return null;
    }

    final saved = await showDeviceAccessDialog(
      context: context,
      ref: ref,
      deviceId: targetDeviceId,
      deviceName: deviceName,
      initialTransport: err.transport ?? 'ssh',
      lockedTransport: err.transport,
      reason: err.message,
    );
    if (!saved || !context.mounted) return null;
    onSaved?.call();

    final shouldConfirm = confirmRetry ?? !(retryAuto ?? true);
    if (shouldConfirm) {
      final proceed = await showConfirmActionDialog(
        context: context,
        title: 'Credencial salva',
        message: actionLabel != null
            ? 'Deseja executar "$actionLabel" agora?'
            : 'Deseja executar a operação novamente agora?',
        confirmLabel: 'Executar agora',
        tone: SemanticTone.primary,
        icon: Icons.play_arrow_rounded,
      );
      if (!proceed || !context.mounted) return null;
    }
    return action();
  }
}

/// Overlay global montado no shell para responder a chamadas imperativas em `deviceAccessProvider`.
class DeviceAccessHost extends ConsumerStatefulWidget {
  const DeviceAccessHost({super.key});

  @override
  ConsumerState<DeviceAccessHost> createState() => _DeviceAccessHostState();
}

class _DeviceAccessHostState extends ConsumerState<DeviceAccessHost> {
  bool _dialogOpen = false;

  @override
  Widget build(BuildContext context) {
    ref.listen<DeviceAccessRequest?>(deviceAccessProvider, (_, next) {
      if (next != null && !_dialogOpen && mounted) {
        _openSheet(next);
      }
    });
    return const SizedBox.shrink();
  }

  Future<void> _openSheet(DeviceAccessRequest req) async {
    _dialogOpen = true;
    final notifier = ref.read(deviceAccessProvider.notifier);
    bool saved = false;
    try {
      saved = await showDeviceAccessDialog(
        context: context,
        ref: ref,
        deviceId: req.deviceId,
        deviceName: req.deviceName,
        initialTransport: req.transport,
        lockedTransport: req.lockedTransport,
        bannerMessage: req.message,
      );
    } finally {
      _dialogOpen = false;
      notifier.settle(saved);
    }
  }
}

/// Exibe diálogo Material 3 claro quando o equipamento remoto recusa login/senha,
/// preservando a sessão do operador no NetMonitor.
Future<void> showDeviceAuthFailureDialog(
  BuildContext context,
  Object error, {
  String? platformOrOs,
}) {
  if (!context.mounted) return Future<void>.value();
  final message = formatDeviceAuthFailureMessage(
    error,
    platformOrOs: platformOrOs,
  );
  return showDialog<void>(
    context: context,
    builder: (dialogContext) {
      final scheme = dialogContext.colorScheme;
      final colors = dialogContext.semanticColors.resolve(
        SemanticTone.error,
        scheme,
      );
      return AlertDialog(
        shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        icon: Container(
          width: 48,
          height: 48,
          decoration: BoxDecoration(
            color: colors.container,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Icon(
            Icons.lock_person_outlined,
            color: colors.onContainer,
            size: 26,
          ),
        ),
        title: Text(
          'Usuário ou senha incorretos',
          textAlign: TextAlign.center,
          style: dialogContext.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        content: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Text(
            message,
            style: dialogContext.textTheme.bodyMedium?.copyWith(
              color: scheme.onSurface,
            ),
          ),
        ),
        actions: <Widget>[
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Entendi'),
            ),
          ),
        ],
      );
    },
  );
}

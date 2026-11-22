import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/clipboard_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';

/// Modal que exibe as regras de firewall sugeridas quando o túnel responde mas o ping não passa.
Future<void> showVpnFirewallHintsDialog({
  required BuildContext context,
  required String deviceName,
  required String rulesContent,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Regras de firewall · $deviceName',
    subtitle:
        'O túnel está conectado, mas o equipamento bloqueia ICMP vindo da VPN.',
    maxWidth: 640,
    actions: <Widget>[
      TextButton(
        onPressed: () => Navigator.of(context).pop(),
        child: const Text('Fechar'),
      ),
      FilledButton.icon(
        onPressed: () => copyToClipboardWithFeedback(
          context,
          rulesContent,
          message: 'Regras copiadas para a área de transferência.',
        ),
        icon: const Icon(Icons.copy, size: 18),
        label: const Text('Copiar regras'),
      ),
    ],
    builder: (_) => CodeViewer(
      title: 'Comandos sugeridos',
      code: rulesContent,
      maxHeight: 340,
    ),
  );
}

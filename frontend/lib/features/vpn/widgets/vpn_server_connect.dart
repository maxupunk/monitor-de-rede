import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_peer_wizard.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_script_viewer.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Fluxo plugável registrado em `vpnArea.serverConnectPaths` para a tela de Servidores Remotos.
/// Máquina de estados em 4 etapas: `idle` -> `setup` (se não configurado) | `wizard` -> `script` -> `onDone`.
class VpnServerConnectDialog extends ConsumerStatefulWidget {
  const VpnServerConnectDialog({
    super.key,
    this.serverName,
    this.agentId,
    this.onDone,
  });

  final String? serverName;
  final int? agentId;
  final VoidCallback? onDone;

  @override
  ConsumerState<VpnServerConnectDialog> createState() =>
      _VpnServerConnectDialogState();
}

enum _VpnConnectStage { loading, needsSetup, ready }

class _VpnServerConnectDialogState
    extends ConsumerState<VpnServerConnectDialog> {
  _VpnConnectStage _stage = _VpnConnectStage.loading;
  String? _error;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _init());
  }

  Future<void> _init() async {
    await ref.read(vpnProvider.notifier).fetchServer();
    if (!mounted) return;
    final vpn = ref.read(vpnProvider);
    if (vpn.error != null) {
      setState(() {
        _error = vpn.error;
        _stage = _VpnConnectStage.needsSetup;
      });
      return;
    }
    if (!vpn.isConfigured) {
      setState(() => _stage = _VpnConnectStage.needsSetup);
      return;
    }
    setState(() => _stage = _VpnConnectStage.ready);
    await _runWizardFlow();
  }

  Future<void> _runWizardFlow() async {
    final artifact = await showVpnPeerWizard(
      context: context,
      preset: VpnPeerWizardPreset(
        name: widget.serverName,
        agentId: widget.agentId,
      ),
    );
    if (!mounted) return;
    if (artifact != null) {
      await showVpnScriptViewer(
        context: context,
        ref: ref,
        artifact: artifact,
        deviceName: widget.serverName,
      );
      widget.onDone?.call();
    }
    if (mounted) {
      Navigator.of(context).pop();
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_stage == _VpnConnectStage.loading ||
        _stage == _VpnConnectStage.ready) {
      return const AlertDialog(
        content: Padding(
          padding: EdgeInsets.symmetric(vertical: 16),
          child: LoadingStateView(message: 'Verificando servidor VPN...'),
        ),
      );
    }

    return AlertDialog(
      icon: Icon(
        Icons.vpn_lock_outlined,
        size: 36,
        color: context.semanticColors
            .resolve(SemanticTone.warning, context.colorScheme)
            .foreground,
      ),
      title: const Text('Configure o servidor VPN primeiro'),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: Text(
          _error ?? 'O servidor WireGuard ainda não está configurado. Defina o endereço público e a faixa da VPN para gerar o túnel deste servidor.',
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancelar'),
        ),
        FilledButton.icon(
          onPressed: () {
            Navigator.of(context).pop();
            context.go('/vpn');
          },
          icon: const Icon(Icons.settings_outlined),
          label: const Text('Ir para VPN WireGuard'),
        ),
      ],
    );
  }
}

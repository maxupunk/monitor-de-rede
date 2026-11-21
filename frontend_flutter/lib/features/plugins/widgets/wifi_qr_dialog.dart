import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/wifi_qr_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_access_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:qr_flutter/qr_flutter.dart';

/// Visualizador efêmero de segredo (`reveals: true`) com QR Code Wi-Fi opcional (ADR 012).
///
/// Segurança:
/// - A senha é buscada sob demanda em `POST /api/devices/{id}/plugins/{id}/reveal/{action}`.
/// - NUNCA é gravada em histórico, estado global ou armazenamento local.
/// - É apagada imediatamente da memória no `dispose()`.
Future<void> showWifiQrRevealDialog({
  required BuildContext context,
  required int deviceId,
  required String deviceName,
  required int pluginId,
  required String ssid,
  required String encryption,
  bool hidden = false,
  required String revealAction,
  Map<String, dynamic> revealParams = const <String, dynamic>{},
  String secretField = 'password',
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'QR Code Wi-Fi · $ssid',
    subtitle: 'Lido diretamente do equipamento $deviceName (não armazenado).',
    maxWidth: 460,
    builder: (_) => _WifiQrBody(
      deviceId: deviceId,
      deviceName: deviceName,
      pluginId: pluginId,
      ssid: ssid,
      encryption: encryption,
      hidden: hidden,
      revealAction: revealAction,
      revealParams: revealParams,
      secretField: secretField,
    ),
  );
}

/// Visualizador genérico efêmero para qualquer ação `reveals: true`.
Future<void> showEphemeralRevealDialog({
  required BuildContext context,
  required int deviceId,
  required int pluginId,
  required String actionName,
  required String actionTitle,
  Map<String, dynamic> params = const <String, dynamic>{},
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: actionTitle,
    subtitle: 'Exibição temporária apenas para esta sessão — nunca gravada em histórico ou logs.',
    maxWidth: 520,
    builder: (_) => _EphemeralRevealBody(
      deviceId: deviceId,
      pluginId: pluginId,
      actionName: actionName,
      params: params,
    ),
  );
}

class _WifiQrBody extends ConsumerStatefulWidget {
  const _WifiQrBody({
    required this.deviceId,
    required this.deviceName,
    required this.pluginId,
    required this.ssid,
    required this.encryption,
    required this.hidden,
    required this.revealAction,
    required this.revealParams,
    required this.secretField,
  });

  final int deviceId;
  final String deviceName;
  final int pluginId;
  final String ssid;
  final String encryption;
  final bool hidden;
  final String revealAction;
  final Map<String, dynamic> revealParams;
  final String secretField;

  @override
  ConsumerState<_WifiQrBody> createState() => _WifiQrBodyState();
}

class _WifiQrBodyState extends ConsumerState<_WifiQrBody> {
  String? _password;
  bool _loading = false;
  bool _showPassword = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    if (wifiNeedsPassword(widget.encryption)) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _fetchSecret());
    }
  }

  @override
  void dispose() {
    // Limpa o segredo da memória imediatamente ao fechar (ADR 012).
    _password = null;
    super.dispose();
  }

  Future<void> _fetchSecret() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final output = await withDeviceAccess<Map<String, dynamic>>(
        context: context,
        ref: ref,
        action: () => ref
            .read(pluginsProvider.notifier)
            .revealSecret(
              deviceId: widget.deviceId,
              pluginId: widget.pluginId,
              action: widget.revealAction,
              params: widget.revealParams,
            ),
      );
      if (!mounted) return;
      final secret = output?[widget.secretField]?.toString();
      setState(() {
        _password = secret;
        _loading = false;
      });
    } catch (err) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = err.toString();
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(
        message: 'Lendo senha diretamente do equipamento...',
      );
    }
    if (_error != null) {
      return ErrorStateView(message: _error!, onRetry: _fetchSecret);
    }

    final needsPwd = wifiNeedsPassword(widget.encryption);
    final payload = wifiQrPayload(
      ssid: widget.ssid,
      encryption: widget.encryption,
      password: _password,
      hidden: widget.hidden,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Center(
          child: Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: AppTheme.borderRadius,
              border: Border.all(color: context.colorScheme.outlineVariant),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                QrImageView(
                  data: payload,
                  version: QrVersions.auto,
                  size: 220,
                  backgroundColor: Colors.white,
                  eyeStyle: const QrEyeStyle(
                    eyeShape: QrEyeShape.square,
                    color: Colors.black,
                  ),
                  dataModuleStyle: const QrDataModuleStyle(
                    dataModuleShape: QrDataModuleShape.square,
                    color: Colors.black,
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  'Wi-Fi: ${widget.ssid}',
                  style: const TextStyle(
                    color: Color(0xFF0F172A),
                    fontWeight: FontWeight.w700,
                    fontSize: 13,
                  ),
                ),
              ],
            ),
          ),
        ),
        if (widget.hidden) ...<Widget>[
          const SizedBox(height: 10),
          const NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              'Esta rede está configurada como oculta (SSID oculto).',
            ),
          ),
        ],
        if (needsPwd && _password != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        'Senha atual',
                        style: context.textTheme.labelSmall?.copyWith(
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                      SelectableText(
                        _showPassword ? _password! : '••••••••••••',
                        style: const TextStyle(
                          fontFamily: 'monospace',
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: _showPassword ? 'Ocultar senha' : 'Mostrar senha',
                  icon: Icon(
                    _showPassword ? Icons.visibility_off : Icons.visibility,
                  ),
                  onPressed: () =>
                      setState(() => _showPassword = !_showPassword),
                ),
                IconButton(
                  tooltip: 'Copiar senha',
                  icon: const Icon(Icons.copy),
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: _password!));
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Senha copiada.')),
                    );
                  },
                ),
              ],
            ),
          ),
        ],
        const SizedBox(height: 10),
        Text(
          'Lida de ${widget.deviceName}. Ela não fica guardada no NetMonitor.',
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }
}

class _EphemeralRevealBody extends ConsumerStatefulWidget {
  const _EphemeralRevealBody({
    required this.deviceId,
    required this.pluginId,
    required this.actionName,
    required this.params,
  });

  final int deviceId;
  final int pluginId;
  final String actionName;
  final Map<String, dynamic> params;

  @override
  ConsumerState<_EphemeralRevealBody> createState() =>
      _EphemeralRevealBodyState();
}

class _EphemeralRevealBodyState extends ConsumerState<_EphemeralRevealBody> {
  Map<String, dynamic>? _output;
  bool _loading = true;
  bool _visible = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  @override
  void dispose() {
    _output = null;
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final result = await withDeviceAccess<Map<String, dynamic>>(
        context: context,
        ref: ref,
        action: () => ref
            .read(pluginsProvider.notifier)
            .revealSecret(
              deviceId: widget.deviceId,
              pluginId: widget.pluginId,
              action: widget.actionName,
              params: widget.params,
            ),
      );
      if (mounted) {
        setState(() {
          _output = result;
          _loading = false;
        });
      }
    } catch (err) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = err.toString();
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Lendo segredo do equipamento...');
    }
    if (_error != null) {
      return ErrorStateView(message: _error!, onRetry: _load);
    }
    final data = _output ?? const <String, dynamic>{};

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        const NestedPanel(
          tone: SemanticTone.warning,
          child: Text(
            'Exibição efêmera exclusiva para o administrador. Ao fechar esta janela os dados são descartados da memória.',
          ),
        ),
        const SizedBox(height: 12),
        for (final entry in data.entries)
          NestedPanel(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(entry.key, style: context.textTheme.labelSmall),
                      SelectableText(
                        _visible ? '${entry.value}' : '••••••••••••',
                        style: const TextStyle(
                          fontFamily: 'monospace',
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: _visible ? 'Ocultar' : 'Mostrar',
                  icon: Icon(
                    _visible ? Icons.visibility_off : Icons.visibility,
                  ),
                  onPressed: () => setState(() => _visible = !_visible),
                ),
                IconButton(
                  tooltip: 'Copiar',
                  icon: const Icon(Icons.copy),
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: '${entry.value}'));
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Valor copiado.')),
                    );
                  },
                ),
              ],
            ),
          ),
      ],
    );
  }
}

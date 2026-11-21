import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:qr_flutter/qr_flutter.dart';

/// Monta a string padrão `WIFI:T:...;S:...;P:...;H:...;;` para QR Code Wi-Fi.
String buildWifiQrPayload({
  required String ssid,
  String password = '',
  String security = 'WPA',
  bool hidden = false,
}) {
  String esc(String input) => input
      .replaceAll('\\', r'\\')
      .replaceAll(';', r'\;')
      .replaceAll(',', r'\,')
      .replaceAll(':', r'\:');

  final effectiveSec = password.isEmpty ? 'nopass' : security;
  return 'WIFI:T:$effectiveSec;S:${esc(ssid)};P:${esc(password)};H:${hidden ? 'true' : 'false'};;';
}

/// Abre o sheet de QR Code Wi-Fi (suporta revelação efêmera `reveals: true` para admins, sem persistir senha).
Future<void> showWifiQrSheet(
  BuildContext context, {
  String initialSsid = '',
  int? deviceId,
  int? pluginId,
  String? revealAction,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'QR Code Wi-Fi',
    subtitle: 'Compartilhe o acesso à rede sem fio com celulares e tablets',
    maxWidth: 460,
    builder: (_) => _WifiQrBody(
      initialSsid: initialSsid,
      deviceId: deviceId,
      pluginId: pluginId,
      revealAction: revealAction,
    ),
  );
}

class _WifiQrBody extends ConsumerStatefulWidget {
  const _WifiQrBody({
    required this.initialSsid,
    this.deviceId,
    this.pluginId,
    this.revealAction,
  });

  final String initialSsid;
  final int? deviceId;
  final int? pluginId;
  final String? revealAction;

  @override
  ConsumerState<_WifiQrBody> createState() => _WifiQrBodyState();
}

class _WifiQrBodyState extends ConsumerState<_WifiQrBody> {
  late final TextEditingController _ssidCtrl;
  final TextEditingController _passCtrl = TextEditingController();
  String _security = 'WPA';
  bool _hidden = false;
  bool _obscure = true;
  bool _revealing = false;
  String? _revealError;

  @override
  void initState() {
    super.initState();
    _ssidCtrl = TextEditingController(text: widget.initialSsid);
  }

  @override
  void dispose() {
    _ssidCtrl.dispose();
    _passCtrl.dispose();
    super.dispose();
  }

  /// Revelação efêmera (`POST /api/devices/:id/plugins/:pluginId/reveal/:action`) — somente admin, nunca salva.
  Future<void> _revealPassword() async {
    if (widget.deviceId == null ||
        widget.pluginId == null ||
        widget.revealAction == null) {
      return;
    }
    setState(() {
      _revealing = true;
      _revealError = null;
    });
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/devices/${widget.deviceId}/plugins/${widget.pluginId}/reveal/${widget.revealAction}',
            timeout: const Duration(seconds: 30),
            data: <String, dynamic>{
              'params': <String, dynamic>{'ssid': _ssidCtrl.text.trim()},
            },
          );
      final out = asJsonMap(asJsonMap(raw)['output']);
      final secret = (out['password'] ?? out['psk'] ?? out['passphrase'])
          ?.toString();
      if (mounted && secret != null) {
        setState(() => _passCtrl.text = secret);
      }
    } catch (err) {
      if (mounted) setState(() => _revealError = err.toString());
    } finally {
      if (mounted) setState(() => _revealing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isAdmin = ref.watch(authProvider).canManagePlugins;
    final ssid = _ssidCtrl.text.trim();
    final qrData = ssid.isNotEmpty
        ? buildWifiQrPayload(
            ssid: ssid,
            password: _passCtrl.text,
            security: _security,
            hidden: _hidden,
          )
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _ssidCtrl,
          decoration: const InputDecoration(labelText: 'Nome da rede (SSID)'),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 10),
        Row(
          children: <Widget>[
            Expanded(
              child: DropdownButtonFormField<String>(
                initialValue: _security,
                decoration: const InputDecoration(labelText: 'Segurança'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(
                    value: 'WPA',
                    child: Text('WPA / WPA2 / WPA3'),
                  ),
                  DropdownMenuItem(value: 'WEP', child: Text('WEP')),
                  DropdownMenuItem(
                    value: 'nopass',
                    child: Text('Aberta (sem senha)'),
                  ),
                ],
                onChanged: (v) => setState(() => _security = v ?? 'WPA'),
              ),
            ),
            const SizedBox(width: 8),
            FilterChip(
              label: const Text('Rede oculta'),
              selected: _hidden,
              onSelected: (v) => setState(() => _hidden = v),
            ),
          ],
        ),
        if (_security != 'nopass') ...<Widget>[
          const SizedBox(height: 10),
          TextField(
            controller: _passCtrl,
            obscureText: _obscure,
            onChanged: (_) => setState(() {}),
            decoration: InputDecoration(
              labelText: 'Senha da rede',
              suffixIcon: IconButton(
                icon: Icon(
                  _obscure
                      ? Icons.visibility_outlined
                      : Icons.visibility_off_outlined,
                ),
                onPressed: () => setState(() => _obscure = !_obscure),
              ),
            ),
          ),
        ],
        if (isAdmin &&
            widget.deviceId != null &&
            widget.pluginId != null &&
            widget.revealAction != null) ...<Widget>[
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: _revealing ? null : _revealPassword,
            icon: const Icon(Icons.key, size: 18),
            label: Text(
              _revealing
                  ? 'Consultando equipamento...'
                  : 'Revelar senha atual (efêmero)',
            ),
          ),
        ],
        if (_revealError != null) ...<Widget>[
          const SizedBox(height: 8),
          NestedPanel(tone: SemanticTone.error, child: Text(_revealError!)),
        ],
        const SizedBox(height: 16),
        if (qrData != null) ...<Widget>[
          Center(
            child: Container(
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                color: Colors.white,
                borderRadius: BorderRadius.circular(14),
                border: Border.all(color: context.colorScheme.outlineVariant),
              ),
              child: QrImageView(
                data: qrData,
                size: 210,
                backgroundColor: Colors.white,
              ),
            ),
          ),
          const SizedBox(height: 10),
          Center(
            child: TextButton.icon(
              onPressed: () {
                Clipboard.setData(ClipboardData(text: qrData));
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Payload Wi-Fi copiado.')),
                );
              },
              icon: const Icon(Icons.copy, size: 16),
              label: const Text('Copiar string Wi-Fi'),
            ),
          ),
        ] else
          const EmptyStateView(
            title: 'Informe o SSID para gerar o QR Code',
            icon: Icons.qr_code_2,
            tone: SemanticTone.plugin,
          ),
      ],
    );
  }
}

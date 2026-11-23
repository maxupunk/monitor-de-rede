import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Future<void> showLogSetupGuideSheet(
  BuildContext context,
  WidgetRef ref, {
  int? defaultDeviceId,
}) async {
  await ref.read(coreSettingsProvider.notifier).fetchServerAddresses();
  await ref.read(logsProvider.notifier).fetchSetupGuide();
  if (!context.mounted) return;

  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Configurar envio de Syslog',
    subtitle:
        'Comandos prontos por sistema operacional ou ativação automática.',
    maxWidth: 700,
    builder: (_) => _LogSetupGuideSheetBody(
      parentContext: context,
      defaultDeviceId: defaultDeviceId,
    ),
  );
}

class _LogSetupGuideSheetBody extends ConsumerStatefulWidget {
  const _LogSetupGuideSheetBody({
    required this.parentContext,
    this.defaultDeviceId,
  });

  final BuildContext parentContext;
  final int? defaultDeviceId;

  @override
  ConsumerState<_LogSetupGuideSheetBody> createState() =>
      _LogSetupGuideSheetBodyState();
}

class _LogSetupGuideSheetBodyState
    extends ConsumerState<_LogSetupGuideSheetBody> {
  String? _selectedAddress;

  @override
  Widget build(BuildContext context) {
    final logs = ref.watch(logsProvider);
    final settings = ref.watch(coreSettingsProvider);
    final auth = ref.watch(authProvider);
    final guide = logs.setupGuide;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (settings.usableAddresses.isNotEmpty) ...<Widget>[
          AppDropdownFormField<String?>(
            initialValue: _selectedAddress,
            decoration: const InputDecoration(
              labelText: 'Endereço deste servidor nos comandos',
            ),
            items: <DropdownMenuItem<String?>>[
              const DropdownMenuItem<String?>(
                value: null,
                child: Text(
                  'Padrão do servidor',
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              for (final addr in settings.usableAddresses)
                DropdownMenuItem<String?>(
                  value: addr.value,
                  child: Text(
                    '${addr.label} (${addr.value})',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
            onChanged: (v) async {
              setState(() => _selectedAddress = v);
              await ref.read(logsProvider.notifier).fetchSetupGuide(address: v);
            },
          ),
          const SizedBox(height: 12),
        ],
        if (auth.canWrite) ...<Widget>[
          OutlinedButton.icon(
            onPressed: () {
              Navigator.of(context).pop();
              if (widget.parentContext.mounted) {
                showLogAutoProvisionSheet(
                  widget.parentContext,
                  ref,
                  defaultDeviceId: widget.defaultDeviceId,
                );
              }
            },
            icon: const Icon(Icons.auto_fix_high, size: 18),
            label: const Text('Ativar syslog automaticamente via SSH/Telnet'),
          ),
          const SizedBox(height: 14),
        ],
        if (guide == null)
          const LoadingStateView(message: 'Carregando guia de configuração...')
        else ...<Widget>[
          KeyValueRow(
            label: 'Destino Syslog',
            value: '${guide.serverAddress}:${guide.port}',
            monospace: true,
            copyable: true,
          ),
          const SizedBox(height: 10),
          for (final snippet in guide.snippets) ...<Widget>[
            CodeViewer(title: snippet.label, code: snippet.commands),
            if (snippet.notes != null && snippet.notes!.isNotEmpty) ...<Widget>[
              const SizedBox(height: 4),
              Text(
                snippet.notes!,
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
            const SizedBox(height: 12),
          ],
        ],
      ],
    );
  }
}

Future<void> showLogAutoProvisionSheet(
  BuildContext context,
  WidgetRef ref, {
  int? defaultDeviceId,
}) async {
  final devices = ref.read(monitorsProvider).devices;
  if (devices.isEmpty) {
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Cadastre ao menos um dispositivo antes de provisionar.'),
      ),
    );
    return;
  }

  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Ativação automática de Syslog',
    subtitle:
        'A senha nunca é gravada nem exibida na transcrição (exceto se salvar credencial).',
    maxWidth: 640,
    builder: (_) => _LogAutoProvisionSheetBody(
      initialDeviceId: defaultDeviceId ?? devices.first.id,
    ),
  );
}

class _LogAutoProvisionSheetBody extends ConsumerStatefulWidget {
  const _LogAutoProvisionSheetBody({required this.initialDeviceId});

  final int initialDeviceId;

  @override
  ConsumerState<_LogAutoProvisionSheetBody> createState() =>
      _LogAutoProvisionSheetBodyState();
}

class _LogAutoProvisionSheetBodyState
    extends ConsumerState<_LogAutoProvisionSheetBody> {
  late int _selectedDeviceId;
  String _protocol = 'ssh';
  String? _osId;
  bool _useSaved = false;
  bool _saveCred = false;
  bool _running = false;
  ProvisionHintsResponse? _hints;
  ProvisionLoggingResponse? _result;
  String? _error;

  final TextEditingController _portCtrl = TextEditingController(text: '22');
  final TextEditingController _userCtrl = TextEditingController();
  final TextEditingController _passCtrl = TextEditingController();
  final TextEditingController _addrCtrl = TextEditingController();
  final TextEditingController _macCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _selectedDeviceId = widget.initialDeviceId;
    _loadHints(_selectedDeviceId);
  }

  @override
  void dispose() {
    // Segurança: descarta a senha em memória ao encerrar o estado do modal.
    _passCtrl.clear();
    _portCtrl.dispose();
    _userCtrl.dispose();
    _passCtrl.dispose();
    _addrCtrl.dispose();
    _macCtrl.dispose();
    super.dispose();
  }

  Future<void> _loadHints(int devId) async {
    final h = await ref.read(logsProvider.notifier).fetchProvisionHints(devId);
    if (!mounted) return;
    setState(() {
      _hints = h;
      if (h != null) {
        if (h.serverAddress != null) _addrCtrl.text = h.serverAddress!;
        if (h.macAddress != null) _macCtrl.text = h.macAddress!;
        if (h.operatingSystem != null) _osId = h.operatingSystem;
        if (h.sshOpen) {
          _protocol = 'ssh';
          _portCtrl.text = '22';
        } else if (h.telnetOpen) {
          _protocol = 'telnet';
          _portCtrl.text = '23';
        } else if (!h.layer2Reachable && _protocol == 'mactelnet') {
          _protocol = 'ssh';
          _portCtrl.text = '22';
        }
        _useSaved = h.savedCredentials.contains(_protocol);
      }
    });
  }

  Future<void> _applyProvision() async {
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      final res = await ref
          .read(logsProvider.notifier)
          .provisionDevice(_selectedDeviceId, <String, dynamic>{
            'protocol': _protocol,
            'port': parseIntOrNull(_portCtrl.text),
            'username': _userCtrl.text.trim(),
            'password': _passCtrl.text,
            'savedCredential': _useSaved ? _protocol : null,
            'saveCredential': !_useSaved && _saveCred,
            'operatingSystem': _osId,
            'serverAddress': _addrCtrl.text.trim().isEmpty
                ? null
                : _addrCtrl.text.trim(),
            'macAddress': _macCtrl.text.trim().isEmpty
                ? null
                : _macCtrl.text.trim(),
          });
      if (!mounted) return;
      setState(() {
        _running = false;
        _result = res;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _running = false;
        _error = describeError(err);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final devices = ref.watch(monitorsProvider).devices;
    final systems = ref.watch(coreSettingsProvider).operatingSystems;
    final syslogSystems = systems.where((s) => s.supportsSyslog).toList();
    final allowMacTelnet = _hints?.layer2Reachable == true;
    final safeProtocol = (!allowMacTelnet && _protocol == 'mactelnet')
        ? 'ssh'
        : _protocol;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (devices.isNotEmpty)
          AppDropdownFormField<int>(
            initialValue: devices.any((d) => d.id == _selectedDeviceId)
                ? _selectedDeviceId
                : devices.first.id,
            decoration: const InputDecoration(labelText: 'Dispositivo'),
            items: <DropdownMenuItem<int>>[
              for (final d in devices)
                DropdownMenuItem<int>(
                  value: d.id,
                  child: Text(d.name, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) {
              if (v != null) {
                setState(() => _selectedDeviceId = v);
                _loadHints(v);
              }
            },
          ),
        const SizedBox(height: 12),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            AppDropdownFormField<String>(
              initialValue: safeProtocol,
              decoration: const InputDecoration(labelText: 'Protocolo'),
              items: <DropdownMenuItem<String>>[
                const DropdownMenuItem<String>(
                  value: 'ssh',
                  child: Text('SSH', overflow: TextOverflow.ellipsis),
                ),
                const DropdownMenuItem<String>(
                  value: 'telnet',
                  child: Text('Telnet', overflow: TextOverflow.ellipsis),
                ),
                if (allowMacTelnet)
                  const DropdownMenuItem<String>(
                    value: 'mactelnet',
                    child: Text('MAC-Telnet', overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: (v) {
                if (v != null) {
                  setState(() {
                    _protocol = v;
                    _portCtrl.text = switch (v) {
                      'telnet' => '23',
                      'mactelnet' => '20561',
                      _ => '22',
                    };
                  });
                }
              },
            ),
            TextField(
              controller: _portCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Porta'),
            ),
          ],
        ),
        if (safeProtocol == 'mactelnet') ...<Widget>[
          const SizedBox(height: 12),
          TextField(
            controller: _macCtrl,
            decoration: const InputDecoration(
              labelText: 'Endereço MAC (MAC-Telnet)',
              hintText: 'AA:BB:CC:DD:EE:FF',
            ),
          ),
        ],
        const SizedBox(height: 12),
        if (syslogSystems.isNotEmpty) ...<Widget>[
          AppDropdownFormField<String?>(
            initialValue: syslogSystems.any((s) => s.id == _osId)
                ? _osId
                : null,
            decoration: const InputDecoration(labelText: 'Sistema operacional'),
            items: <DropdownMenuItem<String?>>[
              const DropdownMenuItem<String?>(
                value: null,
                child: Text(
                  'Detectar automaticamente',
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              for (final s in syslogSystems)
                DropdownMenuItem<String?>(
                  value: s.id,
                  child: Text(s.label, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) => setState(() => _osId = v),
          ),
          const SizedBox(height: 12),
        ],
        TextField(
          controller: _addrCtrl,
          decoration: const InputDecoration(
            labelText: 'Endereço deste servidor (que o equipamento alcança)',
          ),
        ),
        const SizedBox(height: 12),
        if (_hints?.savedCredentials.isNotEmpty == true)
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _useSaved,
            onChanged: (v) => setState(() => _useSaved = v),
            title: const Text('Usar credencial salva do equipamento'),
          ),
        if (!_useSaved) ...<Widget>[
          TextField(
            controller: _userCtrl,
            decoration: const InputDecoration(labelText: 'Usuário'),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _passCtrl,
            obscureText: true,
            decoration: const InputDecoration(labelText: 'Senha'),
          ),
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            controlAffinity: ListTileControlAffinity.leading,
            value: _saveCred,
            onChanged: (v) => setState(() => _saveCred = v ?? false),
            title: const Text('Salvar credencial no cofre do equipamento'),
          ),
        ],
        if (_error != null) ...<Widget>[
          const SizedBox(height: 8),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (_result != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: _result!.confirmed == true
                ? SemanticTone.success
                : SemanticTone.info,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  _result!.confirmed == true
                      ? 'Syslog configurado e confirmado!'
                      : 'Comandos aplicados com sucesso.',
                  style: const TextStyle(fontWeight: FontWeight.w700),
                ),
                if (_result!.persistenceWarning != null) ...<Widget>[
                  const SizedBox(height: 4),
                  Text(_result!.persistenceWarning!),
                ],
              ],
            ),
          ),
          if (_result!.transcript.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            CodeViewer(
              title: 'Transcrição',
              code: _result!.transcript,
              maxHeight: 220,
            ),
          ],
        ],
        const SizedBox(height: 16),
        FilledButton.icon(
          onPressed: _running ? null : _applyProvision,
          icon: _running
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.play_arrow),
          label: const Text('Aplicar configuração'),
        ),
      ],
    );
  }
}

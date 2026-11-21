import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/plugins/data/plugins_repository.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Executa uma chamada e intercepta erro `{ code: "missing_credential", deviceId, transport }`:
/// se o usuário for admin, abre `DeviceAccessDialog` travado no transporte faltante e
/// reexecuta automaticamente (`retryAuto == true`) ou pede confirmação (`retryAuto == false`).
Future<T?> withDeviceAccess<T>({
  required BuildContext context,
  required WidgetRef ref,
  required Future<T> Function() action,
  bool retryAuto = true,
  String? actionLabel,
}) async {
  try {
    return await action();
  } on ApiException catch (err) {
    if (!err.isMissingCredential ||
        err.deviceId == null ||
        err.transport == null ||
        !ref.read(authProvider).canManagePlugins ||
        !context.mounted) {
      rethrow;
    }
    final saved = await showDeviceAccessDialog(
      context: context,
      ref: ref,
      deviceId: err.deviceId!,
      lockedTransport: err.transport,
      reason: err.message,
    );
    if (!saved || !context.mounted) return null;
    if (!retryAuto) {
      final confirmed = await showConfirmActionDialog(
        context: context,
        title: 'Executar novamente?',
        message: actionLabel != null
            ? 'Credencial salva. Deseja executar "$actionLabel" agora?'
            : 'Credencial salva. Deseja tentar a operação novamente?',
        confirmLabel: 'Executar',
        tone: SemanticTone.primary,
        icon: Icons.play_arrow_outlined,
      );
      if (!confirmed) return null;
    }
    return action();
  }
}

/// Abre o diálogo de cadastro de credenciais / acesso dedicado SSH (ADR 016).
/// Retorna `true` se o usuário salvou ou provisionou uma credencial.
Future<bool> showDeviceAccessDialog({
  required BuildContext context,
  required WidgetRef ref,
  required int deviceId,
  String? lockedTransport,
  String? reason,
}) async {
  final saved = await showAdaptiveFormSheet<bool>(
    context: context,
    title: 'Acesso ao equipamento',
    subtitle: lockedTransport != null
        ? 'Transporte necessário: ${lockedTransport.toUpperCase()}'
        : 'Configure SSH (chave dedicada ou sessão temporária), HTTP ou Telnet.',
    maxWidth: 560,
    builder: (_) => _DeviceAccessBody(
      deviceId: deviceId,
      lockedTransport: lockedTransport,
      reason: reason,
    ),
  );
  return saved == true;
}

class _DeviceAccessBody extends ConsumerStatefulWidget {
  const _DeviceAccessBody({
    required this.deviceId,
    this.lockedTransport,
    this.reason,
  });

  final int deviceId;
  final String? lockedTransport;
  final String? reason;

  @override
  ConsumerState<_DeviceAccessBody> createState() => _DeviceAccessBodyState();
}

class _DeviceAccessBodyState extends ConsumerState<_DeviceAccessBody> {
  DeviceAccessView? _access;
  bool _loading = true;
  bool _saving = false;
  String? _error;

  late String _kind;
  String _sshMode = 'ask'; // 'dedicated' | 'ask' | 'vault'
  bool _https = false;
  int? _viaProbeId;
  bool _obscureSecret = true;

  final _userCtrl = TextEditingController();
  final _secretCtrl = TextEditingController();
  final _portCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _kind = widget.lockedTransport ?? 'ssh';
    _load();
  }

  @override
  void dispose() {
    _userCtrl.dispose();
    _secretCtrl.dispose();
    _portCtrl.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final view = await ref
          .read(pluginsRepositoryProvider)
          .fetchAccess(widget.deviceId);
      if (!mounted) return;
      _access = view;
      _applyDefaultsForKind(_kind);
      setState(() => _loading = false);
    } catch (err) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = err.toString();
        });
      }
    }
  }

  void _applyDefaultsForKind(String kind) {
    final existing = _access?.credentials
        .where((c) => c.kind == kind)
        .firstOrNull;
    _userCtrl.text = existing?.username ?? (kind == 'ssh' ? 'admin' : '');
    _secretCtrl.clear();
    _portCtrl.text =
        (existing?.port ?? (kind == 'ssh' ? 22 : (kind == 'telnet' ? 23 : 80)))
            .toString();
    _https = existing?.https ?? false;
    _viaProbeId = existing?.viaProbeId;
    if (kind == 'ssh') {
      if (existing?.dedicated != null) {
        _sshMode = 'dedicated';
      } else if (_access?.dedicatedPlatform != null && existing == null) {
        _sshMode = 'dedicated';
      } else {
        _sshMode = existing?.storage ?? 'ask';
      }
    }
  }

  Future<void> _save() async {
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final port = int.tryParse(_portCtrl.text.trim());
      if (_kind == 'ssh' && _sshMode == 'dedicated') {
        await ref.read(pluginsProvider.notifier).provisionDedicated(
          widget.deviceId,
          <String, dynamic>{
            if (_access?.dedicatedPlatform != null)
              'platform': _access!.dedicatedPlatform,
            'username': _userCtrl.text.trim(),
            'password': _secretCtrl.text,
            'port': ?port,
            if (_viaProbeId != null) 'viaProbeId': _viaProbeId,
          },
        );
      } else {
        await ref.read(pluginsProvider.notifier).saveCredential(
          widget.deviceId,
          <String, dynamic>{
            'kind': _kind,
            'username': _userCtrl.text.trim(),
            if (_secretCtrl.text.isNotEmpty) 'secret': _secretCtrl.text,
            'storage': _kind == 'ssh' ? _sshMode : 'ask',
            'port': ?port,
            if (_kind == 'http') 'https': _https,
            if (_viaProbeId != null) 'viaProbeId': _viaProbeId,
          },
        );
      }
      if (mounted) Navigator.of(context).pop(true);
    } catch (err) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = err.toString();
        });
      }
    }
  }

  Future<void> _rotateDedicated() async {
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref.read(pluginsProvider.notifier).rotateDedicated(widget.deviceId);
      await _load();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Chave SSH dedicada rotacionada.')),
        );
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _revokeDedicated({required bool localOnly}) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: localOnly
          ? 'Remover credencial localmente?'
          : 'Revogar acesso dedicado no equipamento?',
      message: localOnly
          ? 'Remove a chave apenas no NetMonitor (use se o equipamento estiver fora de alcance).'
          : 'Remove o usuário/chave dedicados no equipamento e apaga a credencial local.',
      confirmLabel: 'Revogar',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final leftovers = await ref
          .read(pluginsProvider.notifier)
          .revokeDedicated(widget.deviceId, localOnly: localOnly);
      if (!mounted) return;
      if (leftovers.isNotEmpty) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Pendências: ${leftovers.join(', ')}')),
        );
      }
      await _load();
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(
        message: 'Carregando acessos do equipamento...',
      );
    }

    final existingSsh = _access?.credentials
        .where((c) => c.kind == 'ssh')
        .firstOrNull;
    final dedicated = existingSsh?.dedicated;
    final agents = (_access?.agents ?? const <AgentRouteOption>[])
        .where((a) => a.hasDeviceIo)
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        if (widget.reason != null && widget.reason!.isNotEmpty) ...<Widget>[
          NestedPanel(tone: SemanticTone.warning, child: Text(widget.reason!)),
          const SizedBox(height: 12),
        ],
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 12),
        ],
        if (widget.lockedTransport == null) ...<Widget>[
          SegmentedButton<String>(
            selected: <String>{_kind},
            onSelectionChanged: (sel) {
              setState(() {
                _kind = sel.first;
                _applyDefaultsForKind(_kind);
              });
            },
            segments: const <ButtonSegment<String>>[
              ButtonSegment<String>(value: 'ssh', label: Text('SSH')),
              ButtonSegment<String>(value: 'http', label: Text('HTTP')),
              ButtonSegment<String>(value: 'telnet', label: Text('Telnet')),
            ],
          ),
          const SizedBox(height: 12),
        ],
        if (_kind == 'ssh' && dedicated != null) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.plugin,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    const SemanticChip(
                      label: 'Chave dedicada ativa',
                      tone: SemanticTone.plugin,
                      icon: Icons.vpn_key_outlined,
                      compact: true,
                    ),
                    const Spacer(),
                    if (dedicated.rotatedAt != null)
                      Text(
                        'Rotacionada ${formatRelativeTime(dedicated.rotatedAt)}',
                        style: context.textTheme.labelSmall,
                      ),
                  ],
                ),
                const SizedBox(height: 8),
                KeyValueRow(
                  label: 'Usuário dedicado',
                  value: dedicated.username,
                ),
                if (dedicated.fingerprint != null)
                  KeyValueRow(
                    label: 'Fingerprint',
                    value: dedicated.fingerprint!,
                    monospace: true,
                  ),
                if (dedicated.hostKey != null)
                  KeyValueRow(
                    label: 'Host key',
                    value: dedicated.hostKey!,
                    monospace: true,
                  ),
                const SizedBox(height: 10),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    FilledButton.tonalIcon(
                      onPressed: _saving ? null : _rotateDedicated,
                      icon: const Icon(Icons.key_outlined, size: 17),
                      label: const Text('Rotacionar chave'),
                    ),
                    OutlinedButton(
                      onPressed: _saving
                          ? null
                          : () => _revokeDedicated(localOnly: false),
                      child: const Text('Revogar'),
                    ),
                    TextButton(
                      onPressed: _saving
                          ? null
                          : () => _revokeDedicated(localOnly: true),
                      child: const Text('Apagar só aqui'),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ] else ...<Widget>[
          if (_kind == 'ssh') ...<Widget>[
            SegmentedButton<String>(
              selected: <String>{_sshMode},
              onSelectionChanged: (sel) => setState(() => _sshMode = sel.first),
              segments: <ButtonSegment<String>>[
                if (_access?.dedicatedPlatform != null)
                  const ButtonSegment<String>(
                    value: 'dedicated',
                    label: Text('Chave dedicada'),
                  ),
                const ButtonSegment<String>(
                  value: 'ask',
                  label: Text('Sessão 30 min'),
                ),
                const ButtonSegment<String>(
                  value: 'vault',
                  label: Text('Cofre'),
                ),
              ],
            ),
            const SizedBox(height: 10),
            if (_sshMode == 'dedicated')
              const NestedPanel(
                tone: SemanticTone.info,
                child: Text(
                  'A senha de administrador é usada uma única vez para criar um usuário/chave dedicados no equipamento e nunca é armazenada.',
                ),
              ),
            const SizedBox(height: 10),
          ],
          TextField(
            controller: _userCtrl,
            decoration: const InputDecoration(labelText: 'Usuário *'),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _secretCtrl,
            obscureText: _obscureSecret,
            decoration: InputDecoration(
              labelText: _kind == 'ssh' && _sshMode == 'dedicated'
                  ? 'Senha de administrador (uso único) *'
                  : 'Senha / Segredo',
              suffixIcon: IconButton(
                icon: Icon(
                  _obscureSecret ? Icons.visibility : Icons.visibility_off,
                ),
                onPressed: () =>
                    setState(() => _obscureSecret = !_obscureSecret),
              ),
            ),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _portCtrl,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(labelText: 'Porta'),
          ),
          if (_kind == 'http') ...<Widget>[
            const SizedBox(height: 6),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Usar HTTPS'),
              value: _https,
              onChanged: (v) => setState(() => _https = v),
            ),
          ],
          if (agents.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            DropdownButtonFormField<int?>(
              initialValue: _viaProbeId,
              decoration: const InputDecoration(
                labelText: 'Rota de acesso (agente com device_io)',
              ),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text('Direto pela central'),
                ),
                for (final agent in agents)
                  DropdownMenuItem<int?>(
                    value: agent.probeId,
                    child: Text(
                      '${agent.name}${agent.online ? '' : ' (offline)'}',
                    ),
                  ),
              ],
              onChanged: (v) => setState(() => _viaProbeId = v),
            ),
          ],
          const SizedBox(height: 16),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: <Widget>[
              TextButton(
                onPressed: _saving
                    ? null
                    : () => Navigator.of(context).pop(false),
                child: const Text('Cancelar'),
              ),
              const SizedBox(width: 8),
              FilledButton.icon(
                onPressed: _saving ? null : _save,
                icon: _saving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.save_outlined, size: 18),
                label: Text(
                  _kind == 'ssh' && _sshMode == 'dedicated'
                      ? 'Provisionar chave dedicada'
                      : 'Salvar acesso',
                ),
              ),
            ],
          ),
        ],
      ],
    );
  }
}

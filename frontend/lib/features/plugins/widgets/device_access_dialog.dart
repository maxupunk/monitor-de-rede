import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/device_defaults.dart';
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
  String _sshMode =
      'ask'; // 'dedicated' | 'ask' (Rust SecretStorage: ask | key)
  bool _https = false;
  int? _viaProbeId;
  bool _obscureSecret = true;

  final _userCtrl = TextEditingController();
  final _secretCtrl = TextEditingController();
  final _portCtrl = TextEditingController();
  bool _userEditedUsername = false;

  String? get _effectivePlatform =>
      _access?.dedicatedPlatform ??
      _access?.credentials
          .where((c) => c.dedicated?.platform != null)
          .map((c) => c.dedicated!.platform)
          .firstOrNull ??
      ref.read(pluginsProvider).deviceViews[widget.deviceId]?.platform;

  @override
  void initState() {
    super.initState();
    _kind = widget.lockedTransport ?? 'ssh';
    _userCtrl.text = _kind == 'ssh' ? defaultDeviceAdminUsername(null) : '';
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
          _error = _formatError(err);
        });
      }
    }
  }

  void _applyDefaultsForKind(String kind) {
    final existing = _access?.credentials
        .where((c) => c.kind == kind)
        .firstOrNull;
    if (!_userEditedUsername) {
      final savedUsername = existing?.username.trim() ?? '';
      if (savedUsername.isNotEmpty) {
        _userCtrl.text = savedUsername;
      } else if (kind == 'ssh' || kind == 'telnet') {
        _userCtrl.text = defaultDeviceAdminUsername(_effectivePlatform);
      } else {
        _userCtrl.text = '';
      }
    }
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
        _sshMode = 'ask';
      }
    }
  }

  String _formatError(Object err) {
    if (isDeviceAuthFailure(err)) {
      return formatDeviceAuthFailureMessage(
        err,
        platformOrOs: _effectivePlatform,
      );
    }
    if (err is ApiException) {
      return err.message.isNotEmpty
          ? err.message
          : 'Erro ao comunicar com o servidor.';
    }
    return err.toString();
  }

  Future<void> _showAuthFailureDialog(Object err) {
    final message = formatDeviceAuthFailureMessage(
      err,
      platformOrOs: _effectivePlatform,
    );
    return showDialog<void>(
      context: context,
      builder: (alertCtx) => AlertDialog(
        title: Row(
          children: <Widget>[
            Icon(Icons.lock_person_outlined, color: alertCtx.colorScheme.error),
            const SizedBox(width: 10),
            const Expanded(child: Text('Usuário ou senha incorretos')),
          ],
        ),
        content: Text(message),
        actions: <Widget>[
          FilledButton(
            onPressed: () => Navigator.of(alertCtx).pop(),
            child: const Text('Entendi'),
          ),
        ],
      ),
    );
  }

  Future<void> _save() async {
    final username = _userCtrl.text.trim();
    if (username.isEmpty) {
      setState(() => _error = 'Informe o usuário de acesso.');
      return;
    }
    final isDedicatedProvision =
        _kind == 'ssh' &&
        _sshMode == 'dedicated' &&
        _access?.dedicatedPlatform != null;
    if (isDedicatedProvision && _secretCtrl.text.isEmpty) {
      setState(
        () => _error =
            'Informe a senha de administrador (uso único) para provisionar a chave SSH.',
      );
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final port = int.tryParse(_portCtrl.text.trim());
      if (isDedicatedProvision) {
        // DTO Rust `ProvisionInput` (POST /devices/{id}/access/dedicated)
        await ref
            .read(pluginsProvider.notifier)
            .provisionDedicated(widget.deviceId, <String, dynamic>{
              if (_access?.dedicatedPlatform != null)
                'platform': _access!.dedicatedPlatform,
              'username': username,
              'password': _secretCtrl.text,
              'port': ?port,
              if (_viaProbeId != null) 'viaProbeId': _viaProbeId,
            });
      } else {
        // DTO Rust `CredentialInput` (PUT /devices/{id}/credentials)
        // SecretStorage aceita somente 'ask' (sessão 30 min) ou 'key' (via provisionDedicated).
        await ref
            .read(pluginsProvider.notifier)
            .saveCredential(widget.deviceId, <String, dynamic>{
              'kind': _kind,
              'username': username,
              if (_secretCtrl.text.isNotEmpty) 'secret': _secretCtrl.text,
              'storage': 'ask',
              'port': ?port,
              if (_kind == 'http') 'https': _https,
              if (_viaProbeId != null) 'viaProbeId': _viaProbeId,
            });
      }
      if (mounted) Navigator.of(context).pop(true);
    } catch (err) {
      if (!mounted) return;
      final formatted = _formatError(err);
      setState(() {
        _saving = false;
        _error = formatted;
      });
      if (isDeviceAuthFailure(err)) {
        await _showAuthFailureDialog(err);
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
      if (mounted) setState(() => _error = _formatError(err));
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
      if (mounted) setState(() => _error = _formatError(err));
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
            showSelectedIcon: false,
            style: SegmentedButton.styleFrom(
              visualDensity: VisualDensity.compact,
              padding: const EdgeInsets.symmetric(horizontal: 10),
            ),
            selected: <String>{_kind},
            onSelectionChanged: (sel) {
              setState(() {
                _kind = sel.first;
                _userEditedUsername = false;
                _applyDefaultsForKind(_kind);
              });
            },
            segments: const <ButtonSegment<String>>[
              ButtonSegment<String>(
                value: 'ssh',
                label: Text('SSH', maxLines: 1, softWrap: false),
              ),
              ButtonSegment<String>(
                value: 'http',
                label: Text('HTTP', maxLines: 1, softWrap: false),
              ),
              ButtonSegment<String>(
                value: 'telnet',
                label: Text('Telnet', maxLines: 1, softWrap: false),
              ),
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
                Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    const SemanticChip(
                      label: 'Chave dedicada ativa',
                      tone: SemanticTone.plugin,
                      icon: Icons.vpn_key_outlined,
                      compact: true,
                    ),
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
                  value: dedicated.username.isNotEmpty
                      ? dedicated.username
                      : (existingSsh?.username ?? 'netmonitor'),
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
                SymmetricalActionBar(
                  children: <Widget>[
                    FilledButton.tonalIcon(
                      onPressed: _saving ? null : _rotateDedicated,
                      icon: const Icon(Icons.key_outlined, size: 17),
                      label: const Text(
                        'Rotacionar chave',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    OutlinedButton(
                      style: OutlinedButton.styleFrom(
                        foregroundColor: context.colorScheme.error,
                        side: BorderSide(color: context.colorScheme.error),
                      ),
                      onPressed: _saving
                          ? null
                          : () => _revokeDedicated(localOnly: false),
                      child: const Text(
                        'Revogar',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    OutlinedButton(
                      onPressed: _saving
                          ? null
                          : () => _revokeDedicated(localOnly: true),
                      child: const Text(
                        'Apagar só aqui',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ] else ...<Widget>[
          if (_kind == 'ssh' && _access?.dedicatedPlatform != null) ...<Widget>[
            ChoiceCard(
              title: 'Chave SSH dedicada (${_access!.dedicatedPlatform})',
              subtitle:
                  'Cria chave ed25519 exclusiva no equipamento usando a senha admin uma única vez (nunca salva).',
              icon: Icons.vpn_key_outlined,
              badge: 'Recomendado',
              badgeTone: SemanticTone.success,
              tone: SemanticTone.plugin,
              selected: _sshMode == 'dedicated',
              onTap: () => setState(() => _sshMode = 'dedicated'),
            ),
            const SizedBox(height: 8),
            ChoiceCard(
              title: 'Usuário e senha (sessão 30 min)',
              subtitle:
                  'Mantém a credencial apenas em memória por 30 minutos (nunca gravada no banco).',
              icon: Icons.schedule,
              tone: SemanticTone.primary,
              selected: _sshMode != 'dedicated',
              onTap: () => setState(() => _sshMode = 'ask'),
            ),
            const SizedBox(height: 12),
          ],
          ResponsiveFieldRow(
            breakpoint: 480,
            spacing: 10,
            runSpacing: 10,
            children: <Widget>[
              TextField(
                controller: _userCtrl,
                onChanged: (_) => _userEditedUsername = true,
                decoration: InputDecoration(
                  labelText:
                      _kind == 'ssh' &&
                          _sshMode == 'dedicated' &&
                          _access?.dedicatedPlatform != null
                      ? 'Usuário admin atual *'
                      : 'Usuário *',
                ),
              ),
              TextField(
                controller: _portCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Porta'),
              ),
            ],
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _secretCtrl,
            obscureText: _obscureSecret,
            decoration: InputDecoration(
              labelText:
                  _kind == 'ssh' &&
                      _sshMode == 'dedicated' &&
                      _access?.dedicatedPlatform != null
                  ? 'Senha admin (uso único) *'
                  : 'Senha / Segredo (sessão 30 min)',
              suffixIcon: IconButton(
                icon: Icon(
                  _obscureSecret ? Icons.visibility : Icons.visibility_off,
                ),
                onPressed: () =>
                    setState(() => _obscureSecret = !_obscureSecret),
              ),
            ),
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
            AppDropdownFormField<int?>(
              initialValue: _viaProbeId,
              decoration: const InputDecoration(
                labelText: 'Rota de acesso (agente com device_io)',
              ),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text(
                    'Direto pela central',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                for (final agent in agents)
                  DropdownMenuItem<int?>(
                    value: agent.probeId,
                    child: Text(
                      '${agent.name}${agent.online ? '' : ' (offline)'}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
              ],
              onChanged: (v) => setState(() => _viaProbeId = v),
            ),
          ],
          const SizedBox(height: 16),
          LayoutBuilder(
            builder: (context, constraints) {
              final isDedicatedProvision =
                  _kind == 'ssh' &&
                  _sshMode == 'dedicated' &&
                  _access?.dedicatedPlatform != null;
              final cancelBtn = OutlinedButton(
                onPressed: _saving
                    ? null
                    : () => Navigator.of(context).pop(false),
                child: const Text(
                  'Cancelar',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              );
              final submitBtn = FilledButton.icon(
                style: isDedicatedProvision
                    ? FilledButton.styleFrom(
                        backgroundColor: context.semanticColors.plugin,
                        foregroundColor: context.semanticColors.onPlugin,
                      )
                    : null,
                onPressed: _saving ? null : _save,
                icon: _saving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(
                        isDedicatedProvision
                            ? Icons.vpn_key_outlined
                            : Icons.save_outlined,
                        size: 18,
                      ),
                label: Text(
                  isDedicatedProvision
                      ? 'Provisionar chave dedicada'
                      : 'Salvar acesso',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              );

              if (isDedicatedProvision && constraints.maxWidth < 430) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    submitBtn,
                    const SizedBox(height: 8),
                    cancelBtn,
                  ],
                );
              }
              return SymmetricalActionBar(
                children: <Widget>[cancelBtn, submitBtn],
              );
            },
          ),
        ],
      ],
    );
  }
}

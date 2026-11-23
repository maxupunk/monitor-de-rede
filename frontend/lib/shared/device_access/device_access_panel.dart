import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/clipboard_utils.dart';
import 'package:netmonitor_app/core/utils/device_defaults.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/shared/device_access/device_access_controller.dart';
import 'package:netmonitor_app/shared/device_access/device_access_dialogs.dart';
import 'package:netmonitor_app/shared/device_access/device_access_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Painel reutilizável de credenciais e chave SSH dedicada (ADR 016),
/// usado tanto em modais (`showDeviceAccessDialog` / `DeviceAccessHost`)
/// quanto embutido na aba Acesso do detalhe de dispositivo.
class DeviceAccessPanel extends ConsumerStatefulWidget {
  const DeviceAccessPanel({
    super.key,
    required this.deviceId,
    this.deviceName,
    this.platformHint,
    this.initialTransport,
    this.lockedTransport,
    this.bannerMessage,
    this.reason,
    this.onSaved,
  });

  final int deviceId;
  final String? deviceName;
  final String? platformHint;
  final String? initialTransport;
  final String? lockedTransport;
  final String? bannerMessage;
  final String? reason;
  final VoidCallback? onSaved;

  @override
  ConsumerState<DeviceAccessPanel> createState() => _DeviceAccessPanelState();
}

class _DeviceAccessPanelState extends ConsumerState<DeviceAccessPanel> {
  DeviceAccessViewModel? _view;
  bool _loading = true;
  bool _saving = false;
  bool _dedicatedBusy = false;
  String? _error;

  late String _transport;
  String? _accessMode; // 'dedicated' | 'password'
  bool _useHttps = false;
  int? _viaProbeId;
  bool _obscureAdminPass = true;
  bool _obscureSessionPass = true;
  bool _adminUserEdited = false;
  bool _standardUserEdited = false;

  final TextEditingController _usernameCtrl = TextEditingController();
  final TextEditingController _passwordCtrl = TextEditingController();
  final TextEditingController _portCtrl = TextEditingController(text: '22');
  final TextEditingController _adminUserCtrl = TextEditingController();
  final TextEditingController _adminPassCtrl = TextEditingController();
  final TextEditingController _dedicatedPortCtrl = TextEditingController(
    text: '22',
  );

  String? get _effectiveBanner => widget.bannerMessage ?? widget.reason;

  String? _resolvePlatformHint([DeviceAccessViewModel? view]) {
    final target = view ?? _view;
    final candidates = <String?>[
      target?.dedicatedPlatform,
      target?.platformHint,
      widget.platformHint,
      widget.deviceName,
    ];
    final joined = candidates
        .whereType<String>()
        .map((e) => e.trim())
        .where((e) => e.isNotEmpty)
        .join(' ');
    return joined.isEmpty ? null : joined;
  }

  @override
  void initState() {
    super.initState();
    _transport = (widget.lockedTransport ?? widget.initialTransport ?? 'ssh')
        .toLowerCase();
    final initialUser = defaultDeviceAdminUsername(_resolvePlatformHint());
    _adminUserCtrl.text = initialUser;
    _usernameCtrl.text = initialUser;
    _portCtrl.text = '${defaultTransportPort(_transport)}';
    _load();
  }

  @override
  void dispose() {
    _usernameCtrl.dispose();
    _passwordCtrl.dispose();
    _portCtrl.dispose();
    _adminUserCtrl.dispose();
    _adminPassCtrl.dispose();
    _dedicatedPortCtrl.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final view = await ref
          .read(deviceAccessProvider.notifier)
          .fetchAccess(widget.deviceId);
      if (!mounted) return;
      final hint = _resolvePlatformHint(view);
      final defaultUser = defaultDeviceAdminUsername(hint);
      setState(() {
        _view = view;
        _loading = false;
        if (!_adminUserEdited) {
          _adminUserCtrl.text = defaultUser;
        }
        final hasDedicated = view.dedicatedCredential != null;
        if (_accessMode == null) {
          final canUseDedicated =
              (_transport == 'ssh') &&
              (view.supportsDedicatedKey || hasDedicated);
          _accessMode = canUseDedicated ? 'dedicated' : 'password';
        }
        _prefillForTransport(_transport);
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = formatDeviceAccessError(
          err,
          platformOrOs: _resolvePlatformHint(),
        );
      });
    }
  }

  void _prefillForTransport(String kind) {
    final existing = _view?.credentialFor(kind);
    if (existing != null) {
      if (!_standardUserEdited && existing.username.isNotEmpty) {
        _usernameCtrl.text = existing.username;
      }
      _portCtrl.text =
          '${existing.port ?? defaultTransportPort(kind, https: existing.https)}';
      _useHttps = existing.https;
      _viaProbeId = existing.viaProbeId;
    } else {
      if (!_standardUserEdited) {
        _usernameCtrl.text = defaultDeviceAdminUsername(_resolvePlatformHint());
      }
      _useHttps = false;
      _portCtrl.text = '${defaultTransportPort(kind)}';
    }
  }

  Future<void> _saveStandard() async {
    if (_saving) return;
    final user = _usernameCtrl.text.trim();
    if (user.isEmpty) {
      setState(() => _error = 'Informe o nome de usuário do equipamento.');
      return;
    }
    final existingCred = _view?.credentialFor(_transport);
    final hasActiveSession = existingCred?.sessionActive == true;
    if (_transport != 'http' &&
        _passwordCtrl.text.isEmpty &&
        (!hasActiveSession || widget.lockedTransport != null)) {
      setState(() => _error = 'Informe a senha de acesso do equipamento.');
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref
          .read(deviceAccessProvider.notifier)
          .saveSessionCredential(
            deviceId: widget.deviceId,
            kind: _transport,
            username: user,
            secret: _passwordCtrl.text,
            port: int.tryParse(_portCtrl.text.trim()),
            https: _useHttps,
            viaProbeId: _viaProbeId,
          );
      _passwordCtrl.clear();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Credencial salva com sucesso.')),
      );
      await _load();
      if (!mounted) return;
      widget.onSaved?.call();
    } catch (err) {
      if (!mounted) return;
      final hint = _resolvePlatformHint();
      setState(() {
        _saving = false;
        _error = formatDeviceAccessError(err, platformOrOs: hint);
      });
      if (isDeviceAuthFailure(err)) {
        await showDeviceAuthFailureDialog(context, err, platformOrOs: hint);
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _deleteCredential(String kind) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Remover credencial ${kind.toUpperCase()}?',
      message: 'A credencial deste transporte será removida do equipamento.',
      confirmLabel: 'Remover',
      tone: SemanticTone.error,
    );
    if (!ok || !mounted) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref
          .read(deviceAccessProvider.notifier)
          .deleteCredential(deviceId: widget.deviceId, kind: kind);
      if (!mounted) return;
      await _load();
    } catch (err) {
      if (mounted) {
        setState(
          () => _error = formatDeviceAccessError(
            err,
            platformOrOs: _resolvePlatformHint(),
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _provisionDedicated() async {
    final platform = _view?.dedicatedPlatform;
    if (platform == null || platform.isEmpty) {
      setState(
        () => _error =
            'Plataforma dedicada indisponível para este equipamento. Use usuário e senha.',
      );
      return;
    }
    if (_adminUserCtrl.text.trim().isEmpty || _adminPassCtrl.text.isEmpty) {
      setState(
        () => _error =
            'Informe o usuário e a senha administrativa atual para provisionar a chave SSH.',
      );
      return;
    }
    setState(() {
      _dedicatedBusy = true;
      _error = null;
    });
    try {
      await ref
          .read(deviceAccessProvider.notifier)
          .provisionDedicatedKey(
            deviceId: widget.deviceId,
            platform: platform,
            username: _adminUserCtrl.text.trim(),
            password: _adminPassCtrl.text,
            port: int.tryParse(_dedicatedPortCtrl.text.trim()) ?? 22,
            viaProbeId: _viaProbeId,
          );
      _adminPassCtrl.clear();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Chave SSH dedicada provisionada no equipamento.'),
        ),
      );
      await _load();
      if (!mounted) return;
      widget.onSaved?.call();
    } catch (err) {
      if (!mounted) return;
      final hint = _resolvePlatformHint();
      setState(() {
        _dedicatedBusy = false;
        _error = formatDeviceAccessError(err, platformOrOs: hint);
      });
      if (isDeviceAuthFailure(err)) {
        await showDeviceAuthFailureDialog(context, err, platformOrOs: hint);
      }
    } finally {
      if (mounted) setState(() => _dedicatedBusy = false);
    }
  }

  Future<void> _rotateDedicated() async {
    setState(() {
      _dedicatedBusy = true;
      _error = null;
    });
    try {
      await ref
          .read(deviceAccessProvider.notifier)
          .rotateDedicatedKey(widget.deviceId);
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Chave SSH rotacionada.')));
      await _load();
    } catch (err) {
      if (!mounted) return;
      final hint = _resolvePlatformHint();
      setState(() {
        _dedicatedBusy = false;
        _error = formatDeviceAccessError(err, platformOrOs: hint);
      });
      if (isDeviceAuthFailure(err)) {
        await showDeviceAuthFailureDialog(context, err, platformOrOs: hint);
      }
    } finally {
      if (mounted) setState(() => _dedicatedBusy = false);
    }
  }

  Future<void> _revokeDedicated({required bool localOnly}) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: localOnly
          ? 'Remover chave apenas na central?'
          : 'Revogar acesso dedicado no equipamento?',
      message: localOnly
          ? 'A chave será esquecida no NetMonitor sem conectar ao equipamento.'
          : 'O NetMonitor removerá a conta dedicada diretamente no equipamento e descartará a chave privada.',
      confirmLabel: localOnly ? 'Remover local' : 'Revogar',
      tone: SemanticTone.error,
    );
    if (!ok || !mounted) return;
    setState(() {
      _dedicatedBusy = true;
      _error = null;
    });
    try {
      await ref
          .read(deviceAccessProvider.notifier)
          .revokeDedicatedKey(widget.deviceId, localOnly: localOnly);
      if (!mounted) return;
      await _load();
    } catch (err) {
      if (mounted) {
        setState(
          () => _error = formatDeviceAccessError(
            err,
            platformOrOs: _resolvePlatformHint(),
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _dedicatedBusy = false);
    }
  }

  Widget _buildAgentRouteField(List<AgentRouteOption> agents) {
    final validAgentIds = agents.map((a) => a.id).toSet();
    final safeViaProbe =
        (_viaProbeId != null && validAgentIds.contains(_viaProbeId))
        ? _viaProbeId
        : null;
    return AppDropdownFormField<int?>(
      initialValue: safeViaProbe,
      decoration: const InputDecoration(labelText: 'Rota de acesso'),
      items: <DropdownMenuItem<int?>>[
        const DropdownMenuItem<int?>(
          value: null,
          child: Text('Direto pela central', overflow: TextOverflow.ellipsis),
        ),
        for (final a in agents)
          DropdownMenuItem<int?>(
            value: a.id,
            child: Text(
              'Via agente: ${a.name}${a.online ? '' : ' (offline)'}',
              overflow: TextOverflow.ellipsis,
            ),
          ),
      ],
      onChanged: (v) => setState(() => _viaProbeId = v),
    );
  }

  @override
  Widget build(BuildContext context) {
    final canManage = ref.watch(authProvider).canManagePlugins;
    if (_loading) {
      return const LoadingStateView(
        message: 'Consultando métodos de acesso...',
      );
    }

    final credentials = _view?.credentials ?? const <DeviceCredentialView>[];
    final agents = _view?.eligibleAgents ?? const <AgentRouteOption>[];
    final dedicatedCred = _view?.dedicatedCredential;
    final hasDedicated = dedicatedCred != null;
    final supportsDedicated =
        (_transport == 'ssh') &&
        ((_view?.supportsDedicatedKey ?? false) || hasDedicated);
    final effectiveMode = supportsDedicated
        ? (_accessMode ?? 'dedicated')
        : 'password';
    final isLocked = widget.lockedTransport != null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (_effectiveBanner != null &&
            _effectiveBanner!.trim().isNotEmpty) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.warning,
            child: Row(
              children: <Widget>[
                const Icon(Icons.lock_outline, size: 20),
                const SizedBox(width: 10),
                Expanded(child: Text(_effectiveBanner!)),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 12),
        ],
        if (credentials.isNotEmpty) ...<Widget>[
          Text(
            'Credenciais registradas',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          for (final c in credentials)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: NestedPanel(
                padding: const EdgeInsets.all(10),
                child: Row(
                  children: <Widget>[
                    SemanticChip(
                      label: c.kind.toUpperCase(),
                      tone: c.dedicated
                          ? SemanticTone.plugin
                          : SemanticTone.primary,
                      compact: true,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            c.username.isNotEmpty
                                ? c.username
                                : (c.dedicated
                                      ? 'Chave SSH dedicada'
                                      : 'Configurado'),
                            style: const TextStyle(fontWeight: FontWeight.w700),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          Text(
                            c.dedicated
                                ? 'Chave Ed25519${c.effectiveFingerprint != null ? ' · ${c.effectiveFingerprint}' : ''}'
                                : c.sessionActive && c.sessionExpiresAt != null
                                ? 'Sessão ativa até ${formatClockTime(c.sessionExpiresAt)}'
                                : 'Porta ${c.port ?? defaultTransportPort(c.kind, https: c.https)} · ${c.storage}',
                            style: context.textTheme.bodySmall?.copyWith(
                              color: context.colorScheme.onSurfaceVariant,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ),
                    ),
                    if (c.dedicatedInfo?.publicKey != null &&
                        c.dedicatedInfo!.publicKey!.isNotEmpty)
                      IconButton(
                        tooltip: 'Copiar chave pública',
                        visualDensity: VisualDensity.compact,
                        icon: const Icon(Icons.copy, size: 16),
                        onPressed: () => copyToClipboardWithFeedback(
                          context,
                          c.dedicatedInfo!.publicKey!,
                          message: 'Chave pública copiada.',
                        ),
                      ),
                    SemanticChip(
                      label: (c.dedicated || c.sessionActive)
                          ? 'Ativo'
                          : 'Salvo',
                      tone: (c.dedicated || c.sessionActive)
                          ? SemanticTone.success
                          : SemanticTone.info,
                      compact: true,
                    ),
                    if (canManage && !c.dedicated) ...<Widget>[
                      const SizedBox(width: 4),
                      IconButton(
                        tooltip: 'Remover credencial',
                        visualDensity: VisualDensity.compact,
                        icon: Icon(
                          Icons.delete_outline,
                          size: 18,
                          color: context.colorScheme.error,
                        ),
                        onPressed: _saving
                            ? null
                            : () => _deleteCredential(c.kind),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          const SizedBox(height: 8),
        ],
        if (!canManage)
          const NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              'Apenas administradores podem cadastrar ou rotacionar credenciais de equipamentos.',
            ),
          )
        else ...<Widget>[
          if (!isLocked) ...<Widget>[
            SegmentedButton<String>(
              showSelectedIcon: false,
              segments: const <ButtonSegment<String>>[
                ButtonSegment<String>(
                  value: 'ssh',
                  label: Text('SSH', maxLines: 1, softWrap: false),
                  icon: Icon(Icons.terminal, size: 16),
                ),
                ButtonSegment<String>(
                  value: 'http',
                  label: Text('HTTP', maxLines: 1, softWrap: false),
                  icon: Icon(Icons.http, size: 16),
                ),
                ButtonSegment<String>(
                  value: 'telnet',
                  label: Text('Telnet', maxLines: 1, softWrap: false),
                  icon: Icon(Icons.cable, size: 16),
                ),
              ],
              selected: <String>{_transport},
              onSelectionChanged: (sel) {
                setState(() {
                  _transport = sel.first;
                  _error = null;
                  _prefillForTransport(_transport);
                });
              },
            ),
            const SizedBox(height: 12),
          ],
          if (supportsDedicated) ...<Widget>[
            SegmentedButton<String>(
              showSelectedIcon: false,
              segments: const <ButtonSegment<String>>[
                ButtonSegment<String>(
                  value: 'dedicated',
                  icon: Icon(Icons.vpn_key_outlined, size: 16),
                  label: Text(
                    'Chave SSH',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                ButtonSegment<String>(
                  value: 'password',
                  icon: Icon(Icons.password_outlined, size: 16),
                  label: Text(
                    'Usuário e senha',
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
              selected: <String>{effectiveMode},
              onSelectionChanged: (sel) {
                setState(() {
                  _accessMode = sel.first;
                  _error = null;
                });
              },
            ),
            const SizedBox(height: 12),
          ],
          if (effectiveMode == 'dedicated') ...<Widget>[
            NestedPanel(
              tone: SemanticTone.plugin,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Wrap(
                    spacing: 8,
                    runSpacing: 6,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      const Icon(Icons.vpn_key_outlined, size: 18),
                      Text(
                        'Chave SSH dedicada (${_view?.dedicatedPlatform ?? dedicatedCred?.dedicatedInfo?.platform ?? 'SSH'})',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      SemanticChip(
                        label: hasDedicated ? 'Provisionado' : 'Recomendado',
                        tone: hasDedicated
                            ? SemanticTone.success
                            : SemanticTone.plugin,
                        compact: true,
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Cria conta dedicada por chave SSH no equipamento sem guardar a senha administrativa.',
                    style: context.textTheme.bodySmall,
                  ),
                  const SizedBox(height: 12),
                  if (!hasDedicated) ...<Widget>[
                    ResponsiveFieldRow(
                      breakpoint: 480,
                      children: <Widget>[
                        TextField(
                          controller: _adminUserCtrl,
                          onChanged: (_) => _adminUserEdited = true,
                          decoration: InputDecoration(
                            labelText: 'Usuário admin atual',
                            hintText:
                                'Ex.: ${defaultDeviceAdminUsername(_resolvePlatformHint())}',
                          ),
                        ),
                        TextField(
                          controller: _dedicatedPortCtrl,
                          keyboardType: TextInputType.number,
                          decoration: const InputDecoration(
                            labelText: 'Porta SSH',
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),
                    TextField(
                      controller: _adminPassCtrl,
                      obscureText: _obscureAdminPass,
                      decoration: InputDecoration(
                        labelText: 'Senha administrativa (uso único)',
                        helperText:
                            'Usada uma única vez para instalar a chave e descartada em seguida.',
                        suffixIcon: IconButton(
                          tooltip: _obscureAdminPass
                              ? 'Mostrar senha'
                              : 'Ocultar senha',
                          icon: Icon(
                            _obscureAdminPass
                                ? Icons.visibility_outlined
                                : Icons.visibility_off_outlined,
                          ),
                          onPressed: () => setState(
                            () => _obscureAdminPass = !_obscureAdminPass,
                          ),
                        ),
                      ),
                    ),
                    if (agents.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 10),
                      _buildAgentRouteField(agents),
                    ],
                    const SizedBox(height: 12),
                    SymmetricalActionBar(
                      children: <Widget>[
                        FilledButton.icon(
                          onPressed: _dedicatedBusy
                              ? null
                              : _provisionDedicated,
                          icon: _dedicatedBusy
                              ? const SizedBox(
                                  width: 16,
                                  height: 16,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.key),
                          label: const Text('Provisionar chave SSH'),
                        ),
                      ],
                    ),
                  ] else ...<Widget>[
                    SymmetricalActionBar(
                      children: <Widget>[
                        OutlinedButton.icon(
                          onPressed: _dedicatedBusy ? null : _rotateDedicated,
                          icon: const Icon(Icons.autorenew),
                          label: const Text(
                            'Rotacionar chave',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        OutlinedButton.icon(
                          onPressed: _dedicatedBusy
                              ? null
                              : () => _revokeDedicated(localOnly: false),
                          icon: const Icon(Icons.delete_outline),
                          label: const Text(
                            'Revogar',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        OutlinedButton(
                          onPressed: _dedicatedBusy
                              ? null
                              : () => _revokeDedicated(localOnly: true),
                          child: const Text(
                            'Remover só na central',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ],
                ],
              ),
            ),
          ] else ...<Widget>[
            Text(
              'Credencial temporária de sessão (senha mantida apenas em memória por 30 min — ADR 016).',
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 12),
            ResponsiveFieldRow(
              breakpoint: 480,
              children: <Widget>[
                TextField(
                  controller: _usernameCtrl,
                  onChanged: (_) => _standardUserEdited = true,
                  decoration: const InputDecoration(labelText: 'Usuário'),
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
              controller: _passwordCtrl,
              obscureText: _obscureSessionPass,
              decoration: InputDecoration(
                labelText: 'Senha de sessão (30 min)',
                hintText: 'Digite para liberar acesso nesta sessão',
                suffixIcon: IconButton(
                  tooltip: _obscureSessionPass
                      ? 'Mostrar senha'
                      : 'Ocultar senha',
                  icon: Icon(
                    _obscureSessionPass
                        ? Icons.visibility_outlined
                        : Icons.visibility_off_outlined,
                  ),
                  onPressed: () => setState(
                    () => _obscureSessionPass = !_obscureSessionPass,
                  ),
                ),
              ),
            ),
            const SizedBox(height: 10),
            if (_transport == 'http')
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Usar HTTPS'),
                value: _useHttps,
                onChanged: (v) {
                  setState(() {
                    _useHttps = v;
                    if (_portCtrl.text == '80' || _portCtrl.text == '443') {
                      _portCtrl.text = v ? '443' : '80';
                    }
                  });
                },
              ),
            if (agents.isNotEmpty) ...<Widget>[
              _buildAgentRouteField(agents),
              const SizedBox(height: 10),
            ],
            const SizedBox(height: 4),
            SymmetricalActionBar(
              children: <Widget>[
                FilledButton.icon(
                  onPressed: _saving ? null : _saveStandard,
                  icon: _saving
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.save_outlined),
                  label: const Text('Salvar credencial'),
                ),
              ],
            ),
          ],
        ],
      ],
    );
  }
}

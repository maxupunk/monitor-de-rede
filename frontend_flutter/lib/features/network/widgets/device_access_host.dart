import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Pedido de credencial pendente exibido pelo `DeviceAccessHost`.
@immutable
class DeviceAccessRequest {
  const DeviceAccessRequest({
    required this.deviceId,
    this.transport = 'ssh',
    this.deviceName,
    this.message,
  });

  final int deviceId;
  final String transport;
  final String? deviceName;
  final String? message;
}

class DeviceAccessController extends Notifier<DeviceAccessRequest?> {
  Completer<bool>? _pending;

  @override
  DeviceAccessRequest? build() => null;

  /// Abre o diálogo global de credenciais do equipamento e aguarda o desfecho.
  Future<bool> requestAccess({
    required int deviceId,
    String transport = 'ssh',
    String? deviceName,
    String? message,
  }) {
    _pending?.complete(false);
    final completer = Completer<bool>();
    _pending = completer;
    state = DeviceAccessRequest(
      deviceId: deviceId,
      transport: transport,
      deviceName: deviceName,
      message: message,
    );
    return completer.future;
  }

  void settle(bool saved) {
    final completer = _pending;
    _pending = null;
    state = null;
    if (completer != null && !completer.isCompleted) {
      completer.complete(saved);
    }
  }

  Future<DeviceAccessViewModel> fetchAccess(int deviceId) async {
    final raw = await ref
        .read(apiClientProvider)
        .get<dynamic>('/devices/$deviceId/access');
    return DeviceAccessViewModel.fromJson(asJsonMap(raw));
  }

  Future<List<DeviceCredentialView>> fetchCredentials(int deviceId) async {
    final raw = await ref
        .read(apiClientProvider)
        .get<dynamic>('/devices/$deviceId/credentials');
    return asJsonMapList(raw)
        .map(DeviceCredentialView.fromJson)
        .toList(growable: false);
  }

  Future<void> saveCredential(
    int deviceId,
    Map<String, dynamic> payload,
  ) async {
    await ref
        .read(apiClientProvider)
        .put<dynamic>('/devices/$deviceId/credentials', data: payload);
  }

  Future<void> deleteCredential(int deviceId, String kind) async {
    await ref
        .read(apiClientProvider)
        .delete<dynamic>(
          '/devices/$deviceId/credentials/${Uri.encodeComponent(kind)}',
        );
  }

  Future<DeviceAccessViewModel> provisionDedicatedKey(
    int deviceId,
    Map<String, dynamic> payload,
  ) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/devices/$deviceId/access/dedicated',
          data: payload,
          timeout: const Duration(seconds: 45),
        );
    return DeviceAccessViewModel.fromJson(asJsonMap(raw));
  }

  Future<DeviceAccessViewModel> rotateDedicatedKey(int deviceId) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/devices/$deviceId/access/dedicated/rotate',
          timeout: const Duration(seconds: 45),
        );
    return DeviceAccessViewModel.fromJson(asJsonMap(raw));
  }

  Future<void> revokeDedicatedKey(
    int deviceId, {
    required bool localOnly,
  }) async {
    await ref
        .read(apiClientProvider)
        .delete<dynamic>(
          '/devices/$deviceId/access/dedicated',
          data: <String, dynamic>{'localOnly': localOnly},
          timeout: const Duration(seconds: 45),
        );
  }
}

final deviceAccessProvider =
    NotifierProvider<DeviceAccessController, DeviceAccessRequest?>(
      DeviceAccessController.new,
    );

/// Intercepta `ApiException` com código `missing_credential`, abre o
/// `DeviceAccessHost` (para admins) e repete a ação após salvar.
Future<T?> withDeviceAccess<T>({
  required BuildContext context,
  required WidgetRef ref,
  required Future<T> Function() action,
  bool confirmRetry = false,
  String? actionLabel,
  VoidCallback? onSaved,
}) async {
  try {
    return await action();
  } on ApiException catch (err) {
    if (!err.isMissingCredential || err.deviceId == null) rethrow;

    final auth = ref.read(authProvider);
    if (!auth.canManagePlugins) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              err.message.isNotEmpty
                  ? '${err.message} Peça a um administrador para cadastrar a credencial.'
                  : 'Credencial ausente no equipamento. Apenas administradores podem cadastrá-la.',
            ),
          ),
        );
      }
      return null;
    }

    final saved = await ref
        .read(deviceAccessProvider.notifier)
        .requestAccess(
          deviceId: err.deviceId!,
          transport: err.transport ?? 'ssh',
          message: err.message,
        );
    if (!saved) return null;
    onSaved?.call();

    if (confirmRetry && context.mounted) {
      final proceed = await showConfirmActionDialog(
        context: context,
        title: 'Credencial salva',
        message: 'Deseja executar ${actionLabel ?? 'a operação'} agora?',
        confirmLabel: 'Executar agora',
        tone: SemanticTone.primary,
        icon: Icons.play_arrow_rounded,
      );
      if (!proceed) return null;
    }
    return action();
  }
}

/// Overlay global montado no shell para responder a qualquer pedido `missing_credential`.
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
    bool? saved;
    try {
      saved = await showAdaptiveFormSheet<bool>(
        context: context,
        title: 'Acesso ao equipamento',
        subtitle: req.deviceName ?? 'Dispositivo #${req.deviceId}',
        maxWidth: 640,
        builder: (sheetCtx) => DeviceAccessPanel(
          deviceId: req.deviceId,
          initialTransport: req.transport,
          bannerMessage: req.message,
          onSaved: () => Navigator.of(sheetCtx).pop(true),
        ),
      );
    } finally {
      _dialogOpen = false;
      ref.read(deviceAccessProvider.notifier).settle(saved == true);
    }
  }
}

/// Painel reutilizável de credenciais padrão (`ssh`/`http`/`telnet`) + Chave SSH Dedicada (ADR 016).
class DeviceAccessPanel extends ConsumerStatefulWidget {
  const DeviceAccessPanel({
    super.key,
    required this.deviceId,
    this.initialTransport = 'ssh',
    this.bannerMessage,
    this.onSaved,
  });

  final int deviceId;
  final String initialTransport;
  final String? bannerMessage;
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
  String _storage = 'persistent';
  bool _useHttps = false;
  int? _viaProbeId;

  final _usernameCtrl = TextEditingController();
  final _passwordCtrl = TextEditingController();
  final _portCtrl = TextEditingController();
  final _adminUserCtrl = TextEditingController();
  final _adminPassCtrl = TextEditingController();
  final _dedicatedUserCtrl = TextEditingController(text: 'netmonitor');

  @override
  void initState() {
    super.initState();
    _transport = widget.initialTransport;
    _portCtrl.text = _defaultPortFor(_transport).toString();
    Future<void>.microtask(_load);
  }

  @override
  void dispose() {
    _usernameCtrl.dispose();
    _passwordCtrl.dispose();
    _portCtrl.dispose();
    _adminUserCtrl.dispose();
    _adminPassCtrl.dispose();
    _dedicatedUserCtrl.dispose();
    super.dispose();
  }

  int _defaultPortFor(String transport) {
    switch (transport) {
      case 'http':
        return _useHttps ? 443 : 80;
      case 'telnet':
        return 23;
      default:
        return 22;
    }
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
      _view = view;
      _prefillForTransport(_transport);
      setState(() => _loading = false);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = err.toString();
      });
    }
  }

  void _prefillForTransport(String transport) {
    DeviceCredentialView? existing;
    for (final c in _view?.credentials ?? const <DeviceCredentialView>[]) {
      if (c.kind == transport) {
        existing = c;
        break;
      }
    }
    _usernameCtrl.text = existing?.username ?? '';
    _passwordCtrl.clear();
    _useHttps = existing?.useHttps ?? false;
    _storage = existing?.storage ?? 'persistent';
    _viaProbeId = existing?.viaProbeId;
    _portCtrl.text = (existing?.port ?? _defaultPortFor(transport)).toString();
  }

  Future<void> _saveStandard() async {
    if (_usernameCtrl.text.trim().isEmpty) {
      setState(() => _error = 'Informe o usuário para acesso $_transport.');
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref.read(deviceAccessProvider.notifier).saveCredential(
        widget.deviceId,
        <String, dynamic>{
          'kind': _transport,
          'username': _usernameCtrl.text.trim(),
          if (_passwordCtrl.text.isNotEmpty) 'password': _passwordCtrl.text,
          'port':
              int.tryParse(_portCtrl.text.trim()) ??
              _defaultPortFor(_transport),
          'storage': _storage,
          if (_transport == 'http') 'useHttps': _useHttps,
          if (_viaProbeId != null) 'viaProbeId': _viaProbeId,
        },
      );
      if (!mounted) return;
      await _load();
      widget.onSaved?.call();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Credencial salva com segurança.')),
        );
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _deleteStandard(String kind) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Remover credencial',
      message: 'Remover a credencial $kind deste equipamento?',
      confirmLabel: 'Remover',
    );
    if (!ok) return;
    setState(() => _saving = true);
    try {
      await ref
          .read(deviceAccessProvider.notifier)
          .deleteCredential(widget.deviceId, kind);
      await _load();
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _provisionDedicated() async {
    if (_adminUserCtrl.text.trim().isEmpty || _adminPassCtrl.text.isEmpty) {
      setState(
        () => _error =
            'Informe usuário e senha administrativos para provisionar a chave.',
      );
      return;
    }
    setState(() {
      _dedicatedBusy = true;
      _error = null;
    });
    try {
      final view = await ref
          .read(deviceAccessProvider.notifier)
          .provisionDedicatedKey(widget.deviceId, <String, dynamic>{
            'adminUsername': _adminUserCtrl.text.trim(),
            'adminPassword': _adminPassCtrl.text,
            'dedicatedUsername': _dedicatedUserCtrl.text.trim().isEmpty
                ? 'netmonitor'
                : _dedicatedUserCtrl.text.trim(),
            if (_viaProbeId != null) 'viaProbeId': _viaProbeId,
          });
      if (!mounted) return;
      _adminPassCtrl.clear();
      setState(() {
        _view = view;
        _dedicatedBusy = false;
      });
      widget.onSaved?.call();
    } catch (err) {
      if (mounted) {
        setState(() {
          _dedicatedBusy = false;
          _error = err.toString();
        });
      }
    }
  }

  Future<void> _rotateDedicated() async {
    setState(() {
      _dedicatedBusy = true;
      _error = null;
    });
    try {
      final view = await ref
          .read(deviceAccessProvider.notifier)
          .rotateDedicatedKey(widget.deviceId);
      if (mounted) setState(() => _view = view);
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _dedicatedBusy = false);
    }
  }

  Future<void> _revokeDedicated({required bool localOnly}) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Revogar acesso dedicado',
      message: localOnly
          ? 'Remover a chave dedicada apenas na central (use se o equipamento estiver inacessível)?'
          : 'Revogar a chave dedicada no equipamento e na central?',
      confirmLabel: 'Revogar',
    );
    if (!ok) return;
    setState(() {
      _dedicatedBusy = true;
      _error = null;
    });
    try {
      await ref
          .read(deviceAccessProvider.notifier)
          .revokeDedicatedKey(widget.deviceId, localOnly: localOnly);
      await _load();
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _dedicatedBusy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final canManage = ref.watch(authProvider).canManagePlugins;
    if (_loading) {
      return const LoadingStateView(
        message: 'Carregando acessos do equipamento...',
      );
    }

    final existingCreds = _view?.credentials ?? const <DeviceCredentialView>[];
    final hasDedicated = existingCreds.any((c) => c.dedicated);
    final agents = _view?.agents ?? const <AgentRouteOption>[];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        if (widget.bannerMessage != null &&
            widget.bannerMessage!.isNotEmpty) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.warning,
            child: Row(
              children: <Widget>[
                const Icon(Icons.vpn_key_outlined, size: 18),
                const SizedBox(width: 10),
                Expanded(child: Text(widget.bannerMessage!)),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 12),
        ],
        if (existingCreds.isNotEmpty) ...<Widget>[
          Text(
            'Credenciais cadastradas',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: existingCreds
                .map((c) {
                  return InputChip(
                    avatar: Icon(
                      c.dedicated ? Icons.key : Icons.lock_outline,
                      size: 16,
                    ),
                    label: Text(
                      '${c.kind.toUpperCase()} · ${c.username ?? '—'}${c.storage == 'ask' ? ' (sessão 30m)' : ''}',
                    ),
                    onDeleted: canManage && !c.dedicated
                        ? () => _deleteStandard(c.kind)
                        : null,
                  );
                })
                .toList(growable: false),
          ),
          const SizedBox(height: 16),
        ],
        if (!canManage)
          const NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              'Somente administradores podem cadastrar, rotacionar ou remover credenciais de acesso.',
            ),
          )
        else ...<Widget>[
          Text(
            'Credencial de transporte',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          SegmentedButton<String>(
            segments: const <ButtonSegment<String>>[
              ButtonSegment<String>(
                value: 'ssh',
                label: Text('SSH'),
                icon: Icon(Icons.terminal, size: 16),
              ),
              ButtonSegment<String>(
                value: 'http',
                label: Text('HTTP'),
                icon: Icon(Icons.http, size: 16),
              ),
              ButtonSegment<String>(
                value: 'telnet',
                label: Text('Telnet'),
                icon: Icon(Icons.cable, size: 16),
              ),
            ],
            selected: <String>{_transport},
            onSelectionChanged: (sel) {
              setState(() {
                _transport = sel.first;
                _prefillForTransport(_transport);
              });
            },
          ),
          const SizedBox(height: 12),
          Row(
            children: <Widget>[
              Expanded(
                flex: 2,
                child: TextField(
                  controller: _usernameCtrl,
                  decoration: const InputDecoration(labelText: 'Usuário'),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: TextField(
                  controller: _portCtrl,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(labelText: 'Porta'),
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _passwordCtrl,
            obscureText: true,
            decoration: const InputDecoration(
              labelText: 'Senha (nunca exibida após salvar)',
              hintText: 'Digite para definir ou alterar',
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
          DropdownButtonFormField<String>(
            initialValue: _storage,
            decoration: const InputDecoration(
              labelText: 'Armazenamento da senha',
            ),
            items: const <DropdownMenuItem<String>>[
              DropdownMenuItem(
                value: 'persistent',
                child: Text('Persistente (cifrada na central)'),
              ),
              DropdownMenuItem(
                value: 'ask',
                child: Text('Temporária (memória de sessão · 30 min)'),
              ),
            ],
            onChanged: (v) {
              if (v != null) setState(() => _storage = v);
            },
          ),
          if (agents.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            DropdownButtonFormField<int?>(
              initialValue: _viaProbeId,
              decoration: const InputDecoration(
                labelText: 'Rota de saída (DeviceIo)',
              ),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text('Direto pela central'),
                ),
                for (final agent in agents)
                  DropdownMenuItem<int?>(
                    value: agent.id,
                    child: Text(
                      '${agent.name}${agent.online ? ' (online)' : ' (offline)'}',
                    ),
                  ),
              ],
              onChanged: (v) => setState(() => _viaProbeId = v),
            ),
          ],
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
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
          ),
          if (_view?.supportsDedicatedKey == true) ...<Widget>[
            const SizedBox(height: 18),
            const Divider(),
            const SizedBox(height: 12),
            NestedPanel(
              tone: SemanticTone.plugin,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Row(
                    children: <Widget>[
                      const Icon(Icons.vpn_key_outlined, size: 18),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          'Acesso dedicado por chave SSH (${_view!.dedicatedPlatform})',
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                      SemanticChip(
                        label: hasDedicated ? 'Provisionado' : 'Disponível',
                        tone: hasDedicated
                            ? SemanticTone.success
                            : SemanticTone.plugin,
                        compact: true,
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'A central gera um par de chaves SSH e cria uma conta dedicada no equipamento sem guardar a senha administrativa.',
                    style: context.textTheme.bodySmall,
                  ),
                  const SizedBox(height: 10),
                  if (!hasDedicated) ...<Widget>[
                    TextField(
                      controller: _dedicatedUserCtrl,
                      decoration: const InputDecoration(
                        labelText: 'Usuário dedicado a criar',
                      ),
                    ),
                    const SizedBox(height: 8),
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: TextField(
                            controller: _adminUserCtrl,
                            decoration: const InputDecoration(
                              labelText: 'Usuário admin atual',
                            ),
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: TextField(
                            controller: _adminPassCtrl,
                            obscureText: true,
                            decoration: const InputDecoration(
                              labelText: 'Senha admin (uso único)',
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),
                    FilledButton.icon(
                      onPressed: _dedicatedBusy ? null : _provisionDedicated,
                      icon: const Icon(Icons.key),
                      label: const Text('Provisionar chave SSH'),
                    ),
                  ] else ...<Widget>[
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: <Widget>[
                        OutlinedButton.icon(
                          onPressed: _dedicatedBusy ? null : _rotateDedicated,
                          icon: const Icon(Icons.autorenew),
                          label: const Text('Rotacionar chave'),
                        ),
                        OutlinedButton.icon(
                          onPressed: _dedicatedBusy
                              ? null
                              : () => _revokeDedicated(localOnly: false),
                          icon: const Icon(Icons.delete_outline),
                          label: const Text('Revogar'),
                        ),
                        TextButton(
                          onPressed: _dedicatedBusy
                              ? null
                              : () => _revokeDedicated(localOnly: true),
                          child: const Text('Remover só na central'),
                        ),
                      ],
                    ),
                  ],
                ],
              ),
            ),
          ],
        ],
      ],
    );
  }
}

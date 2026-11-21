import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/providers/agents_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Diálogo / bottom sheet de instalação de agente remoto com seleção de endereço,
/// política `AGENT_ALLOW`, comandos copiáveis (systemd/Docker) e detecção SSE ao vivo.
Future<void> showAgentEnrollmentDialog(
  BuildContext context, {
  required AgentView agent,
  required AgentEnrollmentView enrollment,
  List<String> preferredKinds = const <String>[],
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Instalar agente em ${agent.name}',
    subtitle: 'Código de uso único · detecção automática quando conectar',
    maxWidth: 740,
    builder: (_) => _AgentEnrollmentBody(
      agent: agent,
      initialEnrollment: enrollment,
      preferredKinds: preferredKinds,
    ),
  );
}

class _AgentEnrollmentBody extends ConsumerStatefulWidget {
  const _AgentEnrollmentBody({
    required this.agent,
    required this.initialEnrollment,
    required this.preferredKinds,
  });

  final AgentView agent;
  final AgentEnrollmentView initialEnrollment;
  final List<String> preferredKinds;

  @override
  ConsumerState<_AgentEnrollmentBody> createState() =>
      _AgentEnrollmentBodyState();
}

class _AgentEnrollmentBodyState extends ConsumerState<_AgentEnrollmentBody> {
  late AgentInstallCommands _commands;
  late InstallPolicyView _policy;
  late TextEditingController _targetsController;
  String? _selectedAddressId;
  bool _reloading = false;
  bool _connectedNow = false;
  bool _wasConnectedInitially = false;
  int _selectedCommandTab = 0;

  @override
  void initState() {
    super.initState();
    _commands = widget.initialEnrollment.commands;
    _policy = _commands.policy.allow.isNotEmpty
        ? _commands.policy
        : defaultInstallPolicy();
    _targetsController = TextEditingController(
      text: _policy.databaseTargets.isNotEmpty
          ? _policy.databaseTargets
          : defaultDatabaseTargets,
    );
    _selectedAddressId = _commands.addressId;
    _wasConnectedInitially = widget.agent.connected;

    Future<void>.microtask(_bootstrapAddressesAndPolicy);
  }

  Future<void> _bootstrapAddressesAndPolicy() async {
    final notifier = ref.read(agentsProvider.notifier);
    await notifier.fetchServerAddresses();
    if (!mounted) return;
    final addresses = ref.read(agentsProvider).addresses;
    if (addresses.isNotEmpty && widget.preferredKinds.isNotEmpty) {
      ServerAddressOption? preferred;
      for (final kind in widget.preferredKinds) {
        for (final addr in addresses) {
          if (addr.kind == kind) {
            preferred = addr;
            break;
          }
        }
        if (preferred != null) break;
      }
      if (preferred != null && preferred.id != _selectedAddressId) {
        setState(() => _selectedAddressId = preferred!.id);
        await _regenerateCommands();
      }
    }
  }

  @override
  void dispose() {
    _targetsController.dispose();
    super.dispose();
  }

  Future<void> _regenerateCommands() async {
    setState(() => _reloading = true);
    final updatedPolicy = _policy.copyWith(
      databaseTargets: _targetsController.text.trim(),
    );
    final next = await ref
        .read(agentsProvider.notifier)
        .installCommands(
          code: widget.initialEnrollment.code,
          addressId: _selectedAddressId,
          policy: updatedPolicy,
        );
    if (!mounted) return;
    setState(() {
      _reloading = false;
      if (next != null) {
        _commands = next;
        _policy = updatedPolicy;
      }
    });
  }

  void _togglePermission(String key, bool checked) {
    if (key == 'read') return;
    final current = List<String>.from(_policy.allow);
    if (checked) {
      if (!current.contains(key)) current.add(key);
    } else {
      current.remove(key);
    }
    setState(() => _policy = _policy.copyWith(allow: current));
    unawaited(_regenerateCommands());
  }

  void _selectAllPermissions() {
    setState(
      () => _policy = _policy.copyWith(
        allow: agentPermissionsCatalog
            .map((e) => e.key)
            .toList(growable: false),
      ),
    );
    unawaited(_regenerateCommands());
  }

  void _selectReadOnlyPermissions() {
    setState(() => _policy = _policy.copyWith(allow: const <String>['read']));
    unawaited(_regenerateCommands());
  }

  @override
  Widget build(BuildContext context) {
    final agentsState = ref.watch(agentsProvider);
    final liveAgent = agentsState.byId(widget.agent.id) ?? widget.agent;

    if (!_wasConnectedInitially && liveAgent.connected && !_connectedNow) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) setState(() => _connectedNow = true);
      });
    }

    final addresses = agentsState.addresses;
    ServerAddressOption? selectedAddr;
    for (final a in addresses) {
      if (a.id == _selectedAddressId) {
        selectedAddr = a;
        break;
      }
    }

    final serverUrl = _commands.serverUrl;
    final parsedUri = Uri.tryParse(serverUrl);
    final isInsecurePublic =
        serverUrl.startsWith('http://') &&
        parsedUri != null &&
        !isPrivateHost(parsedUri.host);
    final isVpnAddress = selectedAddr?.kind == 'vpn';
    final expiresMin = (widget.initialEnrollment.expiresInSeconds / 60)
        .round()
        .clamp(1, 999);
    final codePreview = widget.initialEnrollment.code.length > 12
        ? '${widget.initialEnrollment.code.substring(0, 12)}…'
        : widget.initialEnrollment.code;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (_connectedNow) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.success,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(
                      Icons.check_circle,
                      color: context.semanticColors.success,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        '${liveAgent.name} conectado!',
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                const Text(
                  'O agente já se registrou na central e está pronto para Docker, métricas e deploys.',
                ),
                const SizedBox(height: 12),
                FilledButton.icon(
                  onPressed: () {
                    Navigator.of(context).pop();
                    context.go(
                      '/docker?host=${Uri.encodeQueryComponent(liveAgent.hostKey)}',
                    );
                  },
                  icon: const Icon(Icons.view_in_ar_outlined),
                  label: const Text('Abrir Docker'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
        ] else ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(
                  Icons.key_outlined,
                  size: 20,
                  color: context.semanticColors.info,
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'Código único ($codePreview) válido por $expiresMin min. '
                    'Ele deixa de valer assim que o servidor conectar pela primeira vez.',
                    style: context.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 14),
        ],

        if (addresses.isNotEmpty) ...<Widget>[
          DropdownButtonFormField<String?>(
            initialValue: addresses.any((a) => a.id == _selectedAddressId)
                ? _selectedAddressId
                : null,
            decoration: const InputDecoration(
              labelText: 'Endereço da central',
              helperText: 'O agente disca para fora até este endereço.',
            ),
            items: <DropdownMenuItem<String?>>[
              const DropdownMenuItem<String?>(
                value: null,
                child: Text('Padrão configurado na central'),
              ),
              ...addresses.map(
                (addr) => DropdownMenuItem<String?>(
                  value: addr.id,
                  child: Text('${addr.label} (${addr.host})'),
                ),
              ),
            ],
            onChanged: (value) {
              setState(() => _selectedAddressId = value);
              unawaited(_regenerateCommands());
            },
          ),
          const SizedBox(height: 12),
        ] else if (_commands.addressId == null) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.secondary,
            child: Text(
              'Usando endereço padrão da central ($serverUrl). '
              'Você pode configurar endereços adicionais em Configurações → Conectividade.',
              style: context.textTheme.bodySmall,
            ),
          ),
          const SizedBox(height: 12),
        ],

        if (isVpnAddress) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              'Este endereço usa a VPN da central. O servidor remoto precisa já estar dentro do túnel WireGuard para alcançá-lo.',
              style: context.textTheme.bodySmall,
            ),
          ),
          const SizedBox(height: 12),
        ],

        if (isInsecurePublic) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.warning,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(
                  Icons.warning_amber_rounded,
                  color: context.semanticColors.warning,
                  size: 20,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Atenção: o endereço $serverUrl usa HTTP sem TLS fora da rede privada. '
                    'Prefira HTTPS ou a VPN do NetMonitor pela internet.',
                    style: context.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],

        // Política local AGENT_ALLOW
        NestedPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Wrap(
                spacing: 8,
                runSpacing: 8,
                alignment: WrapAlignment.spaceBetween,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  Text(
                    'Permissões locais (AGENT_ALLOW)',
                    style: context.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  Wrap(
                    spacing: 6,
                    children: <Widget>[
                      TextButton(
                        onPressed: _selectAllPermissions,
                        child: const Text('Tudo'),
                      ),
                      TextButton(
                        onPressed: _selectReadOnlyPermissions,
                        child: const Text('Só leitura'),
                      ),
                    ],
                  ),
                ],
              ),
              const SizedBox(height: 6),
              Text(
                'A política local no servidor é soberana: a central nunca consegue ampliá-la remotamente.',
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: 10),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: agentPermissionsCatalog
                    .map((perm) {
                      final selected =
                          perm.key == 'read' ||
                          _policy.allow.contains(perm.key);
                      return FilterChip(
                        selected: selected,
                        label: Text(perm.label),
                        tooltip: perm.description,
                        onSelected: perm.key == 'read'
                            ? null
                            : (checked) => _togglePermission(perm.key, checked),
                      );
                    })
                    .toList(growable: false),
              ),
              if (_policy.allow.contains('database')) ...<Widget>[
                const SizedBox(height: 12),
                TextField(
                  controller: _targetsController,
                  decoration: const InputDecoration(
                    labelText:
                        'AGENT_DATABASE_TARGETS (faixas privadas permitidas)',
                    hintText: defaultDatabaseTargets,
                    helperText: 'Lista host/CIDR:porta alcançável pela ponte de banco (ADR 013).',
                  ),
                  onSubmitted: (_) => _regenerateCommands(),
                  onEditingComplete: _regenerateCommands,
                ),
              ],
            ],
          ),
        ),
        const SizedBox(height: 14),

        SegmentedButton<int>(
          segments: const <ButtonSegment<int>>[
            ButtonSegment<int>(
              value: 0,
              label: Text('Serviço systemd'),
              icon: Icon(Icons.terminal),
            ),
            ButtonSegment<int>(
              value: 1,
              label: Text('Container Docker'),
              icon: Icon(Icons.view_in_ar_outlined),
            ),
          ],
          selected: <int>{_selectedCommandTab},
          onSelectionChanged: (set) =>
              setState(() => _selectedCommandTab = set.first),
        ),
        const SizedBox(height: 12),

        if (_reloading)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 12),
            child: LinearProgressIndicator(),
          ),

        CodeViewer(
          title: _selectedCommandTab == 0
              ? 'Instalar como serviço systemd (recomendado)'
              : 'Executar como container Docker',
          code: _selectedCommandTab == 0
              ? _commands.systemdCommand
              : _commands.dockerCommand,
          maxHeight: 260,
        ),
      ],
    );
  }
}

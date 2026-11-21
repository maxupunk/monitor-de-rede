import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Redes" do Docker (`/docker/networks`).
class DockerNetworksPage extends ConsumerStatefulWidget {
  const DockerNetworksPage({super.key});

  @override
  ConsumerState<DockerNetworksPage> createState() => _DockerNetworksPageState();
}

class _DockerNetworksPageState extends ConsumerState<DockerNetworksPage> {
  final TextEditingController _searchController = TextEditingController();
  String _search = '';

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _openCreateNetworkDialog() async {
    final nameController = TextEditingController();
    var driver = 'bridge';
    final created = await showAdaptiveFormSheet<bool>(
      context: context,
      title: 'Nova rede Docker',
      builder: (ctx) => StatefulBuilder(
        builder: (innerCtx, setModalState) => Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            TextField(
              controller: nameController,
              autofocus: true,
              decoration: const InputDecoration(
                labelText: 'Nome da rede',
                hintText: 'ex.: app-internal',
              ),
            ),
            const SizedBox(height: 12),
            DropdownButtonFormField<String>(
              initialValue: driver,
              decoration: const InputDecoration(labelText: 'Driver'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(value: 'bridge', child: Text('bridge')),
                DropdownMenuItem(value: 'overlay', child: Text('overlay')),
                DropdownMenuItem(value: 'macvlan', child: Text('macvlan')),
                DropdownMenuItem(value: 'ipvlan', child: Text('ipvlan')),
              ],
              onChanged: (v) {
                if (v != null) setModalState(() => driver = v);
              },
            ),
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton(
                onPressed: () async {
                  final name = nameController.text.trim();
                  if (name.isEmpty) return;
                  final ok = await ref
                      .read(dockerProvider.notifier)
                      .runAction(
                        () => ref
                            .read(dockerProvider.notifier)
                            .apiFor()
                            .createNetwork(name, driver: driver),
                        refresh: ref.read(dockerProvider.notifier).refreshAll,
                      );
                  if (innerCtx.mounted) Navigator.of(innerCtx).pop(ok);
                },
                child: const Text('Criar rede'),
              ),
            ),
          ],
        ),
      ),
    );
    nameController.dispose();
    if (created == true && mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('Rede Docker criada.')));
    }
  }

  Future<void> _removeNetwork(DockerNetworkSummary net) async {
    if (net.isBuiltIn) return;
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Remover rede ${net.name}?',
      message:
          'A rede não pode ter containers conectados no momento da remoção.',
      confirmLabel: 'Remover rede',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref
        .read(dockerProvider.notifier)
        .runAction(
          () =>
              ref.read(dockerProvider.notifier).apiFor().removeNetwork(net.id),
          refresh: ref.read(dockerProvider.notifier).refreshAll,
        );
    if (ok && mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text('Rede ${net.name} removida.')));
    }
  }

  Future<void> _inspectNetwork(DockerNetworkSummary net) {
    return showAdaptiveFormSheet<void>(
      context: context,
      title: net.name,
      subtitle: '${net.driver} · ${shortDockerId(net.id)}',
      maxWidth: 680,
      builder: (_) => _NetworkDetailBody(networkId: net.id),
    );
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final isAdmin = ref.watch(authProvider).isAdmin;
    final term = _search.trim().toLowerCase();
    final filtered = docker.current.networks
        .where((n) {
          if (term.isEmpty) return true;
          return '${n.name} ${n.driver} ${n.id}'.toLowerCase().contains(term);
        })
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          searchController: _searchController,
          searchHint: 'Buscar rede Docker...',
          onSearchChanged: (v) => setState(() => _search = v),
          actions: <Widget>[
            if (isAdmin)
              FilledButton.icon(
                onPressed: _openCreateNetworkDialog,
                icon: const Icon(Icons.add),
                label: const Text('Nova rede'),
              ),
          ],
        ),
        const SizedBox(height: 14),
        if (filtered.isEmpty)
          const EmptyStateView(
            title: 'Nenhuma rede encontrada',
            icon: Icons.hub_outlined,
          )
        else
          ...filtered.map((net) {
            final subnets = net.ipamConfig
                .map((e) => e.subnet)
                .whereType<String>()
                .where((s) => s.isNotEmpty)
                .join(', ');
            return Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Card(
                elevation: 2,
                shape: RoundedRectangleBorder(
                  borderRadius: AppTheme.borderRadius,
                ),
                child: ListTile(
                  onTap: () => _inspectNetwork(net),
                  leading: const Icon(Icons.hub_outlined),
                  title: Wrap(
                    spacing: 8,
                    runSpacing: 4,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      Text(
                        net.name,
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                      SemanticChip(
                        label: net.internal ? 'Interna' : 'Externa',
                        tone: net.internal
                            ? SemanticTone.warning
                            : SemanticTone.success,
                        compact: true,
                      ),
                      SemanticChip(
                        label: '${net.connectedContainers} containers',
                        tone: SemanticTone.info,
                        compact: true,
                      ),
                    ],
                  ),
                  subtitle: Text(
                    'Driver: ${net.driver} · Escopo: ${net.scope}'
                    '${subnets.isNotEmpty ? ' · Sub-redes: $subnets' : ''}',
                  ),
                  trailing: isAdmin && !net.isBuiltIn
                      ? IconButton(
                          tooltip: 'Remover rede',
                          icon: const Icon(Icons.delete_outline),
                          onPressed: () => _removeNetwork(net),
                        )
                      : null,
                ),
              ),
            );
          }),
      ],
    );
  }
}

class _NetworkDetailBody extends ConsumerStatefulWidget {
  const _NetworkDetailBody({required this.networkId});
  final String networkId;

  @override
  ConsumerState<_NetworkDetailBody> createState() => _NetworkDetailBodyState();
}

class _NetworkDetailBodyState extends ConsumerState<_NetworkDetailBody> {
  DockerNetworkDetail? _detail;
  bool _loading = true;
  String? _error;
  String? _selectedContainerId;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final d = await ref
          .read(dockerProvider.notifier)
          .apiFor()
          .networkDetail(widget.networkId);
      if (!mounted) return;
      setState(() {
        _detail = d;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _loading = false;
      });
    }
  }

  Future<void> _connect() async {
    if (_selectedContainerId == null) return;
    final ok = await ref
        .read(dockerProvider.notifier)
        .runAction(
          () => ref
              .read(dockerProvider.notifier)
              .apiFor()
              .connectNetwork(widget.networkId, _selectedContainerId!),
        );
    if (ok && mounted) {
      setState(() => _selectedContainerId = null);
      await _load();
    }
  }

  Future<void> _disconnect(DockerNetworkContainerRef c) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Desconectar ${c.name}?',
      message: 'O container será removido desta rede Docker.',
      confirmLabel: 'Desconectar',
      tone: SemanticTone.warning,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref
        .read(dockerProvider.notifier)
        .runAction(
          () => ref
              .read(dockerProvider.notifier)
              .apiFor()
              .disconnectNetwork(widget.networkId, c.containerId),
        );
    if (ok && mounted) await _load();
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Inspecionando rede...');
    }
    if (_error != null) return ErrorStateView(message: _error!, onRetry: _load);
    final d = _detail!;
    final isAdmin = ref.watch(authProvider).isAdmin;
    final allContainers = ref.watch(dockerProvider).current.containers;
    final connectedIds = d.containers.map((c) => c.containerId).toSet();
    final unconnected = allContainers
        .where((c) => !connectedIds.contains(c.id))
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        KeyValueRow(label: 'Driver', value: d.summary.driver),
        KeyValueRow(label: 'Escopo', value: d.summary.scope),
        KeyValueRow(
          label: 'IPAM Driver',
          value: d.summary.ipamDriver ?? 'default',
        ),
        KeyValueRow(
          label: 'Acesso',
          value: d.summary.internal ? 'Interna' : 'Externa',
        ),
        if (d.summary.ipamConfig.isNotEmpty) ...<Widget>[
          const SizedBox(height: 8),
          ...d.summary.ipamConfig.map(
            (ipam) => Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: NestedPanel(
                tone: SemanticTone.info,
                child: Text(
                  'Sub-rede: ${ipam.subnet ?? '—'} · Gateway: ${ipam.gateway ?? '—'}',
                  style: const TextStyle(
                    fontFamily: 'monospace',
                    fontSize: 12.5,
                  ),
                ),
              ),
            ),
          ),
        ],
        const SizedBox(height: 12),
        if (isAdmin && unconnected.isNotEmpty) ...<Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: DropdownButtonFormField<String>(
                  initialValue:
                      unconnected.any((c) => c.id == _selectedContainerId)
                      ? _selectedContainerId
                      : null,
                  decoration: const InputDecoration(
                    labelText: 'Conectar container',
                    isDense: true,
                  ),
                  items: unconnected
                      .map(
                        (c) => DropdownMenuItem<String>(
                          value: c.id,
                          child: Text(containerDisplayName(c)),
                        ),
                      )
                      .toList(growable: false),
                  onChanged: (v) => setState(() => _selectedContainerId = v),
                ),
              ),
              const SizedBox(width: 8),
              FilledButton(
                onPressed: _selectedContainerId == null ? null : _connect,
                child: const Text('Conectar'),
              ),
            ],
          ),
          const SizedBox(height: 12),
        ],
        Text(
          'Containers conectados (${d.containers.length})',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 8),
        if (d.containers.isEmpty)
          const Text('Nenhum container conectado nesta rede.')
        else
          ...d.containers.map(
            (c) => Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: NestedPanel(
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            c.name,
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          Text(
                            'IPv4: ${c.ipv4Address ?? '—'} · MAC: ${c.macAddress ?? '—'}',
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    if (isAdmin)
                      IconButton(
                        tooltip: 'Desconectar',
                        icon: const Icon(Icons.link_off),
                        onPressed: () => _disconnect(c),
                      ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

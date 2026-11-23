import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class SitesPage extends ConsumerStatefulWidget {
  const SitesPage({super.key});

  @override
  ConsumerState<SitesPage> createState() => _SitesPageState();
}

class _SitesPageState extends ConsumerState<SitesPage> {
  final TextEditingController _searchCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() {
      if (!mounted) return;
      ref.read(sitesProvider.notifier).fetchSites();
      final devicesState = ref.read(devicesProvider);
      if (devicesState.items.isEmpty && !devicesState.loading) {
        ref.read(devicesProvider.notifier).fetchDevices();
      }
      final networksState = ref.read(networksProvider);
      if (networksState.items.isEmpty && !networksState.loading) {
        ref.read(networksProvider.notifier).fetchNetworks();
      }
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _openSiteSheet({SiteModel? existing}) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing != null ? 'Editar Site' : 'Novo Site',
      subtitle:
          'Agrupe dispositivos, redes IP e topologia por unidade ou datacenter',
      builder: (ctx) => _SiteFormBody(
        existing: existing,
        onCancel: () => Navigator.of(ctx).pop(),
        onSaved: () => Navigator.of(ctx).pop(),
      ),
    );
  }

  Future<void> _confirmDelete(SiteModel site) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Excluir site',
      message:
          'Remover o site "${site.name}"? Dispositivos vinculados ficarão sem site.',
      confirmLabel: 'Excluir',
    );
    if (!ok || !mounted) return;
    final deleted = await ref.read(sitesProvider.notifier).deleteSite(site.id);
    if (!deleted && mounted) {
      final err = ref.read(sitesProvider).error;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(err ?? 'Erro ao excluir site.')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(sitesProvider);
    final devices = ref.watch(devicesProvider).items;
    final networks = ref.watch(networksProvider).items;
    final canWrite = ref.watch(authProvider).canWriteIn(ProductArea.core);
    final query = _searchCtrl.text.trim().toLowerCase();
    final filtered = state.items
        .where((s) {
          if (query.isEmpty) return true;
          return '${s.name} ${s.location ?? ''} ${s.description ?? ''}'
              .toLowerCase()
              .contains(query);
        })
        .toList(growable: false);

    final devicesBySite = <int, int>{};
    for (final d in devices) {
      if (d.siteId != null) {
        devicesBySite[d.siteId!] = (devicesBySite[d.siteId!] ?? 0) + 1;
      }
    }
    final networksBySite = <int, int>{};
    for (final n in networks) {
      if (n.siteId != null) {
        networksBySite[n.siteId!] = (networksBySite[n.siteId!] ?? 0) + 1;
      }
    }

    int siteDeviceCount(SiteModel s) =>
        s.devicesCount > 0 ? s.devicesCount : (devicesBySite[s.id] ?? 0);
    int siteNetworkCount(SiteModel s) =>
        s.networksCount > 0 ? s.networksCount : (networksBySite[s.id] ?? 0);

    final totalDevices = state.items.fold<int>(
      0,
      (acc, s) => acc + siteDeviceCount(s),
    );
    final totalNetworks = state.items.fold<int>(
      0,
      (acc, s) => acc + siteNetworkCount(s),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        return RefreshIndicator(
          onRefresh: () => ref.read(sitesProvider.notifier).fetchSites(),
          child: ListView(
            children: <Widget>[
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: <Widget>[
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          'Sites',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style:
                              (isCompact
                                      ? context.textTheme.titleLarge
                                      : context.textTheme.headlineSmall)
                                  ?.copyWith(fontWeight: FontWeight.w800),
                        ),
                        if (!isCompact) ...<Widget>[
                          const SizedBox(height: 2),
                          Text(
                            'Unidades físicas, filiais e datacenters monitorados',
                            style: context.textTheme.bodySmall?.copyWith(
                              color: context.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                  if (canWrite) ...<Widget>[
                    const SizedBox(width: 8),
                    FilledButton.icon(
                      onPressed: () => _openSiteSheet(),
                      icon: const Icon(Icons.add, size: 18),
                      label: Text(isCompact ? 'Novo' : 'Novo Site'),
                    ),
                  ],
                ],
              ),
              const SizedBox(height: 10),
              UnifiedFilterBar(
                searchController: _searchCtrl,
                searchHint: 'Buscar site por nome ou localização...',
                onSearchChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 12),
              if (isCompact)
                Row(
                  children: <Widget>[
                    Expanded(
                      child: _CompactSiteStat(
                        label: 'Sites',
                        value: formatInteger(state.items.length),
                        tone: SemanticTone.primary,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: _CompactSiteStat(
                        label: 'Dispositivos',
                        value: formatInteger(totalDevices),
                        tone: SemanticTone.info,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: _CompactSiteStat(
                        label: 'Redes IP',
                        value: formatInteger(totalNetworks),
                        tone: SemanticTone.success,
                      ),
                    ),
                  ],
                )
              else
                ResponsiveGrid(
                  xs: 3,
                  sm: 3,
                  md: 3,
                  lg: 3,
                  xl: 3,
                  children: <Widget>[
                    KpiCard(
                      label: 'Sites cadastrados',
                      value: formatInteger(state.items.length),
                      icon: Icons.domain_outlined,
                      tone: SemanticTone.primary,
                    ),
                    KpiCard(
                      label: 'Dispositivos alocados',
                      value: formatInteger(totalDevices),
                      icon: Icons.devices_outlined,
                      tone: SemanticTone.info,
                    ),
                    KpiCard(
                      label: 'Redes IP associadas',
                      value: formatInteger(totalNetworks),
                      icon: Icons.lan_outlined,
                      tone: SemanticTone.success,
                    ),
                  ],
                ),
              const SizedBox(height: 14),
              if (state.loading && state.items.isEmpty)
                const LoadingStateView(message: 'Carregando sites...')
              else if (state.error != null && state.items.isEmpty)
                ErrorStateView(
                  message: state.error!,
                  onRetry: () => ref.read(sitesProvider.notifier).fetchSites(),
                )
              else if (filtered.isEmpty)
                EmptyStateView(
                  title: 'Nenhum site encontrado',
                  description:
                      'Crie sites para organizar redes, dispositivos e mapas de topologia.',
                  icon: Icons.domain_disabled_outlined,
                  action: canWrite
                      ? FilledButton.icon(
                          onPressed: () => _openSiteSheet(),
                          icon: const Icon(Icons.add),
                          label: const Text('Criar primeiro site'),
                        )
                      : null,
                )
              else
                ResponsiveGrid(
                  xs: 1,
                  sm: 1,
                  md: 2,
                  lg: 3,
                  xl: 3,
                  children: filtered
                      .map((site) {
                        return SectionCard(
                          title: site.name,
                          subtitle:
                              site.location ?? 'Localização não informada',
                          icon: Icons.domain_outlined,
                          stackTrailingOnCompact: false,
                          trailing: canWrite
                              ? Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: <Widget>[
                                    IconButton(
                                      visualDensity: VisualDensity.compact,
                                      tooltip: 'Editar site',
                                      icon: const Icon(
                                        Icons.edit_outlined,
                                        size: 18,
                                      ),
                                      onPressed: () =>
                                          _openSiteSheet(existing: site),
                                    ),
                                    IconButton(
                                      visualDensity: VisualDensity.compact,
                                      tooltip: 'Excluir site',
                                      icon: Icon(
                                        Icons.delete_outline,
                                        size: 18,
                                        color: context.colorScheme.error,
                                      ),
                                      onPressed: () => _confirmDelete(site),
                                    ),
                                  ],
                                )
                              : null,
                          footer: SymmetricalActionBar(
                            children: <Widget>[
                              OutlinedButton.icon(
                                onPressed: () => context.go('/topology'),
                                icon: const Icon(
                                  Icons.account_tree_outlined,
                                  size: 16,
                                ),
                                label: const Text('Topologia'),
                              ),
                              OutlinedButton.icon(
                                onPressed: () => context.go('/networks'),
                                icon: const Icon(Icons.lan_outlined, size: 16),
                                label: const Text('Redes IP'),
                              ),
                            ],
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: <Widget>[
                              Wrap(
                                spacing: 8,
                                runSpacing: 6,
                                children: <Widget>[
                                  SemanticChip(
                                    label: countNoun(
                                      siteDeviceCount(site),
                                      'dispositivo',
                                      'dispositivos',
                                    ),
                                    tone: SemanticTone.primary,
                                    icon: Icons.devices_outlined,
                                    compact: true,
                                  ),
                                  SemanticChip(
                                    label: countNoun(
                                      siteNetworkCount(site),
                                      'rede IP',
                                      'redes IP',
                                    ),
                                    tone: SemanticTone.info,
                                    icon: Icons.lan_outlined,
                                    compact: true,
                                  ),
                                ],
                              ),
                              if (site.description != null &&
                                  site.description!.isNotEmpty) ...<Widget>[
                                const SizedBox(height: 10),
                                Text(
                                  site.description!,
                                  style: context.textTheme.bodySmall,
                                ),
                              ],
                            ],
                          ),
                        );
                      })
                      .toList(growable: false),
                ),
              const SizedBox(height: 24),
            ],
          ),
        );
      },
    );
  }
}

class _CompactSiteStat extends StatelessWidget {
  const _CompactSiteStat({
    required this.label,
    required this.value,
    required this.tone,
  });

  final String label;
  final String value;
  final SemanticTone tone;

  @override
  Widget build(BuildContext context) {
    final pair = context.semanticColors.resolve(tone, context.colorScheme);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      decoration: BoxDecoration(
        color: pair.container,
        borderRadius: AppTheme.borderRadius,
        border: Border.all(color: pair.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: context.textTheme.labelSmall?.copyWith(
              color: pair.onContainer,
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 2),
          Text(
            value,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: context.textTheme.titleMedium?.copyWith(
              color: pair.onContainer,
              fontWeight: FontWeight.w800,
              fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
            ),
          ),
        ],
      ),
    );
  }
}

class _SiteFormBody extends ConsumerStatefulWidget {
  const _SiteFormBody({
    this.existing,
    required this.onCancel,
    required this.onSaved,
  });

  final SiteModel? existing;
  final VoidCallback onCancel;
  final VoidCallback onSaved;

  @override
  ConsumerState<_SiteFormBody> createState() => _SiteFormBodyState();
}

class _SiteFormBodyState extends ConsumerState<_SiteFormBody> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _locationCtrl;
  late final TextEditingController _descCtrl;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _nameCtrl = TextEditingController(text: widget.existing?.name ?? '');
    _locationCtrl = TextEditingController(
      text: widget.existing?.location ?? '',
    );
    _descCtrl = TextEditingController(text: widget.existing?.description ?? '');
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _locationCtrl.dispose();
    _descCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() => _saving = true);
    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      if (_locationCtrl.text.trim().isNotEmpty)
        'location': _locationCtrl.text.trim(),
      if (_descCtrl.text.trim().isNotEmpty)
        'description': _descCtrl.text.trim(),
    };
    final ok = widget.existing != null
        ? await ref
              .read(sitesProvider.notifier)
              .updateSite(widget.existing!.id, payload)
        : await ref.read(sitesProvider.notifier).createSite(payload);
    if (!mounted) return;
    setState(() => _saving = false);
    if (ok) widget.onSaved();
  }

  @override
  Widget build(BuildContext context) {
    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextFormField(
            controller: _nameCtrl,
            decoration: const InputDecoration(labelText: 'Nome do site *'),
            validator: FormValidators.required('Informe o nome do site.'),
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _locationCtrl,
            decoration: const InputDecoration(
              labelText: 'Localização / Endereço',
              hintText: 'Ex.: Matriz · São Paulo - SP',
            ),
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _descCtrl,
            maxLines: 3,
            decoration: const InputDecoration(labelText: 'Descrição / Contato'),
          ),
          const SizedBox(height: 16),
          SymmetricalActionBar(
            children: <Widget>[
              OutlinedButton(
                onPressed: _saving ? null : widget.onCancel,
                child: const Text('Cancelar'),
              ),
              FilledButton.icon(
                onPressed: _saving ? null : _submit,
                icon: const Icon(Icons.check),
                label: Text(widget.existing != null ? 'Salvar' : 'Criar site'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/pages/vpn_layout.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Página `/vpn` — Estado do servidor WireGuard, preflight e configuração da interface.
class VpnServerPage extends ConsumerStatefulWidget {
  const VpnServerPage({super.key});

  @override
  ConsumerState<VpnServerPage> createState() => _VpnServerPageState();
}

class _VpnServerPageState extends ConsumerState<VpnServerPage> {
  final _formKey = GlobalKey<FormState>();
  final _endpointCtrl = TextEditingController();
  final _listenPortCtrl = TextEditingController(text: '51820');
  final _cidrCtrl = TextEditingController(text: '10.8.0.0/24');
  final _mtuCtrl = TextEditingController(text: '1420');
  final _dnsCtrl = TextEditingController();
  bool _allowPeerToPeer = false;
  bool _hydrated = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await ref.read(vpnProvider.notifier).fetchServer(includeAuxiliary: true);
      if (mounted) _hydrateFromState();
    });
  }

  @override
  void dispose() {
    _endpointCtrl.dispose();
    _listenPortCtrl.dispose();
    _cidrCtrl.dispose();
    _mtuCtrl.dispose();
    _dnsCtrl.dispose();
    super.dispose();
  }

  void _hydrateFromState() {
    final s = ref.read(vpnProvider).serverState;
    if (s == null) return;
    final srv = s.server;
    _endpointCtrl.text = srv?.publicEndpoint ?? '';
    _listenPortCtrl.text = (srv?.listenPort ?? 51820).toString();
    _cidrCtrl.text = s.cidr ?? _firstSuggestedCidr();
    _mtuCtrl.text = (srv?.mtu ?? 1420).toString();
    _dnsCtrl.text = srv?.dnsServers ?? '';
    setState(() {
      _allowPeerToPeer = srv?.allowPeerToPeer ?? false;
      _hydrated = true;
    });
  }

  String _firstSuggestedCidr() {
    final free = freeVpnCidrs(ref.read(vpnProvider).existingCidrs);
    return free.isNotEmpty ? free.first : '10.8.0.0/24';
  }

  Future<void> _detectEndpoint() async {
    final detected = await ref.read(vpnProvider.notifier).detectEndpoint();
    if (!mounted) return;
    if (detected != null && detected.isNotEmpty) {
      setState(() => _endpointCtrl.text = detected);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Endereço público detectado: $detected')),
      );
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Não foi possível detectar automaticamente o IP público.',
          ),
        ),
      );
    }
  }

  Future<void> _save() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;

    final current = ref.read(vpnProvider).serverState;
    final previousEndpoint = current?.server?.publicEndpoint?.trim() ?? '';
    final nextEndpoint = _endpointCtrl.text.trim();
    final peersTotal = current?.peersTotal ?? 0;

    // Confirmação obrigatória se mudar endpoint público com peers já ativos.
    if (previousEndpoint.isNotEmpty &&
        nextEndpoint != previousEndpoint &&
        peersTotal > 0) {
      final confirmed = await showConfirmActionDialog(
        context: context,
        title: 'Alterar endereço público?',
        message:
            'Existem ${countNoun(peersTotal, 'dispositivo', 'dispositivos')} configurados com "$previousEndpoint". '
            'Eles perderão o túnel até serem reconfigurados com o novo endereço.',
        confirmLabel: 'Salvar mesmo assim',
        tone: SemanticTone.warning,
        icon: Icons.public_off_outlined,
      );
      if (!confirmed || !mounted) return;
    }

    final ok = await ref
        .read(vpnProvider.notifier)
        .saveServer(
          publicEndpoint: nextEndpoint.isEmpty ? null : nextEndpoint,
          listenPort: int.tryParse(_listenPortCtrl.text.trim()) ?? 51820,
          cidr: _cidrCtrl.text.trim(),
          mtu: int.tryParse(_mtuCtrl.text.trim()) ?? 1420,
          dnsServers: _dnsCtrl.text.trim().isEmpty
              ? null
              : _dnsCtrl.text.trim(),
          allowPeerToPeer: _allowPeerToPeer,
        );
    if (!mounted) return;
    if (ok) {
      _hydrateFromState();
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Configuração do servidor VPN salva.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final vpn = ref.watch(vpnProvider);
    final auth = ref.watch(authProvider);
    final canWrite = auth.canWriteIn(ProductArea.vpn);
    final serverState = vpn.serverState;

    if (!_hydrated && serverState != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_hydrated) _hydrateFromState();
      });
    }

    final scheme = context.colorScheme;
    final configured = vpn.isConfigured;
    final peersTotal = serverState?.peersTotal ?? 0;
    final peersConnected = serverState?.peersConnected ?? 0;

    final statusLabel = !configured
        ? 'NÃO CONFIGURADO'
        : (peersConnected > 0 ? 'ATIVO' : 'AGUARDANDO CONEXÕES');
    final statusTone = !configured
        ? SemanticTone.secondary
        : (peersConnected > 0 ? SemanticTone.success : SemanticTone.warning);

    VpnServerAddressHint? publicOverride;
    for (final addr in vpn.addresses) {
      if (addr.id == 'public' && addr.overridden) {
        publicOverride = addr;
        break;
      }
    }
    final hasDivergence =
        publicOverride != null &&
        publicOverride.address.isNotEmpty &&
        _endpointCtrl.text.trim().isNotEmpty &&
        publicOverride.address.trim() != _endpointCtrl.text.trim();

    final cidrValue = _cidrCtrl.text.trim();
    final overlappingCidr = cidrValue.isNotEmpty
        ? vpn.existingCidrs
              .where(
                (other) =>
                    other != serverState?.cidr &&
                    cidrOverlaps(cidrValue, other),
              )
              .firstOrNull
        : null;
    final freeCidrs = freeVpnCidrs(
      vpn.existingCidrs.where((c) => c != serverState?.cidr),
    );
    final cidrLocked = configured && peersTotal > 0;

    final isCompact = MediaQuery.sizeOf(context).width < 560;
    final preflightBtn = FilledButton.tonalIcon(
      onPressed: vpn.testing
          ? null
          : () => ref
                .read(vpnProvider.notifier)
                .runPreflight(
                  publicEndpoint: _endpointCtrl.text.trim().isEmpty
                      ? null
                      : _endpointCtrl.text.trim(),
                  listenPort:
                      int.tryParse(_listenPortCtrl.text.trim()) ?? 51820,
                ),
      icon: vpn.testing
          ? const SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(strokeWidth: 2),
            )
          : const Icon(Icons.network_check, size: 18),
      label: const Text('Testar acessibilidade'),
    );

    return VpnLayout(
      activeTab: 'server',
      slivers: <Widget>[
        if (vpn.loading && serverState == null)
          const SliverFillRemaining(
            hasScrollBody: false,
            child: LoadingStateView(message: 'Carregando servidor VPN...'),
          )
        else
          SliverToBoxAdapter(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (vpn.error != null) ...<Widget>[
                  NestedPanel(
                    tone: SemanticTone.error,
                    child: Text(
                      vpn.error!,
                      style: context.textTheme.bodySmall?.copyWith(
                        color: context.semanticColors
                            .resolve(SemanticTone.error, scheme)
                            .onContainer,
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                // 1. Cartão de estado do serviço
                SectionCard(
                  title: 'Estado do serviço WireGuard',
                  subtitle: serverState?.server?.interfaceName ?? 'wg0',
                  icon: Icons.shield_outlined,
                  trailing: SemanticChip(
                    label: statusLabel,
                    tone: statusTone,
                    compact: true,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      ResponsiveGrid(
                        xs: 2,
                        sm: 2,
                        md: 4,
                        lg: 4,
                        xl: 4,
                        children: <Widget>[
                          KpiCard(
                            label: 'Peers conectados',
                            value: '$peersConnected / $peersTotal',
                            icon: Icons.devices_outlined,
                            tone: peersConnected > 0
                                ? SemanticTone.success
                                : SemanticTone.info,
                          ),
                          KpiCard(
                            label: 'Tráfego RX',
                            value: formatBytes(serverState?.bytesRx ?? 0),
                            icon: Icons.south_west,
                            tone: SemanticTone.success,
                          ),
                          KpiCard(
                            label: 'Tráfego TX',
                            value: formatBytes(serverState?.bytesTx ?? 0),
                            icon: Icons.north_east,
                            tone: SemanticTone.info,
                          ),
                          KpiCard(
                            label: 'IP do servidor',
                            value: serverState?.serverAddress ?? '—',
                            icon: Icons.hub_outlined,
                            tone: SemanticTone.primary,
                            subtitle: serverState?.cidr,
                          ),
                        ],
                      ),
                      if (serverState?.server?.publicKey.isNotEmpty ==
                          true) ...<Widget>[
                        const SizedBox(height: 12),
                        KeyValueRow(
                          label: 'Chave pública do servidor',
                          value: serverState!.server!.publicKey,
                          monospace: true,
                          copyable: true,
                        ),
                      ],
                    ],
                  ),
                ),
                const SizedBox(height: 16),

                // 2. Cartão de Preflight (acessibilidade externa)
                SectionCard(
                  title: 'Acessibilidade externa',
                  subtitle:
                      'Verifica se a porta UDP configurada responde pelo endereço público.',
                  icon: Icons.travel_explore_outlined,
                  trailing: isCompact ? null : preflightBtn,
                  footer: isCompact
                      ? Align(
                          alignment: Alignment.centerRight,
                          child: preflightBtn,
                        )
                      : null,
                  child: vpn.preflight == null
                      ? Text(
                          'Execute o teste para verificar NAT, CGNAT, resolução DNS e porta UDP.',
                          style: context.textTheme.bodySmall?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        )
                      : NestedPanel(
                          tone: vpn.preflight!.tone,
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Text(
                                vpn.preflight!.message,
                                style: context.textTheme.bodyMedium?.copyWith(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              if (vpn.preflight!.recommendation != null &&
                                  vpn
                                      .preflight!
                                      .recommendation!
                                      .isNotEmpty) ...<Widget>[
                                const SizedBox(height: 4),
                                Text(
                                  vpn.preflight!.recommendation!,
                                  style: context.textTheme.bodySmall,
                                ),
                              ],
                              const SizedBox(height: 8),
                              Wrap(
                                spacing: 8,
                                runSpacing: 6,
                                children: <Widget>[
                                  if (vpn.preflight!.publicIp != null)
                                    SemanticChip(
                                      label:
                                          'IP público: ${vpn.preflight!.publicIp}',
                                      tone: SemanticTone.info,
                                      compact: true,
                                    ),
                                  SemanticChip(
                                    label: 'Porta UDP ${vpn.preflight!.port}',
                                    tone: SemanticTone.primary,
                                    compact: true,
                                  ),
                                  if (vpn.preflight!.isCgnat)
                                    const SemanticChip(
                                      label: 'CGNAT detectado',
                                      tone: SemanticTone.warning,
                                      compact: true,
                                    ),
                                  if (!vpn.preflight!.verified)
                                    const SemanticChip(
                                      label: 'Verificação parcial',
                                      tone: SemanticTone.warning,
                                      compact: true,
                                    ),
                                ],
                              ),
                            ],
                          ),
                        ),
                ),
                const SizedBox(height: 16),

                // 3. Configuração do servidor
                SectionCard(
                  title: 'Configuração da interface',
                  subtitle:
                      'Parâmetros aplicados ao WireGuard pelo watcher local.',
                  icon: Icons.tune,
                  child: Form(
                    key: _formKey,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        if (hasDivergence) ...<Widget>[
                          NestedPanel(
                            tone: SemanticTone.warning,
                            child: Wrap(
                              spacing: 8,
                              runSpacing: 6,
                              alignment: WrapAlignment.spaceBetween,
                              crossAxisAlignment: WrapCrossAlignment.center,
                              children: <Widget>[
                                const Text(
                                  'O endereço público definido na central difere do endpoint da VPN.',
                                ),
                                TextButton(
                                  onPressed: canWrite
                                      ? () => setState(
                                          () => _endpointCtrl.text =
                                              publicOverride!.address,
                                        )
                                      : null,
                                  child: Text('Usar ${publicOverride.address}'),
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(height: 12),
                        ],
                        TextFormField(
                          controller: _endpointCtrl,
                          enabled: canWrite,
                          decoration: InputDecoration(
                            labelText: 'Endpoint público (IP ou domínio)',
                            hintText: 'vpn.seudominio.com.br',
                            suffixIcon: IconButton(
                              tooltip: 'Detectar endereço público',
                              onPressed: canWrite && !vpn.testing
                                  ? _detectEndpoint
                                  : null,
                              icon: const Icon(Icons.my_location_outlined),
                            ),
                          ),
                        ),
                        if (vpn.addresses.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 8),
                          Wrap(
                            spacing: 6,
                            runSpacing: 6,
                            children: vpn.addresses
                                .take(4)
                                .map((addr) {
                                  return ActionChip(
                                    label: Text(
                                      '${addr.label}: ${addr.address}',
                                    ),
                                    onPressed: canWrite
                                        ? () => setState(
                                            () => _endpointCtrl.text =
                                                addr.address,
                                          )
                                        : null,
                                  );
                                })
                                .toList(growable: false),
                          ),
                        ],
                        const SizedBox(height: 12),
                        ResponsiveFieldRow(
                          breakpoint: 640,
                          spacing: 12,
                          runSpacing: 12,
                          children: <Widget>[
                            TextFormField(
                              controller: _listenPortCtrl,
                              enabled: canWrite,
                              keyboardType: TextInputType.number,
                              decoration: const InputDecoration(
                                labelText: 'Porta UDP *',
                                hintText: '51820',
                              ),
                              validator: FormValidators.compose(
                                <FormFieldValidator<String>>[
                                  FormValidators.required(
                                    'Porta entre 1 e 65535',
                                  ),
                                  FormValidators.port('Porta entre 1 e 65535'),
                                ],
                              ),
                            ),
                            TextFormField(
                              controller: _cidrCtrl,
                              enabled: canWrite && !cidrLocked,
                              onChanged: (_) => setState(() {}),
                              decoration: InputDecoration(
                                labelText: 'Faixa CIDR da VPN *',
                                hintText: '10.8.0.0/24',
                                helperText: cidrLocked
                                    ? 'Bloqueado enquanto houver peers ativos'
                                    : null,
                              ),
                              validator: (v) {
                                final parsed = parseIpv4Cidr(v ?? '');
                                if (parsed == null ||
                                    parsed.prefix < 8 ||
                                    parsed.prefix > 30) {
                                  return 'CIDR IPv4 inválido (ex.: 10.8.0.0/24)';
                                }
                                return null;
                              },
                            ),
                            TextFormField(
                              controller: _mtuCtrl,
                              enabled: canWrite,
                              keyboardType: TextInputType.number,
                              decoration: const InputDecoration(
                                labelText: 'MTU',
                                hintText: '1420',
                              ),
                              validator: FormValidators.compose(
                                <FormFieldValidator<String>>[
                                  FormValidators.required(
                                    'MTU entre 1280 e 1500',
                                  ),
                                  FormValidators.integerRange(
                                    1280,
                                    1500,
                                    message: 'MTU entre 1280 e 1500',
                                  ),
                                ],
                              ),
                            ),
                          ],
                        ),
                        if (!cidrLocked && freeCidrs.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 8),
                          Wrap(
                            spacing: 6,
                            runSpacing: 6,
                            children: freeCidrs
                                .take(4)
                                .map((cidr) {
                                  return ActionChip(
                                    label: Text(cidr),
                                    onPressed: canWrite
                                        ? () => setState(
                                            () => _cidrCtrl.text = cidr,
                                          )
                                        : null,
                                  );
                                })
                                .toList(growable: false),
                          ),
                        ],
                        if (overlappingCidr != null) ...<Widget>[
                          const SizedBox(height: 8),
                          NestedPanel(
                            tone: SemanticTone.warning,
                            child: Text(
                              'Atenção: a faixa $cidrValue sobrepõe a rede já cadastrada $overlappingCidr.',
                            ),
                          ),
                        ],
                        const SizedBox(height: 12),
                        TextFormField(
                          controller: _dnsCtrl,
                          enabled: canWrite,
                          decoration: const InputDecoration(
                            labelText: 'Servidores DNS (opcional)',
                            hintText: '1.1.1.1, 8.8.8.8',
                          ),
                        ),
                        const SizedBox(height: 8),
                        SwitchListTile(
                          contentPadding: EdgeInsets.zero,
                          title: const Text('Permitir tráfego entre peers'),
                          subtitle: const Text(
                            'Por padrão os dispositivos conversam apenas com a central (isolamento de clientes).',
                          ),
                          value: _allowPeerToPeer,
                          onChanged: canWrite
                              ? (v) => setState(() => _allowPeerToPeer = v)
                              : null,
                        ),
                        const SizedBox(height: 14),
                        Align(
                          alignment: Alignment.centerRight,
                          child: FilledButton.icon(
                            onPressed: canWrite && !vpn.saving ? _save : null,
                            icon: vpn.saving
                                ? const SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.save_outlined),
                            label: Text(
                              configured
                                  ? 'Salvar configuração'
                                  : 'Ativar servidor VPN',
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

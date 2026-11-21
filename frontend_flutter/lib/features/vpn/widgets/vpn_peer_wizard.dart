import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/features/vpn/data/vpn_repository.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Pré-configuração usada por `VpnServerConnect` ao conectar um servidor remoto via VPN.
@immutable
class VpnPeerWizardPreset {
  const VpnPeerWizardPreset({this.name, this.agentId});

  final String? name;
  final int? agentId;
}

/// Abre o assistente em 2 etapas para criar um novo peer WireGuard.
Future<VpnArtifact?> showVpnPeerWizard({
  required BuildContext context,
  VpnPeerWizardPreset? preset,
}) {
  return showAdaptiveFormSheet<VpnArtifact>(
    context: context,
    title: preset != null
        ? 'Conectar servidor pela VPN'
        : 'Adicionar dispositivo VPN',
    subtitle: preset != null
        ? 'Gera túnel WireGuard + instalador do agente em um único passo.'
        : 'Escolha o perfil do equipamento e defina a identificação na VPN.',
    maxWidth: 640,
    builder: (sheetCtx) => _VpnPeerWizardBody(preset: preset),
  );
}

class _VpnPeerWizardBody extends ConsumerStatefulWidget {
  const _VpnPeerWizardBody({this.preset});

  final VpnPeerWizardPreset? preset;

  @override
  ConsumerState<_VpnPeerWizardBody> createState() => _VpnPeerWizardBodyState();
}

class _VpnPeerWizardBodyState extends ConsumerState<_VpnPeerWizardBody> {
  final _formKey = GlobalKey<FormState>();
  int _step = 0;
  String? _selectedProfile;
  late TextEditingController _nameCtrl;
  late TextEditingController _ipCtrl;
  late TextEditingController _snmpCommunityCtrl;
  late TextEditingController _descriptionCtrl;
  late TextEditingController _dbTargetsCtrl;
  int? _siteId;
  bool _snmpEnabled = false;
  bool _installAgent = false;
  List<String> _agentAllow = <String>['metrics', 'docker_read'];
  bool _loadingIp = false;
  bool _submitting = false;

  bool get _isPreset => widget.preset != null;

  @override
  void initState() {
    super.initState();
    _nameCtrl = TextEditingController(text: widget.preset?.name ?? '');
    _ipCtrl = TextEditingController();
    _snmpCommunityCtrl = TextEditingController(text: 'public');
    _descriptionCtrl = TextEditingController();
    _dbTargetsCtrl = TextEditingController();

    if (_isPreset) {
      _selectedProfile = 'linux';
      _installAgent = true;
      _step = 1;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      _loadInitialData();
    });
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _ipCtrl.dispose();
    _snmpCommunityCtrl.dispose();
    _descriptionCtrl.dispose();
    _dbTargetsCtrl.dispose();
    super.dispose();
  }

  Future<void> _loadInitialData() async {
    final notifier = ref.read(vpnProvider.notifier);
    await Future.wait<void>(<Future<void>>[
      if (ref.read(vpnProvider).serverState == null) notifier.fetchServer(),
      notifier.fetchSites(),
      _refreshNextIp(),
      _loadSuggestedPolicy(),
    ]);
  }

  Future<void> _loadSuggestedPolicy() async {
    final policy = await ref
        .read(vpnRepositoryProvider)
        .fetchSuggestedAgentPolicy();
    if (!mounted) return;
    setState(() {
      _agentAllow = List<String>.from(policy.allow);
      _dbTargetsCtrl.text = policy.databaseTargets;
    });
  }

  Future<void> _refreshNextIp() async {
    setState(() => _loadingIp = true);
    try {
      final suggested = await ref.read(vpnProvider.notifier).suggestNextIp();
      if (mounted && suggested != null && suggested.isNotEmpty) {
        _ipCtrl.text = suggested;
      }
    } finally {
      if (mounted) setState(() => _loadingIp = false);
    }
  }

  Future<void> _submit() async {
    if (_selectedProfile == null) return;
    if (!(_formKey.currentState?.validate() ?? false)) return;

    setState(() => _submitting = true);
    try {
      final installAgent = _selectedProfile == 'linux' && _installAgent;
      final artifact = await ref
          .read(vpnProvider.notifier)
          .createPeer(
            CreateVpnPeerPayload(
              name: _nameCtrl.text.trim(),
              profile: _selectedProfile!,
              ipAddress: _ipCtrl.text.trim().isEmpty
                  ? null
                  : _ipCtrl.text.trim(),
              siteId: _siteId,
              snmpEnabled: _snmpEnabled,
              snmpCommunity: _snmpEnabled
                  ? _snmpCommunityCtrl.text.trim()
                  : null,
              description: _descriptionCtrl.text.trim().isEmpty
                  ? null
                  : _descriptionCtrl.text.trim(),
              installAgent: installAgent,
              agentId: widget.preset?.agentId,
              agentPolicy: installAgent
                  ? VpnInstallPolicy(
                      allow: List<String>.unmodifiable(_agentAllow),
                      databaseTargets: _dbTargetsCtrl.text.trim(),
                    )
                  : null,
            ),
          );
      if (artifact != null && mounted) {
        Navigator.of(context).pop(artifact);
      }
    } finally {
      if (mounted) setState(() => _submitting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final vpn = ref.watch(vpnProvider);
    final profiles = vpn.profiles;
    final scheme = context.colorScheme;

    if (!_isPreset && _step == 0) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            '1. Escolha o equipamento',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 10),
          if (profiles.isEmpty)
            const LoadingStateView(message: 'Carregando perfis...')
          else
            ResponsiveGrid(
              xs: 1,
              sm: 2,
              md: 2,
              lg: 2,
              xl: 2,
              children: profiles
                  .map((card) {
                    return ChoiceCard(
                      title: card.label,
                      subtitle: card.supportsQrCode
                          ? 'Configuração via QR Code ou arquivo'
                          : 'Script guiado / arquivo',
                      icon: iconFromMdiName(card.icon),
                      badge: card.supportsQrCode
                          ? 'QR Code'
                          : 'Script / arquivo',
                      badgeTone: card.supportsQrCode
                          ? SemanticTone.success
                          : SemanticTone.info,
                      selected: _selectedProfile == card.profile,
                      onTap: () {
                        setState(() {
                          _selectedProfile = card.profile;
                          _step = 1;
                        });
                      },
                    );
                  })
                  .toList(growable: false),
            ),
        ],
      );
    }

    final activeProfile = _selectedProfile ?? 'linux';

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (!_isPreset)
            Padding(
              padding: const EdgeInsets.only(bottom: 12),
              child: Row(
                children: <Widget>[
                  TextButton.icon(
                    onPressed: () => setState(() => _step = 0),
                    icon: const Icon(Icons.arrow_back, size: 18),
                    label: const Text('Trocar perfil'),
                  ),
                  const Spacer(),
                  SemanticChip(
                    label: vpnProfileLabel(activeProfile, profiles),
                    tone: SemanticTone.primary,
                    icon: vpnProfileIconData(activeProfile, profiles),
                    compact: true,
                  ),
                ],
              ),
            ),
          if (_isPreset) ...<Widget>[
            NestedPanel(
              tone: SemanticTone.info,
              child: Text(
                'O agente remoto se conectará à central exclusivamente por dentro do túnel WireGuard, sem expor portas na internet.',
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.semanticColors
                      .resolve(SemanticTone.info, scheme)
                      .onContainer,
                ),
              ),
            ),
            if (widget.preset?.agentId != null) ...<Widget>[
              const SizedBox(height: 8),
              NestedPanel(
                tone: SemanticTone.warning,
                child: Text(
                  'Este servidor já está cadastrado. Ao executar o script, ele migrará a conexão atual para o túnel VPN.',
                  style: context.textTheme.bodySmall?.copyWith(
                    color: context.semanticColors
                        .resolve(SemanticTone.warning, scheme)
                        .onContainer,
                  ),
                ),
              ),
            ],
            const SizedBox(height: 12),
          ],
          TextFormField(
            controller: _nameCtrl,
            decoration: const InputDecoration(
              labelText: 'Nome do dispositivo *',
              hintText: 'Ex.: Roteador Filial Centro',
            ),
            validator: FormValidators.required(
              'Informe o nome do dispositivo.',
            ),
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _ipCtrl,
            decoration: InputDecoration(
              labelText: 'IP fixo na VPN',
              hintText: '10.8.0.2',
              suffixIcon: IconButton(
                tooltip: 'Sugerir próximo IP livre',
                onPressed: _loadingIp ? null : _refreshNextIp,
                icon: _loadingIp
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.refresh),
              ),
            ),
            validator: FormValidators.ipv4('Informe um endereço IPv4 válido.'),
          ),
          if (vpn.sites.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            DropdownButtonFormField<int?>(
              initialValue: _siteId,
              decoration: const InputDecoration(labelText: 'Site (opcional)'),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text('Nenhum'),
                ),
                for (final site in vpn.sites)
                  DropdownMenuItem<int?>(
                    value: site.id,
                    child: Text(site.name),
                  ),
              ],
              onChanged: (value) => setState(() => _siteId = value),
            ),
          ],
          const SizedBox(height: 12),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Monitorar por SNMP'),
            subtitle: const Text(
              'Coleta métricas do equipamento pelo túnel VPN.',
            ),
            value: _snmpEnabled,
            onChanged: (value) => setState(() => _snmpEnabled = value),
          ),
          if (_snmpEnabled) ...<Widget>[
            const SizedBox(height: 8),
            TextFormField(
              controller: _snmpCommunityCtrl,
              decoration: const InputDecoration(
                labelText: 'Comunidade SNMP',
                hintText: 'public',
              ),
            ),
          ],
          if (activeProfile == 'linux') ...<Widget>[
            const Divider(height: 24),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Conectar este servidor à central'),
              subtitle: const Text(
                'Inclui a instalação do agente NetMonitor no script Linux.',
              ),
              value: _installAgent,
              onChanged: _isPreset
                  ? null
                  : (value) => setState(() => _installAgent = value),
            ),
            if (_installAgent) ...<Widget>[
              const SizedBox(height: 6),
              Text(
                'Permissões iniciais do agente (AGENT_ALLOW)',
                style: context.textTheme.labelMedium?.copyWith(
                  color: scheme.onSurfaceVariant,
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 6,
                children: vpnAgentPermissions
                    .map((perm) {
                      final selected = _agentAllow.contains(perm.id);
                      return FilterChip(
                        label: Text(perm.label),
                        tooltip: perm.description,
                        selected: selected,
                        onSelected: (value) {
                          setState(() {
                            if (value) {
                              _agentAllow.add(perm.id);
                            } else {
                              _agentAllow.remove(perm.id);
                            }
                          });
                        },
                      );
                    })
                    .toList(growable: false),
              ),
              if (_agentAllow.contains('database')) ...<Widget>[
                const SizedBox(height: 10),
                TextFormField(
                  controller: _dbTargetsCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Destinos de banco permitidos',
                    hintText: '10.0.0.10:5432, 10.0.0.11:3306',
                  ),
                ),
              ],
            ],
          ],
          const SizedBox(height: 12),
          TextFormField(
            controller: _descriptionCtrl,
            maxLines: 2,
            decoration: const InputDecoration(
              labelText: 'Observações (opcional)',
            ),
          ),
          if (vpn.error != null) ...<Widget>[
            const SizedBox(height: 12),
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
          ],
          const SizedBox(height: 16),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: <Widget>[
              TextButton(
                onPressed: _submitting
                    ? null
                    : () => Navigator.of(context).pop(),
                child: const Text('Cancelar'),
              ),
              const SizedBox(width: 8),
              FilledButton.icon(
                onPressed: _submitting ? null : _submit,
                icon: _submitting
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.vpn_key_outlined),
                label: const Text('Criar e gerar chaves'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

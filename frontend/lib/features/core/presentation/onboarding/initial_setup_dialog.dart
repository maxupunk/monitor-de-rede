import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/notifications/notifications_service.dart';
import 'package:netmonitor_app/core/notifications/web_notifications_interop.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/network_address_utils.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/features/core/presentation/backup/backup_page.dart';
import 'package:netmonitor_app/features/core/providers/onboarding_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Overlay montado no shell pelo Núcleo (`coreArea.shellOverlays`):
/// - Registra `onboardingTriggerProvider`
/// - Verifica `GET /api/settings/onboarding` após login e abre o assistente quando necessário
/// - Exibe feedback imediato em `backup:failed` via SSE
class OnboardingShellOverlay extends ConsumerStatefulWidget {
  const OnboardingShellOverlay({super.key});

  @override
  ConsumerState<OnboardingShellOverlay> createState() =>
      _OnboardingShellOverlayState();
}

class _OnboardingShellOverlayState
    extends ConsumerState<OnboardingShellOverlay> {
  bool _dialogOpen = false;
  VoidCallback? _unsubBackupFailed;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ref.read(onboardingTriggerProvider.notifier).state = (ctx) {
        ref.read(onboardingProvider.notifier).openWizard();
      };
      ref.read(onboardingProvider.notifier).checkAndOpenIfNeeded();
    });
    _unsubBackupFailed = ref.read(sseServiceProvider.notifier).onEvent(
      <String>['backup:failed'],
      (data, _) {
        if (!mounted) return;
        final kind = data['kind']?.toString();
        final name = (data['name'] ?? 'sistema').toString();
        final prefix = kind == 'database_restore'
            ? 'Falha na restauração'
            : 'Falha no backup';
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('$prefix: $name'),
            backgroundColor: context.colorScheme.error,
          ),
        );
      },
    );
  }

  @override
  void dispose() {
    _unsubBackupFailed?.call();
    super.dispose();
  }

  void _openIfTriggered(bool showWizard) {
    if (!showWizard || _dialogOpen) return;
    _dialogOpen = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      await showInitialSetupWizard(context, ref);
      _dialogOpen = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final onboarding = ref.watch(onboardingProvider);
    _openIfTriggered(onboarding.showWizard);
    return const SizedBox.shrink();
  }
}

/// Abre o Assistente de Configuração Inicial de 13 etapas (`InitialSetupDialog` parity).
Future<void> showInitialSetupWizard(BuildContext context, WidgetRef ref) async {
  final notifier = ref.read(onboardingProvider.notifier);
  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Assistente de configuração inicial',
    subtitle:
        'Configure áreas, rede, endereços, agentes, VPN e notificações passo a passo.',
    maxWidth: 720,
    isDismissible: false,
    builder: (_) => const _OnboardingWizardBody(),
  );
  if (context.mounted) {
    notifier.dismissWizard(persistOnServer: false);
  }
}

enum _WizardStep {
  start('Início'),
  restore('Restaurar backup'),
  intent('Áreas do sistema'),
  addresses('Endereços do servidor'),
  general('Preferências gerais'),
  site('Site inicial'),
  network('Rede IP'),
  dns('Servidores DNS'),
  agents('Agentes remotos'),
  vpn('VPN WireGuard'),
  apps('Serviços SaaS'),
  notifications('Notificações'),
  review('Revisão'),
  done('Concluído');

  const _WizardStep(this.label);
  final String label;
}

class _OnboardingWizardBody extends ConsumerStatefulWidget {
  const _OnboardingWizardBody();

  @override
  ConsumerState<_OnboardingWizardBody> createState() =>
      _OnboardingWizardBodyState();
}

class _OnboardingWizardBodyState extends ConsumerState<_OnboardingWizardBody> {
  _WizardStep _step = _WizardStep.start;

  // Intent
  bool _enableNetwork = true;
  bool _enableServers = true;
  bool _enableVpn = false;

  // Form fields across steps
  final TextEditingController _lanAddrCtrl = TextEditingController();
  final TextEditingController _pingIntervalCtrl = TextEditingController(
    text: '60',
  );
  final TextEditingController _snmpCommCtrl = TextEditingController(
    text: 'public',
  );
  bool _autoDiscovery = true;

  final TextEditingController _siteNameCtrl = TextEditingController(
    text: 'Matriz',
  );
  final TextEditingController _networkNameCtrl = TextEditingController(
    text: 'LAN Principal',
  );
  final TextEditingController _networkCidrCtrl = TextEditingController(
    text: '192.168.1.0/24',
  );
  final TextEditingController _dnsNameCtrl = TextEditingController(
    text: 'Cloudflare DNS',
  );
  final TextEditingController _dnsIpCtrl = TextEditingController(
    text: '1.1.1.1',
  );
  final TextEditingController _agentNameCtrl = TextEditingController();
  final TextEditingController _vpnEndpointCtrl = TextEditingController();
  final Set<String> _selectedSaas = <String>{};
  bool _enableLocalNotifs = true;

  bool _applying = false;
  bool _requestingNotifPermission = false;
  String? _notifFeedback;
  SemanticTone _notifFeedbackTone = SemanticTone.info;
  String? _error;
  String? _createdAgentToken;
  Map<String, dynamic>? _vpnPreflight;

  @override
  void initState() {
    super.initState();
    final st = ref.read(onboardingProvider).status;
    if (st?.detectedLanIp != null) _lanAddrCtrl.text = st!.detectedLanIp!;
    // Endpoint da VPN: o domínio pelo qual a central é acessada (navegador na
    // Web, URL configurada no Android); IP não vira sugestão.
    _vpnEndpointCtrl.text =
        suggestPublicEndpoint(
          resolveCentralOrigin(ref.read(serverConfigProvider).baseUrl),
        ) ??
        '';
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        ref.read(coreSettingsProvider.notifier).fetchServerAddresses();
      }
    });
  }

  @override
  void dispose() {
    _lanAddrCtrl.dispose();
    _pingIntervalCtrl.dispose();
    _snmpCommCtrl.dispose();
    _siteNameCtrl.dispose();
    _networkNameCtrl.dispose();
    _networkCidrCtrl.dispose();
    _dnsNameCtrl.dispose();
    _dnsIpCtrl.dispose();
    _agentNameCtrl.dispose();
    _vpnEndpointCtrl.dispose();
    super.dispose();
  }

  List<_WizardStep> get _activeFlow => <_WizardStep>[
    _WizardStep.start,
    _WizardStep.intent,
    _WizardStep.addresses,
    _WizardStep.general,
    if (_enableNetwork) _WizardStep.site,
    if (_enableNetwork) _WizardStep.network,
    if (_enableNetwork) _WizardStep.dns,
    if (_enableServers) _WizardStep.agents,
    if (_enableVpn) _WizardStep.vpn,
    if (_enableNetwork) _WizardStep.apps,
    _WizardStep.notifications,
    _WizardStep.review,
    _WizardStep.done,
  ];

  Future<void> _goNext() async {
    setState(() => _error = null);
    if (_step == _WizardStep.intent) {
      setState(() => _applying = true);
      try {
        await ref
            .read(areasProvider.notifier)
            .applyIntent(
              OnboardingIntentInput(
                network: _enableNetwork,
                servers: _enableServers,
                vpn: _enableVpn,
              ),
            );
        ref.read(onboardingProvider.notifier).markIntentChosen();
      } catch (err) {
        if (!mounted) return;
        setState(() {
          _applying = false;
          _error = describeError(err);
        });
        return;
      }
      if (!mounted) return;
      setState(() => _applying = false);
    }

    final flow = _activeFlow;
    final idx = flow.indexOf(_step);
    if (idx >= 0 && idx < flow.length - 1) {
      final next = flow[idx + 1];
      setState(() => _step = next);
      if (next == _WizardStep.vpn) {
        _loadVpnPreflight();
      }
    }
  }

  void _goBack() {
    if (_step == _WizardStep.restore) {
      setState(() => _step = _WizardStep.start);
      return;
    }
    final flow = _activeFlow;
    final idx = flow.indexOf(_step);
    if (idx > 0) {
      setState(() => _step = flow[idx - 1]);
    }
  }

  Future<void> _loadVpnPreflight() async {
    final raw = await ref.read(onboardingProvider.notifier).fetchVpnPreflight();
    if (mounted && raw != null) setState(() => _vpnPreflight = raw);
  }

  Future<void> _commitAll() async {
    setState(() {
      _applying = true;
      _error = null;
    });

    try {
      final createdToken = await ref
          .read(onboardingProvider.notifier)
          .commitInitialSetup(
            preferences: SystemPreferences(
              defaultPingIntervalSeconds:
                  (int.tryParse(_pingIntervalCtrl.text.trim()) ?? 60).clamp(
                    10,
                    86400,
                  ),
              autoDiscoveryEnabled: _autoDiscovery,
            ),
            defaultSnmpCommunity: _snmpCommCtrl.text,
            lanAddressOverride: _lanAddrCtrl.text,
            vpnPublicEndpoint: _enableVpn ? _vpnEndpointCtrl.text : null,
            enableNetwork: _enableNetwork,
            siteName: _siteNameCtrl.text,
            networkName: _networkNameCtrl.text,
            networkCidr: _networkCidrCtrl.text,
            dnsName: _dnsNameCtrl.text,
            dnsIp: _dnsIpCtrl.text,
            selectedSaas: _selectedSaas.toList(growable: false),
            enableServers: _enableServers,
            agentName: _agentNameCtrl.text,
            enableLocalNotifications: _enableLocalNotifs,
          );
      if (!mounted) return;
      setState(() {
        _createdAgentToken = createdToken;
        _applying = false;
        _step = _WizardStep.done;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _applying = false;
        _error = describeError(err);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final flow = _activeFlow;
    final stepIdx = flow.indexOf(_step);
    final progress = stepIdx >= 0 ? (stepIdx + 1) / flow.length : 0.1;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        LinearProgressIndicator(value: progress),
        const SizedBox(height: 8),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: <Widget>[
            Expanded(
              child: Text(
                'Etapa: ${_step.label}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            if (stepIdx >= 0) ...<Widget>[
              const SizedBox(width: 8),
              Text(
                '${stepIdx + 1} / ${flow.length}',
                style: context.textTheme.labelSmall,
              ),
            ],
          ],
        ),
        const SizedBox(height: 14),
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 12),
        ],
        _buildStepBody(context),
        const SizedBox(height: 18),
        if (_step != _WizardStep.start && _step != _WizardStep.done)
          Wrap(
            spacing: 8,
            runSpacing: 8,
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              TextButton.icon(
                onPressed: _applying ? null : _goBack,
                icon: const Icon(Icons.arrow_back, size: 18),
                label: const Text('Voltar'),
              ),
              if (_step == _WizardStep.review)
                FilledButton.icon(
                  onPressed: _applying ? null : _commitAll,
                  icon: _applying
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.check_circle_outline, size: 18),
                  label: const Text('Concluir configuração'),
                )
              else if (_step != _WizardStep.restore)
                FilledButton.icon(
                  onPressed: _applying ? null : _goNext,
                  icon: const Icon(Icons.arrow_forward, size: 18),
                  label: const Text('Avançar'),
                ),
            ],
          ),
      ],
    );
  }

  Future<void> _requestOnboardingNotificationPermission() async {
    setState(() {
      _requestingNotifPermission = true;
      _notifFeedback = null;
    });
    final granted = await ref
        .read(notificationsProvider.notifier)
        .requestPermission();
    if (!mounted) return;
    setState(() {
      _requestingNotifPermission = false;
      if (granted) {
        _enableLocalNotifs = true;
        _notifFeedback = 'Notificações ativadas com sucesso neste navegador!';
        _notifFeedbackTone = SemanticTone.success;
      } else {
        final perm = ref.read(notificationsProvider).permissionState;
        _notifFeedback = perm == NotificationPermissionState.denied
            ? 'Permissão bloqueada no navegador. Você pode liberar depois nas configurações do site.'
            : 'Permissão de notificação não concedida.';
        _notifFeedbackTone = perm == NotificationPermissionState.denied
            ? SemanticTone.error
            : SemanticTone.warning;
      }
    });
  }

  Future<void> _sendOnboardingTestNotification() async {
    final shown = await ref
        .read(notificationsProvider.notifier)
        .sendLocalNotification(
          'NetMonitor - Notificação de Teste',
          body:
              'As notificações PWA em tempo real estão configuradas e operacionais!',
          tag: 'netmonitor-setup-test',
          url: '/',
        );
    if (!mounted) return;
    setState(() {
      _notifFeedback = shown
          ? 'Notificação de teste enviada!'
          : 'Não foi possível exibir a notificação de teste agora.';
      _notifFeedbackTone = shown ? SemanticTone.success : SemanticTone.warning;
    });
  }

  Widget _buildNotificationsStep(BuildContext context) {
    final notifs = ref.watch(notificationsProvider);
    final perm = notifs.permissionState;
    final (chipLabel, chipTone) = switch (perm) {
      NotificationPermissionState.granted => (
        'PERMITIDO',
        SemanticTone.success,
      ),
      NotificationPermissionState.denied => ('BLOQUEADO', SemanticTone.error),
      NotificationPermissionState.unsupported => (
        'INDISPONÍVEL',
        SemanticTone.secondary,
      ),
      NotificationPermissionState.defaultState => (
        'NÃO SOLICITADO',
        SemanticTone.warning,
      ),
    };
    final warnColors = context.semanticColors.resolve(
      SemanticTone.warning,
      context.colorScheme,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          padding: const EdgeInsets.all(14),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Container(
                    width: 40,
                    height: 40,
                    decoration: BoxDecoration(
                      color: warnColors.container,
                      borderRadius: AppTheme.borderRadius,
                    ),
                    child: Icon(
                      Icons.notifications_active_outlined,
                      color: warnColors.onContainer,
                      size: 22,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          'Notificações PWA do Navegador',
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Receba alertas imediatos na área de trabalho quando dispositivos caírem.',
                          style: context.textTheme.bodySmall,
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 8),
                  SemanticChip(label: chipLabel, tone: chipTone, compact: true),
                ],
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: <Widget>[
                  if (perm != NotificationPermissionState.granted)
                    FilledButton.icon(
                      onPressed:
                          perm == NotificationPermissionState.unsupported ||
                              _requestingNotifPermission
                          ? null
                          : _requestOnboardingNotificationPermission,
                      icon: _requestingNotifPermission
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Icon(Icons.notifications_active, size: 18),
                      label: const Text('Ativar notificações no navegador'),
                    )
                  else
                    FilledButton.tonalIcon(
                      onPressed: _sendOnboardingTestNotification,
                      icon: const Icon(Icons.notifications_outlined, size: 18),
                      label: const Text('Enviar notificação de teste'),
                    ),
                ],
              ),
              if (_notifFeedback != null) ...<Widget>[
                const SizedBox(height: 10),
                NestedPanel(
                  tone: _notifFeedbackTone,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 8,
                  ),
                  child: Text(
                    _notifFeedback!,
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                      color: context.semanticColors
                          .resolve(_notifFeedbackTone, context.colorScheme)
                          .onContainer,
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
        const SizedBox(height: 10),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _enableLocalNotifs,
          onChanged: (v) => setState(() => _enableLocalNotifs = v),
          title: const Text(
            'Habilitar notificações de alerta neste dispositivo',
          ),
          subtitle: const Text(
            'Você poderá ajustar Web Push em segundo plano em Configurações › Notificações.',
          ),
        ),
      ],
    );
  }

  Widget _buildStepBody(BuildContext context) {
    return switch (_step) {
      _WizardStep.start => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          ChoiceCard(
            title: 'Começar do zero',
            subtitle:
                'Escolha quais áreas ativar e configure rede, servidores e alertas.',
            icon: Icons.rocket_launch_outlined,
            tone: SemanticTone.primary,
            onTap: () => setState(() => _step = _WizardStep.intent),
          ),
          const SizedBox(height: 10),
          ChoiceCard(
            title: 'Restaurar de um backup',
            subtitle:
                'Envie um arquivo .json ou .json.gz de outra instalação do NetMonitor.',
            icon: Icons.settings_backup_restore,
            tone: SemanticTone.info,
            onTap: () => setState(() => _step = _WizardStep.restore),
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton(
              onPressed: () {
                ref
                    .read(onboardingProvider.notifier)
                    .dismissWizard(persistOnServer: true);
                Navigator.of(context).pop();
              },
              child: const Text('Pular por enquanto'),
            ),
          ),
        ],
      ),
      _WizardStep.restore => BackupFileRestoreCard(
        onRestored: (res) async {
          if (res.signInAgain) {
            await ref
                .read(authProvider.notifier)
                .logout(redirectTo: '/login?restaurado=1');
            if (context.mounted) {
              Navigator.of(context).pop();
              context.go('/login?restaurado=1');
            }
          } else {
            await ref.read(onboardingProvider.notifier).completeOnboarding();
            if (context.mounted) {
              setState(() => _step = _WizardStep.done);
            }
          }
        },
      ),
      _WizardStep.intent => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const Text(
            'Quais áreas do NetMonitor você pretende usar nesta central?',
          ),
          const SizedBox(height: 10),
          SwitchListTile(
            value: _enableNetwork,
            onChanged: (v) => setState(() => _enableNetwork = v),
            title: const Text('Rede'),
            subtitle: const Text(
              'Topologia, descoberta, redes IP, sites, Wi-Fi e sondas.',
            ),
          ),
          SwitchListTile(
            value: _enableServers,
            onChanged: (v) => setState(() => _enableServers = v),
            title: const Text('Servidores'),
            subtitle: const Text(
              'Docker em hosts remotos, deploys contínuos e registry.',
            ),
          ),
          SwitchListTile(
            value: _enableVpn,
            onChanged: (v) => setState(() => _enableVpn = v),
            title: const Text('VPN WireGuard'),
            subtitle: const Text(
              'Túnel seguro para alcançar filiais e agentes remotos.',
            ),
          ),
        ],
      ),
      _WizardStep.addresses => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const Text(
            'Confirme o endereço principal desta central que os equipamentos alcançam na LAN:',
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _lanAddrCtrl,
            decoration: const InputDecoration(
              labelText: 'Endereço IP ou domínio LAN',
              hintText: '192.168.1.10',
            ),
          ),
        ],
      ),
      _WizardStep.general => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextField(
            controller: _pingIntervalCtrl,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Intervalo padrão de Ping (s)',
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _snmpCommCtrl,
            obscureText: true,
            decoration: const InputDecoration(
              labelText: 'Comunidade SNMP padrão (v2c)',
              helperText:
                  'Vira a credencial SNMP padrão. Usuários v3 e outras '
                  'comunidades: Configurações › Rede.',
              helperMaxLines: 2,
            ),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _autoDiscovery,
            onChanged: (v) => setState(() => _autoDiscovery = v),
            title: const Text('Ativar descoberta periódica'),
          ),
        ],
      ),
      _WizardStep.site => TextField(
        controller: _siteNameCtrl,
        decoration: const InputDecoration(
          labelText: 'Nome do local / site inicial',
          hintText: 'Ex.: Matriz, Escritório SP',
        ),
      ),
      _WizardStep.network => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextField(
            controller: _networkNameCtrl,
            decoration: const InputDecoration(labelText: 'Nome da rede'),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _networkCidrCtrl,
            decoration: const InputDecoration(
              labelText: 'Sub-rede CIDR',
              hintText: '192.168.1.0/24',
            ),
          ),
        ],
      ),
      _WizardStep.dns => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextField(
            controller: _dnsNameCtrl,
            decoration: const InputDecoration(
              labelText: 'Nome do servidor DNS',
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _dnsIpCtrl,
            decoration: const InputDecoration(
              labelText: 'IP do DNS',
              hintText: '1.1.1.1 ou 8.8.8.8',
            ),
          ),
        ],
      ),
      _WizardStep.agents => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const Text(
            'Opcionalmente crie o primeiro agente remoto para gerenciar servidores Docker:',
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _agentNameCtrl,
            decoration: const InputDecoration(
              labelText: 'Nome do agente remoto (opcional)',
              hintText: 'Ex.: servidor-producao-01',
            ),
          ),
        ],
      ),
      _WizardStep.vpn => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              _vpnPreflight != null
                  ? 'Preflight VPN carregado: ${_vpnPreflight!['ready'] == true ? 'Pronto para ativar' : 'Verifique os requisitos em VPN › Servidor'}'
                  : 'Você poderá finalizar a configuração do WireGuard na tela VPN.',
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _vpnEndpointCtrl,
            decoration: const InputDecoration(
              labelText: 'Endpoint público / DDNS (opcional)',
              hintText: 'vpn.seudominio.com.br',
            ),
          ),
        ],
      ),
      _WizardStep.apps => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const Text(
            'Selecione serviços externos populares para monitorar desde já:',
          ),
          const SizedBox(height: 10),
          for (final (id, label) in const <(String, String)>[
            ('google-http', 'Google'),
            ('cloudflare-http', 'Cloudflare'),
            ('github-http', 'GitHub'),
            ('whatsapp-http', 'WhatsApp'),
            ('microsoft365-http', 'Microsoft 365'),
          ])
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              value: _selectedSaas.contains(id),
              onChanged: (v) => setState(() {
                if (v == true) {
                  _selectedSaas.add(id);
                } else {
                  _selectedSaas.remove(id);
                }
              }),
              title: Text(label),
            ),
        ],
      ),
      _WizardStep.notifications => _buildNotificationsStep(context),
      _WizardStep.review => NestedPanel(
        tone: SemanticTone.primary,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            const Text(
              'Resumo da configuração',
              style: TextStyle(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: 8),
            Text(
              'Áreas: Núcleo'
              '${_enableNetwork ? ', Rede' : ''}'
              '${_enableServers ? ', Servidores' : ''}'
              '${_enableVpn ? ', VPN' : ''}',
            ),
            Text(
              'Ping padrão: ${_pingIntervalCtrl.text}s · SNMP: credencial padrão v2c',
            ),
            if (_enableNetwork)
              Text(
                'Site: ${_siteNameCtrl.text} · Rede: ${_networkCidrCtrl.text}',
              ),
            if (_selectedSaas.isNotEmpty)
              Text('Serviços SaaS: ${_selectedSaas.join(', ')}'),
          ],
        ),
      ),
      _WizardStep.done => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const EmptyStateView(
            title: 'Configuração concluída!',
            description: 'Sua central NetMonitor está pronta para uso.',
            icon: Icons.verified_outlined,
            tone: SemanticTone.success,
          ),
          if (_createdAgentToken != null) ...<Widget>[
            CodeViewer(
              title: 'Token do agente criado',
              code: _createdAgentToken!,
            ),
            const SizedBox(height: 12),
          ],
          Wrap(
            spacing: 8,
            runSpacing: 8,
            alignment: WrapAlignment.center,
            children: <Widget>[
              FilledButton(
                onPressed: () {
                  Navigator.of(context).pop();
                  context.go('/');
                },
                child: const Text('Ir para o Dashboard'),
              ),
              OutlinedButton(
                onPressed: () {
                  Navigator.of(context).pop();
                  context.go('/monitors');
                },
                child: const Text('Abrir Monitores'),
              ),
            ],
          ),
        ],
      ),
    };
  }
}

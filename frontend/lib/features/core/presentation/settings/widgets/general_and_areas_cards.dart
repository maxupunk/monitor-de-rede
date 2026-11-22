import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/core/utils/theme_preference.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/features/core/providers/dashboard_provider.dart';
import 'package:netmonitor_app/features/core/providers/onboarding_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// 1. PreferencesCard (Geral + Aparência/Tema + Personalização de Menu)
class PreferencesCard extends ConsumerStatefulWidget {
  const PreferencesCard({super.key});

  @override
  ConsumerState<PreferencesCard> createState() => _PreferencesCardState();
}

class _PreferencesCardState extends ConsumerState<PreferencesCard> {
  final TextEditingController _pingCtrl = TextEditingController(text: '60');
  final TextEditingController _snmpCtrl = TextEditingController(text: 'public');
  bool _autoDiscovery = true;
  bool _hydrated = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(coreSettingsProvider.notifier).fetchPreferences();
    });
  }

  @override
  void dispose() {
    _pingCtrl.dispose();
    _snmpCtrl.dispose();
    super.dispose();
  }

  void _syncFromState(SystemPreferences prefs) {
    if (_hydrated) return;
    _pingCtrl.text = '${prefs.defaultPingIntervalSeconds}';
    _snmpCtrl.text = prefs.defaultSnmpCommunity;
    _autoDiscovery = prefs.autoDiscoveryEnabled;
    _hydrated = true;
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(coreSettingsProvider);
    final auth = ref.watch(authProvider);
    final themeMode = ref.watch(themePreferenceProvider);

    if (settings.preferencesLoaded) {
      _syncFromState(settings.preferences);
    }

    return SectionCard(
      title: 'Preferências e Aparência',
      subtitle: 'Padrões de monitoramento, tema visual e organização do menu',
      icon: Icons.tune,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            'Tema da interface',
            style: context.textTheme.labelLarge?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          SegmentedButton<ThemeMode>(
            showSelectedIcon: false,
            style: const ButtonStyle(visualDensity: VisualDensity.compact),
            segments: const <ButtonSegment<ThemeMode>>[
              ButtonSegment<ThemeMode>(
                value: ThemeMode.light,
                icon: Icon(Icons.light_mode_outlined, size: 16),
                label: Text('Claro', overflow: TextOverflow.ellipsis),
              ),
              ButtonSegment<ThemeMode>(
                value: ThemeMode.dark,
                icon: Icon(Icons.dark_mode_outlined, size: 16),
                label: Text('Escuro', overflow: TextOverflow.ellipsis),
              ),
              ButtonSegment<ThemeMode>(
                value: ThemeMode.system,
                icon: Icon(Icons.brightness_auto_outlined, size: 16),
                label: Text('Sistema', overflow: TextOverflow.ellipsis),
              ),
            ],
            selected: <ThemeMode>{themeMode},
            onSelectionChanged: (s) => ref
                .read(themePreferenceProvider.notifier)
                .setThemeMode(s.first),
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerLeft,
            child: OutlinedButton.icon(
              onPressed: () => showMenuCustomizeSheet(context, ref),
              icon: const Icon(Icons.menu_open, size: 18),
              label: const Text('Personalizar menu lateral'),
            ),
          ),
          const Divider(height: 24),
          TextField(
            controller: _pingCtrl,
            enabled: auth.canWrite,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Intervalo padrão de Ping (segundos)',
              helperText: 'Mínimo 10s, máximo 86400s.',
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _snmpCtrl,
            enabled: auth.canWrite,
            decoration: const InputDecoration(
              labelText: 'Comunidade SNMP padrão',
            ),
          ),
          const SizedBox(height: 8),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _autoDiscovery,
            onChanged: auth.canWrite
                ? (v) => setState(() => _autoDiscovery = v)
                : null,
            title: const Text('Descoberta periódica automática'),
          ),
          if (auth.canWrite) ...<Widget>[
            const SizedBox(height: 10),
            FilledButton.icon(
              onPressed: settings.preferencesSaving
                  ? null
                  : () async {
                      final interval = (parseIntOrNull(_pingCtrl.text) ?? 60)
                          .clamp(10, 86400);
                      final ok = await ref
                          .read(coreSettingsProvider.notifier)
                          .savePreferences(
                            SystemPreferences(
                              defaultPingIntervalSeconds: interval,
                              defaultSnmpCommunity:
                                  _snmpCtrl.text.trim().isEmpty
                                  ? 'public'
                                  : _snmpCtrl.text.trim(),
                              autoDiscoveryEnabled: _autoDiscovery,
                            ),
                          );
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text(
                            ok
                                ? 'Preferências salvas.'
                                : 'Erro ao salvar preferências.',
                          ),
                        ),
                      );
                    },
              icon: const Icon(Icons.save_outlined, size: 18),
              label: Text(
                settings.preferencesSaving
                    ? 'Salvando...'
                    : 'Salvar preferências',
              ),
            ),
          ],
        ],
      ),
    );
  }
}

// 2. DashboardSyncCard
class DashboardSyncCard extends ConsumerWidget {
  const DashboardSyncCard({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final dash = ref.watch(dashboardProvider);

    return SectionCard(
      title: 'Sincronização do Dashboard',
      subtitle:
          'Escolha se o layout dos cards sincroniza com o servidor ou fica local',
      icon: Icons.cloud_sync_outlined,
      iconTone: SemanticTone.info,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SegmentedButton<DashboardSyncMode>(
            segments: const <ButtonSegment<DashboardSyncMode>>[
              ButtonSegment<DashboardSyncMode>(
                value: DashboardSyncMode.server,
                label: Text('Servidor'),
                icon: Icon(Icons.cloud_outlined, size: 16),
              ),
              ButtonSegment<DashboardSyncMode>(
                value: DashboardSyncMode.local,
                label: Text('Local'),
                icon: Icon(Icons.devices_outlined, size: 16),
              ),
            ],
            selected: <DashboardSyncMode>{dash.syncMode},
            onSelectionChanged: (s) =>
                ref.read(dashboardProvider.notifier).setSyncMode(s.first),
          ),
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              OutlinedButton.icon(
                onPressed: dash.savingGlobal
                    ? null
                    : () async {
                        final ok = await ref
                            .read(dashboardProvider.notifier)
                            .saveLayoutGlobally();
                        if (!context.mounted) return;
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                            content: Text(
                              ok
                                  ? 'Layout enviado para o servidor.'
                                  : 'Falha ao enviar layout.',
                            ),
                          ),
                        );
                      },
                icon: const Icon(Icons.cloud_upload_outlined, size: 18),
                label: const Text('Enviar para servidor'),
              ),
              OutlinedButton.icon(
                onPressed: dash.loadingServer
                    ? null
                    : () async {
                        await ref
                            .read(dashboardProvider.notifier)
                            .fetchServerLayout();
                        if (!context.mounted) return;
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Layout do servidor consultado.'),
                          ),
                        );
                      },
                icon: const Icon(Icons.cloud_download_outlined, size: 18),
                label: const Text('Baixar do servidor'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

// 3. AreasCard (ADR 017 — impacto e bloqueios antes de desligar)
class AreasCard extends ConsumerWidget {
  const AreasCard({super.key});

  Future<void> _toggleArea(
    BuildContext context,
    WidgetRef ref,
    AreaStateModel item,
    bool nextEnabled,
  ) async {
    if (!nextEnabled) {
      try {
        final impact = await ref
            .read(areasProvider.notifier)
            .fetchImpact(item.area);
        if (!context.mounted) return;
        if (impact.isBlocked) {
          await showDialog<void>(
            context: context,
            builder: (ctx) => AlertDialog(
              title: Text('Não é possível desligar ${item.title}'),
              content: Text(impact.blockerMessage!),
              actions: <Widget>[
                FilledButton(
                  onPressed: () => Navigator.of(ctx).pop(),
                  child: const Text('Entendi'),
                ),
              ],
            ),
          );
          return;
        }
        final confirmed = await showConfirmActionDialog(
          context: context,
          title: 'Desligar área ${item.title}?',
          message:
              'Os dados cadastrados não são apagados, mas as telas e widgets desta área serão ocultados.',
          confirmLabel: 'Desligar área',
          tone: SemanticTone.warning,
          details: impact.consequences.isEmpty
              ? null
              : NestedPanel(
                  tone: SemanticTone.warning,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      for (final c in impact.consequences) Text('• $c'),
                    ],
                  ),
                ),
        );
        if (!confirmed) return;
      } catch (err) {
        if (!context.mounted) return;
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(describeError(err))));
        return;
      }
    }

    try {
      await ref.read(areasProvider.notifier).setEnabled(item.area, nextEnabled);
    } catch (err) {
      if (!context.mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(describeError(err))));
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final areasState = ref.watch(areasProvider);
    final auth = ref.watch(authProvider);

    return SectionCard(
      title: 'Áreas do sistema',
      subtitle:
          'Ative ou desative módulos opcionais conforme o perfil da sua infraestrutura',
      icon: Icons.apps,
      child: Column(
        children: <Widget>[
          for (final item in areasState.states) ...<Widget>[
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: Icon(item.area.defaultIcon),
              title: Wrap(
                spacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  Text(
                    item.title,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  if (!item.optional)
                    const SemanticChip(
                      label: 'Sempre ativa',
                      tone: SemanticTone.primary,
                      compact: true,
                    ),
                ],
              ),
              subtitle: Text(item.summary),
              trailing: item.optional
                  ? Switch(
                      value: item.enabled,
                      onChanged: auth.isAdmin && areasState.saving != item.area
                          ? (v) => _toggleArea(context, ref, item, v)
                          : null,
                    )
                  : const Icon(Icons.lock_outline, size: 18),
            ),
            const Divider(height: 1),
          ],
        ],
      ),
    );
  }
}

// 4. OnboardingCard
class OnboardingCard extends ConsumerStatefulWidget {
  const OnboardingCard({super.key});

  @override
  ConsumerState<OnboardingCard> createState() => _OnboardingCardState();
}

class _OnboardingCardState extends ConsumerState<OnboardingCard> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(onboardingProvider.notifier).fetchStatus();
    });
  }

  @override
  Widget build(BuildContext context) {
    final onboarding = ref.watch(onboardingProvider);
    final auth = ref.watch(authProvider);
    final st = onboarding.status;

    return SectionCard(
      title: 'Assistente de configuração inicial',
      subtitle: st?.completed == true
          ? 'Concluído em ${formatDateTime(st?.completedAt)}'
          : 'Reabra o assistente guiado a qualquer momento para revisar áreas e rede',
      icon: Icons.auto_fix_high,
      iconTone: SemanticTone.primary,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (st == null)
            const Text('Carregando estado da configuração inicial...')
          else
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                SemanticChip(
                  label: '${st.sitesCount} site(s)',
                  tone: SemanticTone.info,
                  compact: true,
                ),
                SemanticChip(
                  label: '${st.networksCount} rede(s)',
                  tone: SemanticTone.info,
                  compact: true,
                ),
                SemanticChip(
                  label: '${st.dnsServersCount} DNS',
                  tone: SemanticTone.info,
                  compact: true,
                ),
                SemanticChip(
                  label: st.vpnConfigured ? 'VPN configurada' : 'VPN pendente',
                  tone: st.vpnConfigured
                      ? SemanticTone.success
                      : SemanticTone.secondary,
                  compact: true,
                ),
              ],
            ),
          if (auth.isAdmin) ...<Widget>[
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerLeft,
              child: FilledButton.tonalIcon(
                onPressed: () {
                  final trigger = ref.read(onboardingTriggerProvider);
                  if (trigger != null) {
                    trigger(context);
                  } else {
                    ref.read(onboardingProvider.notifier).openWizard();
                  }
                },
                icon: const Icon(Icons.play_arrow, size: 18),
                label: const Text('Abrir assistente'),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

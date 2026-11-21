import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/navigation/build_nav_sections.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/theme_preference.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/features/core/providers/backup_provider.dart';
import 'package:netmonitor_app/features/core/providers/dashboard_provider.dart';
import 'package:netmonitor_app/features/core/providers/onboarding_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Cartões de Configurações do Núcleo registrados em `coreArea.settingsCards`.
List<SettingsCardContribution> buildCoreSettingsCards() {
  return <SettingsCardContribution>[
    SettingsCardContribution(
      tab: 'geral',
      order: 10,
      mdCols: 6,
      lgCols: 6,
      builder: (_) => const PreferencesCard(),
    ),
    SettingsCardContribution(
      tab: 'geral',
      order: 20,
      mdCols: 6,
      lgCols: 6,
      builder: (_) => const DashboardSyncCard(),
    ),
    SettingsCardContribution(
      tab: 'geral',
      order: 30,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const AreasCard(),
    ),
    SettingsCardContribution(
      tab: 'geral',
      order: 40,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const OnboardingCard(),
    ),
    SettingsCardContribution(
      tab: 'conectividade',
      order: 10,
      mdCols: 6,
      lgCols: 6,
      builder: (_) => const ServerAddressesCard(),
    ),
    SettingsCardContribution(
      tab: 'notificacoes',
      order: 10,
      mdCols: 8,
      lgCols: 6,
      builder: (_) => const NotificationsCard(),
    ),
    SettingsCardContribution(
      tab: 'sistema',
      order: 10,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const BackupShortcutCard(),
    ),
    SettingsCardContribution(
      tab: 'sistema',
      order: 30,
      mdCols: 12,
      lgCols: 12,
      builder: (_) => const CoreCatalogsCard(),
    ),
  ];
}

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

  Future<void> _openMenuCustomize(BuildContext context) async {
    final auth = ref.read(authProvider);
    final visibleAreas = ref.read(visibleAreasProvider);
    final apps = ref.read(pluginAppsNavProvider);
    final baseSections = buildBaseSections(
      NavContext(isAdmin: auth.isAdmin, apps: apps, enabledAreas: visibleAreas),
      allNavEntries,
    );
    final leaves = navLeaves(baseSections);
    final pinned = List<String>.from(auth.menuPrefs.pinned);
    final hidden = Set<String>.from(auth.menuPrefs.hidden);

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Personalizar menu lateral',
      subtitle:
          'Fixe itens no topo, reordene fixados ou oculte telas pouco usadas.',
      maxWidth: 600,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (pinned.isNotEmpty) ...<Widget>[
                  Text(
                    'Ordem dos fixados',
                    style: context.textTheme.labelLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 6),
                  for (var i = 0; i < pinned.length; i++)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      leading: const Icon(Icons.push_pin, size: 18),
                      title: Text(pinned[i]),
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          IconButton(
                            icon: const Icon(Icons.arrow_upward, size: 18),
                            onPressed: i > 0
                                ? () => setModalState(() {
                                    final tmp = pinned[i - 1];
                                    pinned[i - 1] = pinned[i];
                                    pinned[i] = tmp;
                                  })
                                : null,
                          ),
                          IconButton(
                            icon: const Icon(Icons.arrow_downward, size: 18),
                            onPressed: i < pinned.length - 1
                                ? () => setModalState(() {
                                    final tmp = pinned[i + 1];
                                    pinned[i + 1] = pinned[i];
                                    pinned[i] = tmp;
                                  })
                                : null,
                          ),
                        ],
                      ),
                    ),
                  const Divider(height: 16),
                ],
                for (final entry in leaves) ...<Widget>[
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    leading: Icon(entry.leaf.icon, size: 20),
                    title: Text(entry.leaf.title),
                    subtitle: Text(entry.section),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        IconButton(
                          tooltip: 'Fixar',
                          icon: Icon(
                            pinned.contains(entry.leaf.to)
                                ? Icons.push_pin
                                : Icons.push_pin_outlined,
                            color: pinned.contains(entry.leaf.to)
                                ? context.colorScheme.primary
                                : null,
                          ),
                          onPressed: () {
                            final path = entry.leaf.to;
                            if (path == null) return;
                            setModalState(() {
                              if (pinned.contains(path)) {
                                pinned.remove(path);
                              } else {
                                pinned.add(path);
                                hidden.remove(path);
                              }
                            });
                          },
                        ),
                        IconButton(
                          tooltip: 'Ocultar',
                          icon: Icon(
                            hidden.contains(entry.leaf.to)
                                ? Icons.visibility_off
                                : Icons.visibility_outlined,
                            color: hidden.contains(entry.leaf.to)
                                ? context.colorScheme.error
                                : null,
                          ),
                          onPressed: () {
                            final path = entry.leaf.to;
                            if (path == null) return;
                            setModalState(() {
                              if (hidden.contains(path)) {
                                hidden.remove(path);
                              } else {
                                hidden.add(path);
                                pinned.remove(path);
                              }
                            });
                          },
                        ),
                      ],
                    ),
                  ),
                  const Divider(height: 1),
                ],
                const SizedBox(height: 14),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: <Widget>[
                    TextButton(
                      onPressed: () => setModalState(() {
                        pinned.clear();
                        hidden.clear();
                      }),
                      child: const Text('Restaurar padrão'),
                    ),
                    const SizedBox(width: 8),
                    FilledButton(
                      onPressed: () async {
                        await ref
                            .read(authProvider.notifier)
                            .saveMenuPrefs(
                              MenuPrefs(
                                pinned: List<String>.unmodifiable(pinned),
                                hidden: List<String>.unmodifiable(hidden),
                              ),
                            );
                        if (sheetCtx.mounted) Navigator.of(sheetCtx).pop();
                      },
                      child: const Text('Salvar'),
                    ),
                  ],
                ),
              ],
            );
          },
        );
      },
    );
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
            segments: const <ButtonSegment<ThemeMode>>[
              ButtonSegment<ThemeMode>(
                value: ThemeMode.light,
                icon: Icon(Icons.light_mode_outlined, size: 16),
                label: Text('Claro'),
              ),
              ButtonSegment<ThemeMode>(
                value: ThemeMode.dark,
                icon: Icon(Icons.dark_mode_outlined, size: 16),
                label: Text('Escuro'),
              ),
              ButtonSegment<ThemeMode>(
                value: ThemeMode.system,
                icon: Icon(Icons.brightness_auto_outlined, size: 16),
                label: Text('Sistema'),
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
              onPressed: () => _openMenuCustomize(context),
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
                      final interval =
                          (int.tryParse(_pingCtrl.text.trim()) ?? 60).clamp(
                            10,
                            86400,
                          );
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
      subtitle: 'Escolha se o layout dos cards sincroniza com o servidor ou fica local',
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
          message: 'Os dados cadastrados não são apagados, mas as telas e widgets desta área serão ocultados.',
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
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(err.toString())));
        return;
      }
    }

    try {
      await ref.read(areasProvider.notifier).setEnabled(item.area, nextEnabled);
    } catch (err) {
      if (!context.mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(err.toString())));
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final areasState = ref.watch(areasProvider);
    final auth = ref.watch(authProvider);

    return SectionCard(
      title: 'Áreas do sistema',
      subtitle: 'Ative ou desative módulos opcionais conforme o perfil da sua infraestrutura',
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
      trailing: auth.isAdmin
          ? FilledButton.tonalIcon(
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
            )
          : null,
      child: st == null
          ? const Text('Carregando estado da configuração inicial...')
          : Wrap(
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
    );
  }
}

// 5. ServerAddressesCard + Dialog
class ServerAddressesCard extends ConsumerStatefulWidget {
  const ServerAddressesCard({super.key});

  @override
  ConsumerState<ServerAddressesCard> createState() =>
      _ServerAddressesCardState();
}

class _ServerAddressesCardState extends ConsumerState<ServerAddressesCard> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        ref.read(coreSettingsProvider.notifier).fetchServerAddresses();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(coreSettingsProvider);
    final auth = ref.watch(authProvider);

    return SectionCard(
      title: 'Endereços deste servidor',
      subtitle: 'Usados nos comandos de syslog, agentes remotos e túnel VPN',
      icon: Icons.dns_outlined,
      trailing: auth.isAdmin
          ? OutlinedButton.icon(
              onPressed: () => showServerAddressesSheet(context, ref),
              icon: const Icon(Icons.edit_outlined, size: 18),
              label: const Text('Editar'),
            )
          : null,
      child: settings.addressesLoading && settings.addresses.isEmpty
          ? const LoadingStateView(message: 'Detectando endereços...')
          : Column(
              children: <Widget>[
                for (final entry in settings.addresses) ...<Widget>[
                  KeyValueRow(
                    label: entry.label,
                    value: entry.value ?? 'Não detectado',
                    monospace: true,
                    copyable: entry.value != null,
                    trailing: entry.id == settings.preferredAddressId
                        ? const SemanticChip(
                            label: 'Padrão',
                            tone: SemanticTone.primary,
                            compact: true,
                          )
                        : null,
                  ),
                ],
              ],
            ),
    );
  }
}

Future<void> showServerAddressesSheet(
  BuildContext context,
  WidgetRef ref,
) async {
  await ref
      .read(coreSettingsProvider.notifier)
      .fetchServerAddresses(force: true);
  if (!context.mounted) return;

  final current = ref.read(coreSettingsProvider);
  final builtins = current.addresses.where((e) => e.kind != 'custom').toList();
  final customEntries = current.addresses
      .where((e) => e.kind == 'custom')
      .map(
        (e) => <String, String>{
          'id': e.id,
          'label': e.label,
          'value': e.value ?? '',
        },
      )
      .toList();

  final overrideCtrls = <String, TextEditingController>{
    for (final e in builtins)
      e.id: TextEditingController(text: e.overridden ? (e.value ?? '') : ''),
  };
  String? preferredId = current.preferredAddressId;
  bool domainHttps = current.domainHttps;
  var saving = false;

  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Endereços deste servidor',
    subtitle: 'Corrija endereços detectados ou adicione domínios/FQDNs personalizados.',
    maxWidth: 640,
    builder: (sheetCtx) {
      return StatefulBuilder(
        builder: (context, setModalState) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              for (final entry in builtins) ...<Widget>[
                NestedPanel(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Expanded(
                            child: Text(
                              entry.label,
                              style: const TextStyle(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                          ),
                          if (overrideCtrls[entry.id]!.text.isNotEmpty)
                            TextButton(
                              onPressed: () => setModalState(
                                () => overrideCtrls[entry.id]!.clear(),
                              ),
                              child: const Text('Voltar ao detectado'),
                            ),
                        ],
                      ),
                      Text(
                        '${entry.description} · Detectado: ${entry.detected ?? 'nenhum'} (${entry.source})',
                        style: context.textTheme.bodySmall?.copyWith(
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                      const SizedBox(height: 8),
                      TextField(
                        controller: overrideCtrls[entry.id],
                        style: const TextStyle(fontFamily: 'monospace'),
                        decoration: InputDecoration(
                          labelText: 'Sobrescrever ${entry.label} (opcional)',
                          hintText: entry.detected ?? 'IP ou domínio',
                          isDense: true,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 10),
              ],
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: <Widget>[
                  Text(
                    'Endereços personalizados',
                    style: context.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  TextButton.icon(
                    onPressed: () => setModalState(() {
                      customEntries.add(<String, String>{
                        'id': 'custom:${DateTime.now().millisecondsSinceEpoch}',
                        'label': 'Domínio',
                        'value': '',
                      });
                    }),
                    icon: const Icon(Icons.add, size: 18),
                    label: const Text('Adicionar'),
                  ),
                ],
              ),
              for (var i = 0; i < customEntries.length; i++) ...<Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      flex: 2,
                      child: TextFormField(
                        initialValue: customEntries[i]['label'],
                        decoration: const InputDecoration(
                          labelText: 'Rótulo',
                          isDense: true,
                        ),
                        onChanged: (v) => customEntries[i]['label'] = v,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      flex: 3,
                      child: TextFormField(
                        initialValue: customEntries[i]['value'],
                        decoration: const InputDecoration(
                          labelText: 'Endereço',
                          isDense: true,
                        ),
                        onChanged: (v) => customEntries[i]['value'] = v,
                      ),
                    ),
                    IconButton(
                      icon: Icon(
                        Icons.delete_outline,
                        color: context.colorScheme.error,
                      ),
                      onPressed: () =>
                          setModalState(() => customEntries.removeAt(i)),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
              ],
              const SizedBox(height: 8),
              DropdownButtonFormField<String?>(
                initialValue: current.addresses.any((e) => e.id == preferredId)
                    ? preferredId
                    : null,
                decoration: const InputDecoration(
                  labelText: 'Endereço padrão preferido',
                ),
                items: <DropdownMenuItem<String?>>[
                  const DropdownMenuItem<String?>(
                    value: null,
                    child: Text('Automático'),
                  ),
                  for (final e in current.addresses)
                    DropdownMenuItem<String?>(
                      value: e.id,
                      child: Text(e.label),
                    ),
                ],
                onChanged: (v) => setModalState(() => preferredId = v),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                value: domainHttps,
                onChanged: (v) => setModalState(() => domainHttps = v),
                title: const Text('Domínio atrás de proxy HTTPS'),
                subtitle: const Text(
                  'Agentes remotos passam a usar https:// automaticamente.',
                ),
              ),
              const SizedBox(height: 14),
              FilledButton.icon(
                onPressed: saving
                    ? null
                    : () async {
                        setModalState(() => saving = true);
                        final overrides = <String, String>{};
                        overrideCtrls.forEach((k, ctrl) {
                          if (ctrl.text.trim().isNotEmpty) {
                            overrides[k] = ctrl.text.trim();
                          }
                        });
                        final ok = await ref
                            .read(coreSettingsProvider.notifier)
                            .saveServerAddresses(
                              overrides: overrides,
                              custom: customEntries
                                  .where(
                                    (c) => (c['value'] ?? '').trim().isNotEmpty,
                                  )
                                  .toList(growable: false),
                              preferredId: preferredId,
                              domainHttps: domainHttps,
                            );
                        if (sheetCtx.mounted && ok) {
                          Navigator.of(sheetCtx).pop();
                        }
                      },
                icon: const Icon(Icons.save_outlined),
                label: const Text('Salvar endereços'),
              ),
            ],
          );
        },
      );
    },
  );

  for (final ctrl in overrideCtrls.values) {
    ctrl.dispose();
  }
}

// 6. NotificationsCard (Web Push + preferências locais)
class NotificationsCard extends ConsumerStatefulWidget {
  const NotificationsCard({super.key});

  @override
  ConsumerState<NotificationsCard> createState() => _NotificationsCardState();
}

class _NotificationsCardState extends ConsumerState<NotificationsCard> {
  bool _testing = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(coreSettingsProvider.notifier).fetchPushStatus();
    });
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(coreSettingsProvider);
    final push = settings.pushStatus;

    return SectionCard(
      title: 'Notificações Push e Alertas Locais',
      subtitle: 'Receba alertas críticos, quedas de monitor e falhas de backup em tempo real',
      icon: Icons.notifications_active_outlined,
      iconTone: push?.configured == true
          ? SemanticTone.success
          : SemanticTone.info,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: settings.localNotificationsEnabled,
            onChanged: (v) => ref
                .read(coreSettingsProvider.notifier)
                .setLocalNotificationsEnabled(v),
            title: const Text('Ativar notificações neste dispositivo'),
            subtitle: const Text(
              'Dispara avisos para alert:triggered, monitor down, device offline e backup:failed.',
            ),
          ),
          const Divider(height: 16),
          KeyValueRow(
            label: 'Servidor Web Push (VAPID)',
            value: push == null
                ? 'Consultando...'
                : (push.configured ? 'Configurado' : 'Não configurado'),
          ),
          KeyValueRow(
            label: 'Assinaturas ativas',
            value: formatInteger(push?.subscriptionCount ?? 0),
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerLeft,
            child: FilledButton.tonalIcon(
              onPressed: _testing
                  ? null
                  : () async {
                      setState(() => _testing = true);
                      final msg = await ref
                          .read(coreSettingsProvider.notifier)
                          .sendTestPush();
                      if (!context.mounted) return;
                      setState(() => _testing = false);
                      ScaffoldMessenger.of(context)
                          .showSnackBar(SnackBar(content: Text(msg)));
                    },
              icon: _testing
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.send_outlined, size: 18),
              label: const Text('Enviar notificação de teste'),
            ),
          ),
        ],
      ),
    );
  }
}

// 7. BackupShortcutCard
class BackupShortcutCard extends ConsumerStatefulWidget {
  const BackupShortcutCard({super.key});

  @override
  ConsumerState<BackupShortcutCard> createState() => _BackupShortcutCardState();
}

class _BackupShortcutCardState extends ConsumerState<BackupShortcutCard> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && ref.read(authProvider).isAdmin) {
        ref.read(backupProvider.notifier).fetchPlan();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final plan = ref.watch(backupProvider).plan;
    if (!auth.isAdmin) return const SizedBox.shrink();

    return SectionCard(
      title: 'Backup e Restauração',
      subtitle: plan?.backupEnabled == true
          ? 'Backup automático ativo (${plan!.storageDestinationName ?? 'destino'})'
          : 'Gerencie cópias automáticas, bancos de dados e destinos de armazenamento',
      icon: Icons.settings_backup_restore,
      trailing: FilledButton.tonalIcon(
        onPressed: () => context.go('/backup'),
        icon: const Icon(Icons.open_in_new, size: 18),
        label: const Text('Abrir Backup'),
      ),
      child: KeyValueRow(
        label: 'Último backup do sistema',
        value: plan?.lastBackupAt != null
            ? formatDateTime(plan!.lastBackupAt)
            : 'Nenhum registro',
      ),
    );
  }
}

// 8. CoreCatalogsCard (Perfis SNMP, Sistemas Operacionais & Fabricantes OUI)
class CoreCatalogsCard extends ConsumerStatefulWidget {
  const CoreCatalogsCard({super.key});

  @override
  ConsumerState<CoreCatalogsCard> createState() => _CoreCatalogsCardState();
}

class _CoreCatalogsCardState extends ConsumerState<CoreCatalogsCard> {
  final TextEditingController _macLookupCtrl = TextEditingController();
  MacVendorLookup? _macResult;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final n = ref.read(coreSettingsProvider.notifier);
      n.fetchSnmpProfiles();
      n.fetchOperatingSystems();
      n.fetchVendorStatus();
    });
  }

  @override
  void dispose() {
    _macLookupCtrl.dispose();
    super.dispose();
  }

  Future<void> _openCreateSnmpProfile() async {
    final idCtrl = TextEditingController();
    final nameCtrl = TextEditingController();
    final vendorCtrl = TextEditingController();
    final catCtrl = TextEditingController(text: 'router');
    final oidCtrl = TextEditingController();
    final sensorKeyCtrl = TextEditingController(text: 'temp');
    final sensorLabelCtrl = TextEditingController(text: 'Temperatura');
    final sensorOidCtrl = TextEditingController();

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Novo perfil SNMP',
      subtitle:
          'Cadastre OIDs personalizados para leitura automática de sensores.',
      builder: (sheetCtx) {
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            TextField(
              controller: idCtrl,
              decoration: const InputDecoration(labelText: 'ID único'),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: nameCtrl,
              decoration: const InputDecoration(labelText: 'Nome'),
            ),
            const SizedBox(height: 10),
            Row(
              children: <Widget>[
                Expanded(
                  child: TextField(
                    controller: vendorCtrl,
                    decoration: const InputDecoration(labelText: 'Fabricante'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    controller: catCtrl,
                    decoration: const InputDecoration(labelText: 'Categoria'),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 10),
            TextField(
              controller: oidCtrl,
              decoration: const InputDecoration(
                labelText: 'Prefixo sysObjectID (opcional)',
              ),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: sensorLabelCtrl,
              decoration: const InputDecoration(
                labelText: 'Sensor inicial — Rótulo',
              ),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: sensorOidCtrl,
              style: const TextStyle(fontFamily: 'monospace'),
              decoration: const InputDecoration(
                labelText: 'Sensor inicial — OID',
              ),
            ),
            const SizedBox(height: 14),
            FilledButton(
              onPressed: () async {
                if (idCtrl.text.trim().isEmpty ||
                    nameCtrl.text.trim().isEmpty) {
                  return;
                }
                final profile = SnmpDeviceProfile(
                  id: idCtrl.text.trim(),
                  name: nameCtrl.text.trim(),
                  vendor: vendorCtrl.text.trim(),
                  category: catCtrl.text.trim(),
                  sysObjectIdPrefix: oidCtrl.text.trim().isEmpty
                      ? null
                      : oidCtrl.text.trim(),
                  sensors: sensorOidCtrl.text.trim().isEmpty
                      ? const <SnmpProfileSensor>[]
                      : <SnmpProfileSensor>[
                          SnmpProfileSensor(
                            key: sensorKeyCtrl.text.trim(),
                            label: sensorLabelCtrl.text.trim(),
                            oid: sensorOidCtrl.text.trim(),
                          ),
                        ],
                );
                final ok = await ref
                    .read(coreSettingsProvider.notifier)
                    .saveSnmpProfile(profile);
                if (sheetCtx.mounted && ok) Navigator.of(sheetCtx).pop();
              },
              child: const Text('Salvar perfil SNMP'),
            ),
          ],
        );
      },
    );

    idCtrl.dispose();
    nameCtrl.dispose();
    vendorCtrl.dispose();
    catCtrl.dispose();
    oidCtrl.dispose();
    sensorKeyCtrl.dispose();
    sensorLabelCtrl.dispose();
    sensorOidCtrl.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(coreSettingsProvider);
    final auth = ref.watch(authProvider);
    final vStatus = settings.vendorStatus;

    return SectionCard(
      title: 'Catálogos do Núcleo (SNMP, Sistemas e Fabricantes OUI)',
      subtitle: 'Perfis SNMP reutilizáveis, sistemas operacionais suportados e consulta MAC IEEE',
      icon: Icons.library_books_outlined,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: <Widget>[
              Text(
                'Perfis SNMP (${settings.snmpProfiles.length})',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (auth.canWrite)
                TextButton.icon(
                  onPressed: _openCreateSnmpProfile,
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Novo perfil'),
                ),
            ],
          ),
          for (final p in settings.snmpProfiles.take(6))
            ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(
                p.name,
                style: const TextStyle(fontWeight: FontWeight.w600),
              ),
              subtitle: Text('${p.vendor} · ${p.sensors.length} sensor(es)'),
              trailing: !p.isBuiltin && auth.canWrite
                  ? IconButton(
                      icon: Icon(
                        Icons.delete_outline,
                        color: context.colorScheme.error,
                      ),
                      onPressed: () => ref
                          .read(coreSettingsProvider.notifier)
                          .deleteSnmpProfile(p.id),
                    )
                  : const SemanticChip(
                      label: 'Nativo',
                      tone: SemanticTone.secondary,
                      compact: true,
                    ),
            ),
          const Divider(height: 20),
          Text(
            'Sistemas operacionais cadastrados (${settings.operatingSystems.length})',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: <Widget>[
              for (final os in settings.operatingSystems)
                SemanticChip(
                  label: os.label,
                  tone: os.supportsSyslog
                      ? SemanticTone.info
                      : SemanticTone.secondary,
                  compact: true,
                ),
            ],
          ),
          const Divider(height: 20),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: <Widget>[
              Text(
                'Base de Fabricantes MAC (IEEE OUI)',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (auth.isAdmin)
                OutlinedButton.icon(
                  onPressed: settings.vendorsRefreshing
                      ? null
                      : () => ref
                            .read(coreSettingsProvider.notifier)
                            .refreshVendors(),
                  icon: settings.vendorsRefreshing
                      ? const SizedBox(
                          width: 14,
                          height: 14,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.refresh, size: 16),
                  label: const Text('Atualizar OUI'),
                ),
            ],
          ),
          if (vStatus != null) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              '${formatInteger(vStatus.entries)} registros · Atualizado: ${formatDateTime(vStatus.updatedAt)}',
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
          const SizedBox(height: 10),
          Row(
            children: <Widget>[
              Expanded(
                child: TextField(
                  controller: _macLookupCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Consultar fabricante por MAC',
                    hintText: 'AA:BB:CC:11:22:33',
                    isDense: true,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              FilledButton.tonal(
                onPressed: () async {
                  final res = await ref
                      .read(coreSettingsProvider.notifier)
                      .lookupMacVendor(_macLookupCtrl.text);
                  if (mounted) setState(() => _macResult = res);
                },
                child: const Text('Consultar'),
              ),
            ],
          ),
          if (_macResult != null) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(
              tone: SemanticTone.info,
              child: Text(
                _macResult!.vendor != null
                    ? '${_macResult!.vendor} (${_macResult!.organization ?? _macResult!.source})'
                    : (_macResult!.locallyAdministered
                          ? 'MAC administrado localmente / aleatório'
                          : 'Fabricante não encontrado'),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/dashboard_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Dashboard principal customizável (paridade 100% com `DashboardPage.vue`).
class DashboardPage extends ConsumerStatefulWidget {
  const DashboardPage({super.key});

  @override
  ConsumerState<DashboardPage> createState() => _DashboardPageState();
}

class _DashboardPageState extends ConsumerState<DashboardPage> {
  VoidCallback? _unsubLayout;
  bool _promptOpen = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _bootstrap();
    });
    _unsubLayout = ref.read(sseServiceProvider.notifier).onEvent(
      <String>['dashboard:layout_updated'],
      (data, _) {
        if (mounted) {
          ref.read(dashboardProvider.notifier).applyRemoteLayout(data);
        }
      },
    );
  }

  @override
  void dispose() {
    _unsubLayout?.call();
    super.dispose();
  }

  Future<void> _bootstrap() async {
    await Future.wait<void>(<Future<void>>[
      ref.read(dashboardProvider.notifier).initialize(),
      ref.read(monitorsProvider.notifier).fetchMonitors(),
      ref.read(monitorsProvider.notifier).fetchReferenceCatalogs(),
      ref.read(alertsProvider.notifier).fetchActiveAlerts(),
    ]);
  }

  Future<void> _refresh() async {
    await Future.wait<void>(<Future<void>>[
      ref.read(dashboardProvider.notifier).reload(),
      ref.read(monitorsProvider.notifier).fetchMonitors(),
      ref.read(alertsProvider.notifier).fetchActiveAlerts(),
    ]);
  }

  void _maybeShowServerPrompt(DashboardState dash) {
    if (!dash.showServerPrompt || _promptOpen) return;
    _promptOpen = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      await showAdaptiveFormSheet<void>(
        context: context,
        title: 'Layout encontrado no servidor',
        subtitle:
            'Existe um layout salvo no servidor. Escolha como deseja sincronizar o Dashboard.',
        isDismissible: false,
        builder: (sheetCtx) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              ChoiceCard(
                title: 'Usar layout do servidor (Recomendado)',
                subtitle:
                    'Mantém todos os seus dispositivos sincronizados automaticamente.',
                icon: Icons.cloud_sync_outlined,
                tone: SemanticTone.primary,
                onTap: () async {
                  await ref
                      .read(dashboardProvider.notifier)
                      .chooseInitialSyncMode(DashboardSyncMode.server);
                  if (sheetCtx.mounted) Navigator.of(sheetCtx).pop();
                },
              ),
              const SizedBox(height: 10),
              ChoiceCard(
                title: 'Enviar layout local para o servidor',
                subtitle:
                    'Substitui o layout do servidor pela configuração deste aparelho.',
                icon: Icons.cloud_upload_outlined,
                tone: SemanticTone.info,
                onTap: () async {
                  await ref
                      .read(dashboardProvider.notifier)
                      .saveLayoutGlobally();
                  await ref
                      .read(dashboardProvider.notifier)
                      .chooseInitialSyncMode(DashboardSyncMode.server);
                  if (sheetCtx.mounted) Navigator.of(sheetCtx).pop();
                },
              ),
              const SizedBox(height: 10),
              ChoiceCard(
                title: 'Manter somente neste dispositivo',
                subtitle:
                    'Desativa a sincronização automática neste navegador/aparelho.',
                icon: Icons.devices_outlined,
                tone: SemanticTone.secondary,
                onTap: () async {
                  await ref
                      .read(dashboardProvider.notifier)
                      .chooseInitialSyncMode(DashboardSyncMode.local);
                  if (sheetCtx.mounted) Navigator.of(sheetCtx).pop();
                },
              ),
            ],
          );
        },
      );
      _promptOpen = false;
    });
  }

  Future<void> _openAddWidgetSheet() async {
    final dash = ref.read(dashboardProvider);
    final visibleAreas = ref.read(visibleAreasProvider);
    final catalog = dash.catalog;
    if (catalog == null) return;

    final hiddenDefaults = dash.hiddenWidgets(visibleAreas);
    final multiTemplates = catalog.widgets
        .where((e) => e.allowMultiple && visibleAreas.contains(e.area))
        .toList(growable: false);

    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Adicionar widget ao Dashboard',
      subtitle:
          'Reative cards ocultos ou adicione widgets configuráveis do catálogo.',
      maxWidth: 640,
      builder: (sheetCtx) {
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            if (hiddenDefaults.isNotEmpty) ...<Widget>[
              Text(
                'Cards ocultos',
                style: Theme.of(
                  sheetCtx,
                ).textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 8),
              for (final w in hiddenDefaults) ...<Widget>[
                ChoiceCard(
                  title: w.title,
                  subtitle: w.description,
                  icon: Icons.visibility_outlined,
                  tone: SemanticTone.primary,
                  onTap: () async {
                    await ref
                        .read(dashboardProvider.notifier)
                        .toggleWidgetVisibility(w.id, true);
                    if (sheetCtx.mounted) Navigator.of(sheetCtx).pop();
                  },
                ),
                const SizedBox(height: 8),
              ],
              const SizedBox(height: 12),
            ],
            if (multiTemplates.isNotEmpty) ...<Widget>[
              Text(
                'Widgets configuráveis',
                style: Theme.of(
                  sheetCtx,
                ).textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 8),
              for (final entry in multiTemplates) ...<Widget>[
                ChoiceCard(
                  title: entry.title,
                  subtitle: entry.description,
                  icon: Icons.add_chart,
                  tone: SemanticTone.info,
                  badge: entry.area.defaultTitle,
                  badgeTone: SemanticTone.secondary,
                  onTap: () async {
                    Navigator.of(sheetCtx).pop();
                    await _openWidgetConfigSheet(entry: entry);
                  },
                ),
                const SizedBox(height: 8),
              ],
            ],
            if (hiddenDefaults.isEmpty && multiTemplates.isEmpty)
              const EmptyStateView(
                title: 'Todos os widgets já estão visíveis',
                description:
                    'Não há widgets ocultos ou modelos disponíveis para adicionar.',
                icon: Icons.check_circle_outline,
                tone: SemanticTone.success,
              ),
          ],
        );
      },
    );
  }

  Future<void> _openWidgetConfigSheet({
    WidgetCatalogEntry? entry,
    DashboardWidgetConfig? existing,
  }) async {
    final keys =
        entry?.configKeys ??
        ref
            .read(dashboardProvider)
            .catalog
            ?.findByType(existing?.type)
            ?.configKeys ??
        const <String>[];

    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing != null
          ? 'Configurar widget'
          : 'Adicionar ${entry?.title ?? 'widget'}',
      builder: (_) => _DashboardWidgetConfigSheetBody(
        entry: entry,
        existing: existing,
        configKeys: keys,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final dash = ref.watch(dashboardProvider);
    final visibleAreas = ref.watch(visibleAreasProvider);
    final areasState = ref.watch(areasProvider);
    final auth = ref.watch(authProvider);

    _maybeShowServerPrompt(dash);

    final visibleWidgets = dash.visibleWidgets(visibleAreas);
    final suggestedAreas = areasState.states
        .where(
          (a) =>
              a.optional &&
              !a.enabled &&
              a.evidence?.present == true &&
              !dash.dismissedAreaSuggestions.contains(a.area),
        )
        .toList(growable: false);

    return LayoutBuilder(
      builder: (context, pageConstraints) {
        final isCompact = pageConstraints.maxWidth < 680;
        final scheme = context.colorScheme;

        return RefreshIndicator(
          onRefresh: _refresh,
          child: ListView(
            children: <Widget>[
              if (isCompact) ...<Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        'Dashboard',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.headlineSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                          color: scheme.onSurface,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    FilledButton.tonalIcon(
                      style: FilledButton.styleFrom(
                        visualDensity: VisualDensity.compact,
                        padding: const EdgeInsets.symmetric(
                          horizontal: 12,
                          vertical: 8,
                        ),
                      ),
                      onPressed: () =>
                          ref.read(dashboardProvider.notifier).toggleEditMode(),
                      icon: Icon(
                        dash.isEditMode
                            ? Icons.check
                            : Icons.dashboard_customize_outlined,
                        size: 16,
                      ),
                      label: Text(
                        dash.isEditMode ? 'Concluir' : 'Personalizar',
                      ),
                    ),
                  ],
                ),
                if (dash.isEditMode) ...<Widget>[
                  const SizedBox(height: 8),
                  SymmetricalActionBar(
                    spacing: 8,
                    runSpacing: 8,
                    children: <Widget>[
                      OutlinedButton.icon(
                        onPressed: _openAddWidgetSheet,
                        icon: const Icon(Icons.add, size: 18),
                        label: const Text('Adicionar widget'),
                      ),
                      OutlinedButton.icon(
                        onPressed: () async {
                          final ok = await showConfirmActionDialog(
                            context: context,
                            title: 'Restaurar layout padrão?',
                            message:
                                'Os cards personalizados serão substituídos pelo layout inicial.',
                            confirmLabel: 'Restaurar',
                            tone: SemanticTone.warning,
                          );
                          if (ok) {
                            await ref
                                .read(dashboardProvider.notifier)
                                .resetToDefaultLayout();
                          }
                        },
                        icon: const Icon(Icons.restart_alt, size: 18),
                        label: const Text('Restaurar padrão'),
                      ),
                    ],
                  ),
                ],
              ] else
                SearchFilterHeader(
                  title: 'Dashboard',
                  subtitle: dash.syncMode == DashboardSyncMode.server
                      ? 'Sincronizado com o servidor em tempo real'
                      : 'Layout local neste dispositivo',
                  actions: <Widget>[
                    if (dash.isEditMode) ...<Widget>[
                      OutlinedButton.icon(
                        onPressed: _openAddWidgetSheet,
                        icon: const Icon(Icons.add, size: 18),
                        label: const Text('Adicionar widget'),
                      ),
                      TextButton.icon(
                        onPressed: () async {
                          final ok = await showConfirmActionDialog(
                            context: context,
                            title: 'Restaurar layout padrão?',
                            message:
                                'Os cards personalizados serão substituídos pelo layout inicial.',
                            confirmLabel: 'Restaurar',
                            tone: SemanticTone.warning,
                          );
                          if (ok) {
                            await ref
                                .read(dashboardProvider.notifier)
                                .resetToDefaultLayout();
                          }
                        },
                        icon: const Icon(Icons.restart_alt, size: 18),
                        label: const Text('Restaurar padrão'),
                      ),
                    ],
                    FilledButton.tonalIcon(
                      onPressed: () =>
                          ref.read(dashboardProvider.notifier).toggleEditMode(),
                      icon: Icon(
                        dash.isEditMode
                            ? Icons.check
                            : Icons.dashboard_customize_outlined,
                        size: 18,
                      ),
                      label: Text(
                        dash.isEditMode ? 'Concluir' : 'Personalizar',
                      ),
                    ),
                  ],
                ),
              const SizedBox(height: 12),
              if (auth.isAdmin && suggestedAreas.isNotEmpty) ...<Widget>[
                for (final suggestion in suggestedAreas) ...<Widget>[
                  NestedPanel(
                    tone: SemanticTone.info,
                    child: Wrap(
                      spacing: 12,
                      runSpacing: 8,
                      alignment: WrapAlignment.spaceBetween,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            Icon(suggestion.area.defaultIcon),
                            const SizedBox(width: 10),
                            Flexible(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: <Widget>[
                                  Text(
                                    'Sugestão: ligar área ${suggestion.title}',
                                    style: const TextStyle(
                                      fontWeight: FontWeight.w700,
                                    ),
                                  ),
                                  if (suggestion.evidence?.reason != null)
                                    Text(
                                      suggestion.evidence!.reason!,
                                      style: context.textTheme.bodySmall,
                                    ),
                                ],
                              ),
                            ),
                          ],
                        ),
                        Wrap(
                          spacing: 8,
                          children: <Widget>[
                            FilledButton(
                              onPressed: () => ref
                                  .read(areasProvider.notifier)
                                  .setEnabled(suggestion.area, true),
                              child: const Text('Ligar área'),
                            ),
                            TextButton(
                              onPressed: () => ref
                                  .read(dashboardProvider.notifier)
                                  .dismissAreaSuggestion(suggestion.area),
                              child: const Text('Agora não'),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
              ],
              if (!dash.ready)
                const LoadingStateView(
                  message: 'Carregando catálogo do Dashboard...',
                )
              else if (visibleWidgets.isEmpty)
                EmptyStateView(
                  title: 'Nenhum widget visível',
                  description:
                      'Ative widgets em Personalizar para montar seu painel.',
                  icon: Icons.dashboard_outlined,
                  action: FilledButton.icon(
                    onPressed: _openAddWidgetSheet,
                    icon: const Icon(Icons.add),
                    label: const Text('Adicionar widget'),
                  ),
                )
              else
                LayoutBuilder(
                  builder: (context, constraints) {
                    final width = constraints.maxWidth;
                    const spacing = 12.0;

                    double spanWidth(DashboardWidgetConfig w) {
                      if (width < 700) return width;
                      final cols = width >= 1100 ? w.lg : w.md;
                      final fraction = (cols.clamp(4, 12)) / 12.0;
                      if (fraction >= 0.99) return width;
                      final itemsPerRow = (1 / fraction).round();
                      return (width - spacing * (itemsPerRow - 1)) /
                          itemsPerRow;
                    }

                    return Wrap(
                      spacing: spacing,
                      runSpacing: spacing,
                      children: <Widget>[
                        for (var i = 0; i < visibleWidgets.length; i++)
                          SizedBox(
                            width: spanWidth(visibleWidgets[i]),
                            child: _DashboardWidgetFrame(
                              widgetConfig: visibleWidgets[i],
                              editMode: dash.isEditMode,
                              canMoveUp: i > 0,
                              canMoveDown: i < visibleWidgets.length - 1,
                              hasConfig:
                                  (dash.catalog
                                      ?.findByType(visibleWidgets[i].type)
                                      ?.configKeys
                                      .isNotEmpty ??
                                  false),
                              onMoveUp: () => ref
                                  .read(dashboardProvider.notifier)
                                  .moveWidget(visibleWidgets[i].id, up: true),
                              onMoveDown: () => ref
                                  .read(dashboardProvider.notifier)
                                  .moveWidget(visibleWidgets[i].id, up: false),
                              onResize: (size) => ref
                                  .read(dashboardProvider.notifier)
                                  .resizeWidget(visibleWidgets[i].id, size),
                              onConfigure: () => _openWidgetConfigSheet(
                                existing: visibleWidgets[i],
                              ),
                              onRemove: () => ref
                                  .read(dashboardProvider.notifier)
                                  .removeWidget(visibleWidgets[i].id),
                            ),
                          ),
                      ],
                    );
                  },
                ),
              const SizedBox(height: 24),
            ],
          ),
        );
      },
    );
  }
}

class _DashboardWidgetFrame extends StatelessWidget {
  const _DashboardWidgetFrame({
    required this.widgetConfig,
    required this.editMode,
    required this.canMoveUp,
    required this.canMoveDown,
    required this.hasConfig,
    required this.onMoveUp,
    required this.onMoveDown,
    required this.onResize,
    required this.onConfigure,
    required this.onRemove,
  });

  final DashboardWidgetConfig widgetConfig;
  final bool editMode;
  final bool canMoveUp;
  final bool canMoveDown;
  final bool hasConfig;
  final VoidCallback onMoveUp;
  final VoidCallback onMoveDown;
  final ValueChanged<WidgetSize> onResize;
  final VoidCallback onConfigure;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final contribution = DashboardWidgetRegistry.resolve(widgetConfig.type);
    final child = contribution != null
        ? contribution.builder(context, widgetConfig.config)
        : SectionCard(
            title: widgetConfig.title,
            subtitle: widgetConfig.description,
            icon: Icons.widgets_outlined,
            child: Text(
              'Widget "${widgetConfig.type}" registrado pela área correspondente.',
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colorScheme.onSurfaceVariant,
              ),
            ),
          );

    if (!editMode) return child;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          tone: SemanticTone.primary,
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
          child: Wrap(
            spacing: 4,
            runSpacing: 4,
            crossAxisAlignment: WrapCrossAlignment.center,
            alignment: WrapAlignment.spaceBetween,
            children: <Widget>[
              Text(
                widgetConfig.title,
                style: context.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  IconButton(
                    tooltip: 'Mover para cima',
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.arrow_upward, size: 18),
                    onPressed: canMoveUp ? onMoveUp : null,
                  ),
                  IconButton(
                    tooltip: 'Mover para baixo',
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.arrow_downward, size: 18),
                    onPressed: canMoveDown ? onMoveDown : null,
                  ),
                  PopupMenuButton<WidgetSize>(
                    tooltip: 'Tamanho do card',
                    icon: const Icon(Icons.aspect_ratio, size: 18),
                    onSelected: onResize,
                    itemBuilder: (_) => <PopupMenuEntry<WidgetSize>>[
                      for (final size in WidgetSize.values)
                        PopupMenuItem<WidgetSize>(
                          value: size,
                          child: Text(size.label),
                        ),
                    ],
                  ),
                  if (hasConfig)
                    IconButton(
                      tooltip: 'Configurar widget',
                      visualDensity: VisualDensity.compact,
                      icon: const Icon(Icons.settings_outlined, size: 18),
                      onPressed: onConfigure,
                    ),
                  IconButton(
                    tooltip: 'Ocultar / remover widget',
                    visualDensity: VisualDensity.compact,
                    icon: Icon(
                      Icons.close,
                      size: 18,
                      color: context.colorScheme.error,
                    ),
                    onPressed: onRemove,
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 6),
        child,
      ],
    );
  }
}

class _DashboardWidgetConfigSheetBody extends ConsumerStatefulWidget {
  const _DashboardWidgetConfigSheetBody({
    required this.entry,
    required this.existing,
    required this.configKeys,
  });

  final WidgetCatalogEntry? entry;
  final DashboardWidgetConfig? existing;
  final List<String> configKeys;

  @override
  ConsumerState<_DashboardWidgetConfigSheetBody> createState() =>
      _DashboardWidgetConfigSheetBodyState();
}

class _DashboardWidgetConfigSheetBodyState
    extends ConsumerState<_DashboardWidgetConfigSheetBody> {
  late final TextEditingController _titleCtrl;
  late final TextEditingController _warnCtrl;
  late final TextEditingController _critCtrl;
  int? _selectedDeviceId;
  int? _selectedMonitorId;
  String _timeframe = '1h';
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final existing = widget.existing;
    final entry = widget.entry;
    _titleCtrl = TextEditingController(
      text: existing?.title ?? entry?.title ?? '',
    );
    _selectedDeviceId = parseIntOrNull(existing?.config['deviceId']);
    _selectedMonitorId = parseIntOrNull(existing?.config['monitorId']);
    _timeframe = (existing?.config['timeframe'] ?? '1h').toString();
    _warnCtrl = TextEditingController(
      text: (existing?.config['warningThreshold'] ?? 75).toString(),
    );
    _critCtrl = TextEditingController(
      text: (existing?.config['criticalThreshold'] ?? 90).toString(),
    );
  }

  @override
  void dispose() {
    _titleCtrl.dispose();
    _warnCtrl.dispose();
    _critCtrl.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    setState(() => _saving = true);
    final keys = widget.configKeys;
    final cfg = <String, dynamic>{
      'deviceId': ?_selectedDeviceId,
      'monitorId': ?_selectedMonitorId,
      if (keys.contains('timeframe')) 'timeframe': _timeframe,
      if (keys.contains('warningThreshold'))
        'warningThreshold': parseDoubleOrNull(_warnCtrl.text) ?? 75,
      if (keys.contains('criticalThreshold'))
        'criticalThreshold': parseDoubleOrNull(_critCtrl.text) ?? 90,
    };
    final existing = widget.existing;
    final entry = widget.entry;
    if (existing != null) {
      await ref
          .read(dashboardProvider.notifier)
          .updateWidgetConfig(existing.id, title: _titleCtrl.text, config: cfg);
    } else if (entry != null) {
      await ref
          .read(dashboardProvider.notifier)
          .addCustomWidget(entry.type, title: _titleCtrl.text, config: cfg);
    }
    if (mounted) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final monitorsState = ref.watch(monitorsProvider);
    final keys = widget.configKeys;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _titleCtrl,
          decoration: const InputDecoration(labelText: 'Título do card'),
        ),
        if (keys.contains('deviceId')) ...<Widget>[
          const SizedBox(height: 12),
          AppDropdownFormField<int?>(
            initialValue: _selectedDeviceId,
            decoration: const InputDecoration(labelText: 'Dispositivo'),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text('Selecione...', overflow: TextOverflow.ellipsis),
              ),
              for (final d in monitorsState.devices)
                DropdownMenuItem<int?>(
                  value: d.id,
                  child: Text(d.name, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) => setState(() => _selectedDeviceId = v),
          ),
        ],
        if (keys.contains('monitorId')) ...<Widget>[
          const SizedBox(height: 12),
          AppDropdownFormField<int?>(
            initialValue: _selectedMonitorId,
            decoration: const InputDecoration(labelText: 'Monitor'),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text(
                  'Todos / Selecione',
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              for (final m in monitorsState.monitors)
                DropdownMenuItem<int?>(
                  value: m.id,
                  child: Text(m.name, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) => setState(() => _selectedMonitorId = v),
          ),
        ],
        if (keys.contains('timeframe')) ...<Widget>[
          const SizedBox(height: 12),
          AppDropdownFormField<String>(
            initialValue: _timeframe,
            decoration: const InputDecoration(labelText: 'Janela padrão'),
            items: const <DropdownMenuItem<String>>[
              DropdownMenuItem<String>(
                value: '5m',
                child: Text('5 minutos', overflow: TextOverflow.ellipsis),
              ),
              DropdownMenuItem<String>(
                value: '15m',
                child: Text('15 minutos', overflow: TextOverflow.ellipsis),
              ),
              DropdownMenuItem<String>(
                value: '1h',
                child: Text('1 hora', overflow: TextOverflow.ellipsis),
              ),
              DropdownMenuItem<String>(
                value: '24h',
                child: Text('24 horas', overflow: TextOverflow.ellipsis),
              ),
            ],
            onChanged: (v) {
              if (v != null) setState(() => _timeframe = v);
            },
          ),
        ],
        if (keys.contains('warningThreshold')) ...<Widget>[
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 480,
            children: <Widget>[
              TextField(
                controller: _warnCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Alerta (%)'),
              ),
              TextField(
                controller: _critCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Crítico (%)'),
              ),
            ],
          ),
        ],
        const SizedBox(height: 18),
        FilledButton(
          onPressed: _saving ? null : _save,
          child: Text(
            widget.existing != null ? 'Salvar alterações' : 'Adicionar widget',
          ),
        ),
      ],
    );
  }
}

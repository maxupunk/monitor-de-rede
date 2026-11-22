import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/device_defaults.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_proactive_widgets.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart'
    show showDeviceAuthFailureDialog;
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_access_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_editor_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_output_view.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_review_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_settings_form.dart';
import 'package:netmonitor_app/features/plugins/widgets/wifi_qr_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba Plugins embutida em `/devices/:id` via `DeviceDetailTabRegistry`.
///
/// TODOS os destaques, abas e badges de plugin usam obrigatoriamente
/// `SemanticTone.plugin` (teal `#00796B` claro / `#1DE9B6` escuro — AGENTS.md §12).
class DevicePluginsTab extends ConsumerStatefulWidget {
  const DevicePluginsTab({super.key, required this.deviceId});

  final int deviceId;

  @override
  ConsumerState<DevicePluginsTab> createState() => _DevicePluginsTabState();
}

class _DevicePluginsTabState extends ConsumerState<DevicePluginsTab> {
  /// `'plugin-<id>'` | `'catalog'` | `'credentials'` | `'history'`
  String _section = 'catalog';
  bool _initializedSection = false;
  int? _activeRunId;
  int? _installingPluginId;
  String? _actionError;
  final Map<int, Map<String, dynamic>> _settingsDrafts =
      <int, Map<String, dynamic>>{};

  Future<void> _runPluginOp(
    Future<void> Function() op, {
    String? successMessage,
  }) async {
    if (mounted) setState(() => _actionError = null);
    try {
      await op();
      if (mounted && successMessage != null) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(successMessage)));
      }
    } catch (err) {
      if (!mounted) return;
      final platform = ref
          .read(pluginsProvider)
          .deviceViews[widget.deviceId]
          ?.platform;
      final msg = isDeviceAuthFailure(err)
          ? formatDeviceAuthFailureMessage(err, platformOrOs: platform)
          : (err is ApiException && err.message.trim().isNotEmpty
                ? err.message.trim()
                : err.toString());
      setState(() => _actionError = msg);
      if (isDeviceAuthFailure(err)) {
        await showDeviceAuthFailureDialog(context, err, platformOrOs: platform);
      } else {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(msg)));
      }
    }
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await ref.read(pluginsProvider.notifier).loadDevice(widget.deviceId);
      if (mounted) _selectDefaultSection();
    });
  }

  void _selectDefaultSection() {
    if (_initializedSection) return;
    final view = ref.read(pluginsProvider).deviceViews[widget.deviceId];
    if (view == null) return;
    final firstInstalled = view.plugins.where((p) => p.installed).firstOrNull;
    setState(() {
      _section = firstInstalled != null
          ? 'plugin-${firstInstalled.plugin.id}'
          : 'catalog';
      _initializedSection = true;
    });
  }

  Future<void> _executeAction(PluginSummary plugin, PluginAction action) async {
    if (action.reveals) {
      await showEphemeralRevealDialog(
        context: context,
        deviceId: widget.deviceId,
        pluginId: plugin.id,
        actionName: action.name,
        actionTitle: action.title,
      );
      return;
    }

    final hasParams = parsePluginSchemaFields(action.paramsSchema).isNotEmpty;
    if (!hasParams && !action.isWrite) {
      await _runPluginOp(() async {
        final runId = await withDeviceAccess<int>(
          context: context,
          ref: ref,
          action: () => ref
              .read(pluginsProvider.notifier)
              .runAction(
                deviceId: widget.deviceId,
                pluginId: plugin.id,
                action: action.name,
              ),
        );
        if (runId != null && mounted) {
          setState(() => _activeRunId = runId);
        }
      });
      return;
    }

    var values = pluginSchemaDefaults(action.paramsSchema);
    var confirmWrite = false;
    final formKey = GlobalKey<FormState>();

    await showAdaptiveFormSheet<void>(
      context: context,
      title: action.title,
      subtitle: plugin.name,
      maxWidth: 560,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            return Form(
              key: formKey,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (action.description != null) ...<Widget>[
                    Text(action.description!),
                    const SizedBox(height: 12),
                  ],
                  PluginSettingsForm(
                    schema: action.paramsSchema,
                    values: values,
                    onChanged: (next) => setModalState(() => values = next),
                    onRevealSecret: (fieldName, revealAction) async {
                      Map<String, dynamic>? out;
                      await _runPluginOp(() async {
                        out = await withDeviceAccess<Map<String, dynamic>>(
                          context: context,
                          ref: ref,
                          action: () => ref
                              .read(pluginsProvider.notifier)
                              .revealSecret(
                                deviceId: widget.deviceId,
                                pluginId: plugin.id,
                                action: revealAction,
                                params: values,
                              ),
                        );
                      });
                      return out?[fieldName]?.toString() ??
                          out?['password']?.toString();
                    },
                  ),
                  if (action.isWrite) ...<Widget>[
                    const SizedBox(height: 10),
                    NestedPanel(
                      tone: SemanticTone.error,
                      child: Material(
                        type: MaterialType.transparency,
                        child: CheckboxListTile(
                          contentPadding: EdgeInsets.zero,
                          value: confirmWrite,
                          onChanged: (v) =>
                              setModalState(() => confirmWrite = v == true),
                          title: Text(
                            action.confirmMessage ??
                                'Confirmo que esta ação altera a configuração do equipamento.',
                            style: const TextStyle(fontSize: 13),
                          ),
                        ),
                      ),
                    ),
                  ],
                  const SizedBox(height: 14),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: <Widget>[
                      TextButton(
                        onPressed: () => Navigator.of(sheetCtx).pop(),
                        child: const Text('Cancelar'),
                      ),
                      const SizedBox(width: 8),
                      FilledButton.icon(
                        onPressed: action.isWrite && !confirmWrite
                            ? null
                            : () async {
                                if (!(formKey.currentState?.validate() ??
                                    false)) {
                                  return;
                                }
                                final normalized = normalizePluginFormParams(
                                  action.paramsSchema,
                                  values,
                                );
                                Navigator.of(sheetCtx).pop();
                                await _runPluginOp(() async {
                                  final runId = await withDeviceAccess<int>(
                                    context: context,
                                    ref: ref,
                                    retryAuto: !action.isWrite,
                                    actionLabel: action.title,
                                    action: () => ref
                                        .read(pluginsProvider.notifier)
                                        .runAction(
                                          deviceId: widget.deviceId,
                                          pluginId: plugin.id,
                                          action: action.name,
                                          params: normalized,
                                          confirmWrite: action.isWrite,
                                        ),
                                  );
                                  if (runId != null && mounted) {
                                    setState(() => _activeRunId = runId);
                                  }
                                });
                              },
                        icon: const Icon(Icons.play_arrow, size: 18),
                        label: const Text('Executar'),
                      ),
                    ],
                  ),
                ],
              ),
            );
          },
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(pluginsProvider);
    final isAdmin = ref.watch(authProvider).canManagePlugins;
    final view = state.deviceViews[widget.deviceId];
    final pluginColors = context.semanticColors.resolve(
      SemanticTone.plugin,
      context.colorScheme,
    );

    if (state.deviceLoading && view == null) {
      return const LoadingStateView(
        message: 'Carregando plugins do equipamento...',
      );
    }
    if (view == null) {
      return ErrorStateView(
        message: state.error ?? 'Não foi possível carregar os plugins.',
        onRetry: () =>
            ref.read(pluginsProvider.notifier).loadDevice(widget.deviceId),
      );
    }

    if (!_initializedSection) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_initializedSection) _selectDefaultSection();
      });
    }

    final installed = view.plugins
        .where((p) => p.installed)
        .toList(growable: false);

    return SingleChildScrollView(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          // Cabeçalho com plataforma, firmware e botão IA
          LayoutBuilder(
            builder: (context, constraints) {
              final aiEnabled =
                  ref.watch(aiSettingsProvider).settings?.enabled ?? true;
              final chipsWrap = Wrap(
                spacing: 8,
                runSpacing: 6,
                children: <Widget>[
                  SemanticChip(
                    label: 'Plataforma: ${view.platform}',
                    tone: SemanticTone.plugin,
                    icon: Icons.extension_outlined,
                    compact: true,
                  ),
                  if (view.firmware != null && view.firmware!.isNotEmpty)
                    SemanticChip(
                      label: 'Firmware: ${view.firmware}',
                      tone: SemanticTone.info,
                      compact: true,
                    ),
                ],
              );
              final aiBtn = AiAskButton(
                label: 'Criar ou usar com IA',
                prompt:
                    'Quero consultar ou criar um plugin para o equipamento #${widget.deviceId} (${view.platform}).',
                contextData: AiChatContext(deviceId: widget.deviceId),
                compact: true,
              );
              if (!aiEnabled) return chipsWrap;
              if (constraints.maxWidth < 520) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    chipsWrap,
                    const SizedBox(height: 8),
                    SymmetricalActionBar(children: <Widget>[aiBtn]),
                  ],
                );
              }
              return Wrap(
                spacing: 8,
                runSpacing: 8,
                alignment: WrapAlignment.spaceBetween,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[chipsWrap, aiBtn],
              );
            },
          ),
          const SizedBox(height: 12),

          // Navegação por chips (instalados em teal `plugin` + Gerenciar)
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: <Widget>[
                for (final item in installed) ...<Widget>[
                  ChoiceChip(
                    selected: _section == 'plugin-${item.plugin.id}',
                    selectedColor: pluginColors.container,
                    side: BorderSide(color: pluginColors.border),
                    avatar: Icon(
                      Icons.extension_outlined,
                      size: 16,
                      color: pluginColors.foreground,
                    ),
                    label: Text(
                      item.plugin.status == 'active'
                          ? item.plugin.name
                          : '${item.plugin.name} (${pluginStatusPresentation(item.plugin.status).label})',
                      style: TextStyle(
                        color: _section == 'plugin-${item.plugin.id}'
                            ? pluginColors.onContainer
                            : null,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    onSelected: (_) =>
                        setState(() => _section = 'plugin-${item.plugin.id}'),
                  ),
                  const SizedBox(width: 8),
                ],
                ChoiceChip(
                  selected: _section == 'catalog',
                  avatar: const Icon(Icons.apps_outlined, size: 16),
                  label: const Text('Catálogo'),
                  onSelected: (_) => setState(() => _section = 'catalog'),
                ),
                const SizedBox(width: 8),
                ChoiceChip(
                  selected: _section == 'credentials',
                  avatar: const Icon(Icons.vpn_key_outlined, size: 16),
                  label: Text('Credenciais (${view.credentials.length})'),
                  onSelected: (_) => setState(() => _section = 'credentials'),
                ),
                const SizedBox(width: 8),
                ChoiceChip(
                  selected: _section == 'history',
                  avatar: const Icon(Icons.history, size: 16),
                  label: Text('Histórico (${view.runs.length})'),
                  onSelected: (_) => setState(() => _section = 'history'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 14),
          if (_actionError != null) ...<Widget>[
            NestedPanel(
              tone: SemanticTone.error,
              child: Row(
                children: <Widget>[
                  const Icon(Icons.error_outline, size: 18),
                  const SizedBox(width: 8),
                  Expanded(child: Text(_actionError!)),
                  IconButton(
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.close, size: 16),
                    onPressed: () => setState(() => _actionError = null),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
          ],

          if (_section.startsWith('plugin-'))
            _buildInstalledPluginSection(
              context,
              view,
              installed,
              isAdmin,
              state,
            )
          else if (_section == 'catalog')
            _buildCatalogSection(context, view, isAdmin)
          else if (_section == 'credentials')
            _buildCredentialsSection(context, view, isAdmin)
          else
            _buildHistorySection(context, view),
        ],
      ),
    );
  }

  Widget _buildInstalledPluginSection(
    BuildContext context,
    DevicePluginsView view,
    List<DevicePluginItem> installed,
    bool isAdmin,
    PluginsState state,
  ) {
    final id = int.tryParse(_section.replaceFirst('plugin-', ''));
    final item = installed.where((p) => p.plugin.id == id).firstOrNull;
    if (item == null) {
      return const EmptyStateView(
        title: 'Plugin não encontrado neste equipamento',
        icon: Icons.extension_off_outlined,
        tone: SemanticTone.plugin,
      );
    }

    final plugin = item.plugin;
    final activeRun = state.runById(widget.deviceId, _activeRunId);

    if (plugin.status == 'quarantine' || plugin.status == 'disabled') {
      final isQuarantine = plugin.status == 'quarantine';
      return SectionCard(
        title: plugin.name,
        subtitle: isQuarantine
            ? 'Este plugin está em quarentena e precisa passar pela revisão de segurança.'
            : 'Este plugin está desativado na biblioteca.',
        icon: Icons.gpp_maybe_outlined,
        iconTone: SemanticTone.warning,
        child: SymmetricalActionBar(
          children: <Widget>[
            if (isAdmin && isQuarantine)
              FilledButton.icon(
                onPressed: () => _runPluginOp(() async {
                  final detail = await ref
                      .read(pluginsProvider.notifier)
                      .fetchDetail(plugin.id);
                  if (context.mounted) {
                    await showPluginReviewDialog(
                      context: context,
                      detail: detail,
                      refreshDeviceId: widget.deviceId,
                    );
                  }
                }),
                icon: const Icon(Icons.security_outlined, size: 18),
                label: const Text(
                  'Revisar segurança',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            if (isAdmin && !isQuarantine)
              FilledButton.icon(
                onPressed: () => _runPluginOp(
                  () => ref
                      .read(pluginsProvider.notifier)
                      .lifecycle(
                        plugin.id,
                        'enable',
                        refreshDeviceId: widget.deviceId,
                      ),
                  successMessage: 'Plugin "${plugin.name}" reativado.',
                ),
                icon: const Icon(Icons.power_settings_new, size: 18),
                label: const Text(
                  'Reativar plugin',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            if (isAdmin)
              OutlinedButton(
                onPressed: () => _runPluginOp(() async {
                  await ref
                      .read(pluginsProvider.notifier)
                      .uninstallPlugin(widget.deviceId, plugin.id);
                  if (mounted) setState(() => _section = 'catalog');
                }, successMessage: 'Plugin desinstalado do equipamento.'),
                child: const Text(
                  'Desinstalar',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
        ),
      );
    }

    final draft =
        _settingsDrafts[plugin.id] ??
        Map<String, dynamic>.from(
          item.deviceSettings ?? pluginSchemaDefaults(plugin.settings?.device),
        );

    final isCompact = MediaQuery.sizeOf(context).width < 520;
    final openAppBtn = plugin.opensApp
        ? OutlinedButton.icon(
            onPressed: () => context.go('/apps/${plugin.id}'),
            icon: const Icon(Icons.apps, size: 17),
            label: const Text('Abrir aplicativo'),
          )
        : null;

    final menuBtn = isAdmin
        ? PopupMenuButton<String>(
            tooltip: 'Opções do plugin',
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 36, minHeight: 36),
            onSelected: (cmd) async {
              if (cmd == 'validate') {
                await _runPluginOp(() async {
                  final runId = await withDeviceAccess<int>(
                    context: context,
                    ref: ref,
                    action: () => ref
                        .read(pluginsProvider.notifier)
                        .validatePlugin(widget.deviceId, plugin.id),
                  );
                  if (runId != null && mounted) {
                    setState(() => _activeRunId = runId);
                  }
                });
              } else if (cmd == 'edit') {
                await _runPluginOp(() async {
                  final detail = await ref
                      .read(pluginsProvider.notifier)
                      .fetchDetail(plugin.id);
                  if (context.mounted) {
                    await showPluginEditorDialog(
                      context: context,
                      existing: detail,
                      exclusiveDeviceId: widget.deviceId,
                    );
                  }
                });
              } else if (cmd == 'uninstall') {
                await _runPluginOp(() async {
                  await ref
                      .read(pluginsProvider.notifier)
                      .uninstallPlugin(widget.deviceId, plugin.id);
                  if (mounted) setState(() => _section = 'catalog');
                }, successMessage: 'Plugin desinstalado do equipamento.');
              }
            },
            itemBuilder: (_) => const <PopupMenuEntry<String>>[
              PopupMenuItem<String>(
                value: 'validate',
                child: Text('Validar neste equipamento'),
              ),
              PopupMenuItem<String>(
                value: 'edit',
                child: Text('Ver código / Editar'),
              ),
              PopupMenuItem<String>(
                value: 'uninstall',
                child: Text('Desinstalar'),
              ),
            ],
          )
        : null;

    final Widget? headerTrailing = (isCompact || openAppBtn == null)
        ? menuBtn
        : (menuBtn != null
              ? Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    openAppBtn,
                    const SizedBox(width: 6),
                    menuBtn,
                  ],
                )
              : openAppBtn);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SectionCard(
          title: plugin.name,
          subtitle: plugin.description ?? 'v${plugin.version}',
          icon: Icons.extension_outlined,
          iconTone: SemanticTone.plugin,
          trailing: headerTrailing,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (isCompact && openAppBtn != null) ...<Widget>[
                SymmetricalActionBar(children: <Widget>[openAppBtn]),
                const SizedBox(height: 10),
              ],
              Text(
                'Ações disponíveis',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 8),
              SymmetricalActionBar(
                children: plugin.actions
                    .map((action) {
                      final isWrite = action.isWrite;
                      return FilledButton.tonalIcon(
                        style: FilledButton.styleFrom(
                          backgroundColor: isWrite
                              ? context.semanticColors.errorContainer
                              : context.semanticColors.pluginContainer,
                          foregroundColor: isWrite
                              ? context.semanticColors.onErrorContainer
                              : context.semanticColors.onPluginContainer,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                        ),
                        onPressed: (isWrite || action.reveals) && !isAdmin
                            ? null
                            : () => _executeAction(plugin, action),
                        icon: Icon(
                          action.reveals
                              ? Icons.vpn_key_outlined
                              : (isWrite
                                    ? Icons.edit_outlined
                                    : Icons.play_arrow_outlined),
                          size: 18,
                        ),
                        label: Text(
                          action.title,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      );
                    })
                    .toList(growable: false),
              ),
              if (activeRun != null &&
                  activeRun.pluginId == plugin.id) ...<Widget>[
                const Divider(height: 24),
                Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Wrap(
                      spacing: 8,
                      runSpacing: 4,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        Text(
                          'Execução #${activeRun.id} (${activeRun.action})',
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        SemanticChip(
                          label: runStatusPresentation(activeRun.status).label,
                          tone: runStatusPresentation(activeRun.status).tone,
                          compact: true,
                        ),
                      ],
                    ),
                    if (!activeRun.isFinished)
                      TextButton(
                        onPressed: () => ref
                            .read(pluginsProvider.notifier)
                            .cancelRun(activeRun.id),
                        child: const Text('Parar'),
                      ),
                  ],
                ),
                if (activeRun.error != null) ...<Widget>[
                  const SizedBox(height: 8),
                  NestedPanel(
                    tone: SemanticTone.error,
                    child: Text(activeRun.error!),
                  ),
                ],
                if (activeRun.output != null) ...<Widget>[
                  const SizedBox(height: 8),
                  PluginOutputView(output: activeRun.output),
                ],
                if (activeRun.transcript.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  PluginTranscriptView(entries: activeRun.transcript),
                ],
              ],
            ],
          ),
        ),
        if (plugin.settings?.device != null) ...<Widget>[
          const SizedBox(height: 14),
          SectionCard(
            title: 'Configuração deste equipamento',
            icon: Icons.tune,
            iconTone: SemanticTone.plugin,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                PluginSettingsForm(
                  schema: plugin.settings!.device,
                  values: draft,
                  readOnly: !isAdmin,
                  onChanged: (next) =>
                      setState(() => _settingsDrafts[plugin.id] = next),
                ),
                if (isAdmin) ...<Widget>[
                  const SizedBox(height: 10),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton.icon(
                      onPressed: () => _runPluginOp(
                        () => ref
                            .read(pluginsProvider.notifier)
                            .saveDeviceSettings(
                              widget.deviceId,
                              plugin.id,
                              normalizePluginFormParams(
                                plugin.settings!.device,
                                draft,
                                omitEmpty: false,
                              ),
                            ),
                        successMessage: 'Ajustes do equipamento salvos.',
                      ),
                      icon: const Icon(Icons.save_outlined, size: 18),
                      label: const Text('Salvar ajustes'),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ],
    );
  }

  Widget _buildCatalogSection(
    BuildContext context,
    DevicePluginsView view,
    bool isAdmin,
  ) {
    final compatible = view.plugins
        .where((p) => p.compatibility != 'incompatible')
        .toList(growable: false);
    final incompatible = view.plugins
        .where((p) => p.compatibility == 'incompatible')
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (isAdmin)
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: SymmetricalActionBar(
              children: <Widget>[
                FilledButton.icon(
                  style: FilledButton.styleFrom(
                    backgroundColor: context.semanticColors.plugin,
                    foregroundColor: context.semanticColors.onPlugin,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 10,
                    ),
                  ),
                  onPressed: () => showPluginEditorDialog(
                    context: context,
                    exclusiveDeviceId: widget.deviceId,
                  ),
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text(
                    'Novo plugin',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 10,
                    ),
                  ),
                  onPressed: () => context.go('/plugins'),
                  icon: const Icon(Icons.extension_outlined, size: 18),
                  label: const Text(
                    'Biblioteca completa',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
          ),
        if (compatible.isEmpty)
          const EmptyStateView(
            title: 'Nenhum plugin compatível encontrado',
            icon: Icons.extension_outlined,
            tone: SemanticTone.plugin,
          )
        else
          ResponsiveGrid(
            xs: 1,
            sm: 2,
            md: 2,
            lg: 3,
            xl: 3,
            children: compatible
                .map((item) {
                  final compat = compatPresentation(item.compatibility);
                  return SectionCard(
                    title: item.plugin.name,
                    subtitle:
                        item.plugin.description ?? 'v${item.plugin.version}',
                    icon: Icons.extension_outlined,
                    iconTone: SemanticTone.plugin,
                    trailing: SemanticChip(
                      label: compat.label,
                      tone: compat.tone,
                      compact: true,
                    ),
                    child: SymmetricalActionBar(
                      children: <Widget>[
                        if (item.installed)
                          FilledButton.tonal(
                            onPressed: () => setState(
                              () => _section = 'plugin-${item.plugin.id}',
                            ),
                            child: const Text('Abrir'),
                          )
                        else if (isAdmin)
                          FilledButton.icon(
                            style: FilledButton.styleFrom(
                              backgroundColor: context.semanticColors.plugin,
                              foregroundColor: context.semanticColors.onPlugin,
                            ),
                            onPressed: _installingPluginId != null
                                ? null
                                : () async {
                                    setState(
                                      () =>
                                          _installingPluginId = item.plugin.id,
                                    );
                                    try {
                                      await _runPluginOp(() async {
                                        final ok = await withDeviceAccess<bool>(
                                          context: context,
                                          ref: ref,
                                          action: () async {
                                            await ref
                                                .read(pluginsProvider.notifier)
                                                .installPlugin(
                                                  widget.deviceId,
                                                  item.plugin.id,
                                                );
                                            return true;
                                          },
                                        );
                                        if (ok == true && mounted) {
                                          setState(
                                            () => _section =
                                                'plugin-${item.plugin.id}',
                                          );
                                        }
                                      });
                                    } finally {
                                      if (mounted) {
                                        setState(
                                          () => _installingPluginId = null,
                                        );
                                      }
                                    }
                                  },
                            icon: _installingPluginId == item.plugin.id
                                ? const SizedBox(
                                    width: 15,
                                    height: 15,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.download_outlined, size: 18),
                            label: Text(
                              _installingPluginId == item.plugin.id
                                  ? 'Instalando...'
                                  : 'Instalar',
                            ),
                          ),
                      ],
                    ),
                  );
                })
                .toList(growable: false),
          ),
        if (incompatible.isNotEmpty) ...<Widget>[
          const SizedBox(height: 14),
          NestedPanel(
            padding: EdgeInsets.zero,
            child: Material(
              type: MaterialType.transparency,
              child: ExpansionTile(
                title: Text('Incompatíveis (${incompatible.length})'),
                children: incompatible
                    .map((item) {
                      final cleaned = item.reasons
                          .map(
                            (r) => r
                                .replaceFirst(RegExp(r'^[\s·•\-;,]+'), '')
                                .trim(),
                          )
                          .where((r) => r.isNotEmpty)
                          .join(' · ');
                      return ListTile(
                        title: Text(item.plugin.name),
                        subtitle: cleaned.isNotEmpty ? Text(cleaned) : null,
                      );
                    })
                    .toList(growable: false),
              ),
            ),
          ),
        ],
      ],
    );
  }

  Widget _buildCredentialsSection(
    BuildContext context,
    DevicePluginsView view,
    bool isAdmin,
  ) {
    final isCompact = MediaQuery.sizeOf(context).width < 520;
    final configureBtn = isAdmin
        ? FilledButton.icon(
            onPressed: () async {
              final saved = await showDeviceAccessDialog(
                context: context,
                ref: ref,
                deviceId: widget.deviceId,
              );
              if (saved) {
                await ref
                    .read(pluginsProvider.notifier)
                    .loadDevice(widget.deviceId);
              }
            },
            icon: const Icon(Icons.add, size: 18),
            label: const Text('Configurar acesso'),
          )
        : null;

    return SectionCard(
      title: 'Credenciais de acesso',
      subtitle:
          'Senhas nunca são exibidas; SSH suporta chave dedicada (ADR 016).',
      icon: Icons.vpn_key_outlined,
      iconTone: SemanticTone.plugin,
      trailing: isCompact ? null : configureBtn,
      footer: isCompact && configureBtn != null
          ? SymmetricalActionBar(children: <Widget>[configureBtn])
          : null,
      child: view.credentials.isEmpty
          ? const EmptyStateView(
              title: 'Nenhuma credencial cadastrada',
              description:
                  'Configure SSH, HTTP ou Telnet para executar plugins neste equipamento.',
              icon: Icons.lock_outline,
              tone: SemanticTone.plugin,
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                for (var i = 0; i < view.credentials.length; i++) ...<Widget>[
                  if (i > 0) const SizedBox(height: 10),
                  Builder(
                    builder: (context) {
                      final cred = view.credentials[i];
                      return NestedPanel(
                        padding: const EdgeInsets.all(12),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: <Widget>[
                            Row(
                              children: <Widget>[
                                SemanticChip(
                                  label: cred.kind.toUpperCase(),
                                  tone: SemanticTone.plugin,
                                  compact: true,
                                ),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: Text(
                                    '${cred.username} (porta ${cred.port ?? 'padrão'})',
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: context.textTheme.titleSmall
                                        ?.copyWith(fontWeight: FontWeight.w700),
                                  ),
                                ),
                                if (cred.dedicated != null)
                                  const SemanticChip(
                                    label: 'Chave dedicada',
                                    tone: SemanticTone.success,
                                    compact: true,
                                  )
                                else if (cred.sessionActive)
                                  const SemanticChip(
                                    label: 'Sessão ativa',
                                    tone: SemanticTone.info,
                                    compact: true,
                                  ),
                              ],
                            ),
                            const SizedBox(height: 6),
                            Text(
                              cred.dedicated != null
                                  ? 'Chave dedicada SSH (${cred.dedicated!.username})'
                                  : 'Armazenamento: ${cred.storage}${cred.sessionActive ? ' · Sessão ativa' : ''}',
                              style: context.textTheme.bodySmall?.copyWith(
                                color: context.colorScheme.onSurfaceVariant,
                              ),
                            ),
                            if (isAdmin) ...<Widget>[
                              const SizedBox(height: 10),
                              SymmetricalActionBar(
                                children: <Widget>[
                                  FilledButton.tonalIcon(
                                    style: FilledButton.styleFrom(
                                      visualDensity: VisualDensity.compact,
                                      padding: const EdgeInsets.symmetric(
                                        horizontal: 10,
                                        vertical: 8,
                                      ),
                                    ),
                                    onPressed: () async {
                                      final saved =
                                          await showDeviceAccessDialog(
                                            context: context,
                                            ref: ref,
                                            deviceId: widget.deviceId,
                                            lockedTransport: cred.kind,
                                          );
                                      if (saved) {
                                        await ref
                                            .read(pluginsProvider.notifier)
                                            .loadDevice(widget.deviceId);
                                      }
                                    },
                                    icon: const Icon(
                                      Icons.vpn_key_outlined,
                                      size: 16,
                                    ),
                                    label: const Text(
                                      'Gerenciar',
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                  OutlinedButton.icon(
                                    style: OutlinedButton.styleFrom(
                                      foregroundColor:
                                          context.colorScheme.error,
                                      side: BorderSide(
                                        color: context.colorScheme.error,
                                      ),
                                      visualDensity: VisualDensity.compact,
                                      padding: const EdgeInsets.symmetric(
                                        horizontal: 10,
                                        vertical: 8,
                                      ),
                                    ),
                                    onPressed: () async {
                                      final ok = await showConfirmActionDialog(
                                        context: context,
                                        title: 'Remover credencial',
                                        message:
                                            'Remover a credencial ${cred.kind.toUpperCase()} deste equipamento?',
                                        confirmLabel: 'Remover',
                                      );
                                      if (!ok || !mounted) return;
                                      await _runPluginOp(
                                        () => ref
                                            .read(pluginsProvider.notifier)
                                            .deleteCredential(
                                              widget.deviceId,
                                              cred.kind,
                                            ),
                                        successMessage:
                                            'Credencial ${cred.kind.toUpperCase()} removida.',
                                      );
                                    },
                                    icon: const Icon(
                                      Icons.delete_outline,
                                      size: 16,
                                    ),
                                    label: const Text(
                                      'Remover',
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                ],
                              ),
                            ],
                          ],
                        ),
                      );
                    },
                  ),
                ],
              ],
            ),
    );
  }

  Widget _buildHistorySection(BuildContext context, DevicePluginsView view) {
    if (view.runs.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhuma execução recente',
        icon: Icons.history,
        tone: SemanticTone.plugin,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: view.runs
          .map((run) {
            final pres = runStatusPresentation(run.status);
            return Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: SectionCard(
                padding: EdgeInsets.zero,
                child: ExpansionTile(
                  title: Row(
                    children: <Widget>[
                      Expanded(
                        child: Text(
                          '${run.pluginName ?? 'Plugin #${run.pluginId}'} · ${run.action}',
                          style: const TextStyle(fontWeight: FontWeight.w700),
                        ),
                      ),
                      SemanticChip(
                        label: pres.label,
                        tone: pres.tone,
                        compact: true,
                      ),
                    ],
                  ),
                  subtitle: Text(
                    '${originLabel(run.origin)} · ${formatRelativeTime(run.createdAt)}',
                  ),
                  childrenPadding: const EdgeInsets.all(14),
                  children: <Widget>[
                    if (run.error != null) ...<Widget>[
                      NestedPanel(
                        tone: SemanticTone.error,
                        child: Text(run.error!),
                      ),
                      const SizedBox(height: 8),
                    ],
                    if (run.output != null) ...<Widget>[
                      PluginOutputView(output: run.output),
                      const SizedBox(height: 8),
                    ],
                    PluginTranscriptView(entries: run.transcript),
                  ],
                ),
              ),
            );
          })
          .toList(growable: false),
    );
  }
}

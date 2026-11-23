import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_plugin_installed_section.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_action_runner_sheet.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_output_view.dart';
import 'package:netmonitor_app/features/plugins/widgets/wifi_qr_dialog.dart';
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide CredentialView;
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

typedef PluginAiActionBuilder =
    Widget? Function(
      BuildContext context,
      WidgetRef ref,
      int deviceId,
      String platform,
    );

/// Aba Plugins embutida em `/devices/:id` via `DeviceDetailTabRegistry`.
///
/// TODOS os destaques, abas e badges de plugin usam obrigatoriamente
/// `SemanticTone.plugin` (teal `#00796B` claro / `#1DE9B6` escuro — AGENTS.md §12).
class DevicePluginsTab extends ConsumerStatefulWidget {
  const DevicePluginsTab({
    super.key,
    required this.deviceId,
    this.aiActionBuilder,
  });

  final int deviceId;
  final PluginAiActionBuilder? aiActionBuilder;

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
      final msg = formatDeviceAccessError(err, platformOrOs: platform);
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
    final hasParams = parsePluginSchemaFields(action.paramsSchema).isNotEmpty;
    if (action.reveals && !hasParams) {
      await showEphemeralRevealDialog(
        context: context,
        deviceId: widget.deviceId,
        pluginId: plugin.id,
        actionName: action.name,
        actionTitle: action.title,
      );
      return;
    }

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

    final normalized = await showPluginActionRunnerSheet(
      context: context,
      deviceId: widget.deviceId,
      plugin: plugin,
      action: action,
      runProtectedOp: (op) => _runPluginOp(op),
    );
    if (normalized == null || !mounted) return;

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
  }

  Future<void> _installPluginFromCatalog(int pluginId) async {
    setState(() => _installingPluginId = pluginId);
    try {
      await _runPluginOp(() async {
        final ok = await withDeviceAccess<bool>(
          context: context,
          ref: ref,
          action: () async {
            await ref
                .read(pluginsProvider.notifier)
                .installPlugin(widget.deviceId, pluginId);
            return true;
          },
        );
        if (ok == true && mounted) {
          setState(() => _section = 'plugin-$pluginId');
        }
      });
    } finally {
      if (mounted) setState(() => _installingPluginId = null);
    }
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
          LayoutBuilder(
            builder: (context, constraints) {
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
              final aiBtn = widget.aiActionBuilder?.call(
                context,
                ref,
                widget.deviceId,
                view.platform,
              );
              if (aiBtn == null) return chipsWrap;
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
            Builder(
              builder: (context) {
                final id = int.tryParse(_section.replaceFirst('plugin-', ''));
                final item = installed
                    .where((p) => p.plugin.id == id)
                    .firstOrNull;
                final draft = item == null
                    ? const <String, dynamic>{}
                    : (_settingsDrafts[item.plugin.id] ??
                          Map<String, dynamic>.from(
                            item.deviceSettings ??
                                pluginSchemaDefaults(
                                  item.plugin.settings?.device,
                                ),
                          ));
                return DevicePluginInstalledSection(
                  deviceId: widget.deviceId,
                  item: item,
                  isAdmin: isAdmin,
                  activeRun: state.runById(widget.deviceId, _activeRunId),
                  draft: draft,
                  onDraftChanged: (next) {
                    if (item == null) return;
                    setState(() => _settingsDrafts[item.plugin.id] = next);
                  },
                  onRunAction: _executeAction,
                  onActiveRunChanged: (runId) {
                    if (mounted) setState(() => _activeRunId = runId);
                  },
                  onUninstalled: () {
                    if (mounted) setState(() => _section = 'catalog');
                  },
                  runPluginOp: _runPluginOp,
                );
              },
            )
          else if (_section == 'catalog')
            DevicePluginCatalogSection(
              deviceId: widget.deviceId,
              view: view,
              isAdmin: isAdmin,
              installingPluginId: _installingPluginId,
              onSelectPlugin: (pluginId) =>
                  setState(() => _section = 'plugin-$pluginId'),
              onInstallPlugin: _installPluginFromCatalog,
            )
          else if (_section == 'credentials')
            _buildCredentialsSection(context, view, isAdmin)
          else
            _buildHistorySection(context, view),
        ],
      ),
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
                platformHint: view.platform,
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
                      final dedicatedUser = cred.dedicatedInfo?.username ?? '';
                      final effectiveUser = cred.username.isNotEmpty
                          ? cred.username
                          : dedicatedUser;
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
                                    '$effectiveUser (porta ${cred.port ?? 'padrão'})',
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: context.textTheme.titleSmall
                                        ?.copyWith(fontWeight: FontWeight.w700),
                                  ),
                                ),
                                if (cred.dedicated)
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
                              cred.dedicated
                                  ? 'Chave dedicada SSH (${dedicatedUser.isNotEmpty ? dedicatedUser : effectiveUser})'
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
                                            platformHint: view.platform,
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

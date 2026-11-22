import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_editor_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_output_view.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_review_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_settings_form.dart';
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide CredentialView;
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

typedef DevicePluginOpRunner =
    Future<void> Function(Future<void> Function() op, {String? successMessage});

/// Seção de um plugin instalado na aba Plugins do equipamento.
class DevicePluginInstalledSection extends ConsumerWidget {
  const DevicePluginInstalledSection({
    super.key,
    required this.deviceId,
    required this.item,
    required this.isAdmin,
    required this.activeRun,
    required this.draft,
    required this.onDraftChanged,
    required this.onRunAction,
    required this.onActiveRunChanged,
    required this.onUninstalled,
    required this.runPluginOp,
  });

  final int deviceId;
  final DevicePluginItem? item;
  final bool isAdmin;
  final PluginRunView? activeRun;
  final Map<String, dynamic> draft;
  final ValueChanged<Map<String, dynamic>> onDraftChanged;
  final void Function(PluginSummary plugin, PluginAction action) onRunAction;
  final ValueChanged<int> onActiveRunChanged;
  final VoidCallback onUninstalled;
  final DevicePluginOpRunner runPluginOp;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (item == null) {
      return const EmptyStateView(
        title: 'Plugin não encontrado neste equipamento',
        icon: Icons.extension_off_outlined,
        tone: SemanticTone.plugin,
      );
    }

    final plugin = item!.plugin;

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
                onPressed: () => runPluginOp(() async {
                  final detail = await ref
                      .read(pluginsProvider.notifier)
                      .fetchDetail(plugin.id);
                  if (context.mounted) {
                    await showPluginReviewDialog(
                      context: context,
                      detail: detail,
                      refreshDeviceId: deviceId,
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
                onPressed: () => runPluginOp(
                  () => ref
                      .read(pluginsProvider.notifier)
                      .lifecycle(
                        plugin.id,
                        'enable',
                        refreshDeviceId: deviceId,
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
                onPressed: () => runPluginOp(() async {
                  await ref
                      .read(pluginsProvider.notifier)
                      .uninstallPlugin(deviceId, plugin.id);
                  onUninstalled();
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
                await runPluginOp(() async {
                  final runId = await withDeviceAccess<int>(
                    context: context,
                    ref: ref,
                    action: () => ref
                        .read(pluginsProvider.notifier)
                        .validatePlugin(deviceId, plugin.id),
                  );
                  if (runId != null) onActiveRunChanged(runId);
                });
              } else if (cmd == 'edit') {
                await runPluginOp(() async {
                  final detail = await ref
                      .read(pluginsProvider.notifier)
                      .fetchDetail(plugin.id);
                  if (context.mounted) {
                    await showPluginEditorDialog(
                      context: context,
                      existing: detail,
                      exclusiveDeviceId: deviceId,
                    );
                  }
                });
              } else if (cmd == 'uninstall') {
                await runPluginOp(() async {
                  await ref
                      .read(pluginsProvider.notifier)
                      .uninstallPlugin(deviceId, plugin.id);
                  onUninstalled();
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
                            : () => onRunAction(plugin, action),
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
                  activeRun!.pluginId == plugin.id) ...<Widget>[
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
                          'Execução #${activeRun!.id} (${activeRun!.action})',
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        SemanticChip(
                          label: runStatusPresentation(activeRun!.status).label,
                          tone: runStatusPresentation(activeRun!.status).tone,
                          compact: true,
                        ),
                      ],
                    ),
                    if (!activeRun!.isFinished)
                      TextButton(
                        onPressed: () => ref
                            .read(pluginsProvider.notifier)
                            .cancelRun(activeRun!.id),
                        child: const Text('Parar'),
                      ),
                  ],
                ),
                if (activeRun!.error != null) ...<Widget>[
                  const SizedBox(height: 8),
                  NestedPanel(
                    tone: SemanticTone.error,
                    child: Text(activeRun!.error!),
                  ),
                ],
                if (activeRun!.output != null) ...<Widget>[
                  const SizedBox(height: 8),
                  PluginOutputView(output: activeRun!.output),
                ],
                if (activeRun!.transcript.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  PluginTranscriptView(entries: activeRun!.transcript),
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
                  onChanged: onDraftChanged,
                ),
                if (isAdmin) ...<Widget>[
                  const SizedBox(height: 10),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton.icon(
                      onPressed: () => runPluginOp(
                        () => ref
                            .read(pluginsProvider.notifier)
                            .saveDeviceSettings(
                              deviceId,
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
}

/// Seção de catálogo de plugins compatíveis / incompatíveis para o equipamento.
class DevicePluginCatalogSection extends ConsumerWidget {
  const DevicePluginCatalogSection({
    super.key,
    required this.deviceId,
    required this.view,
    required this.isAdmin,
    required this.installingPluginId,
    required this.onSelectPlugin,
    required this.onInstallPlugin,
  });

  final int deviceId;
  final DevicePluginsView view;
  final bool isAdmin;
  final int? installingPluginId;
  final ValueChanged<int> onSelectPlugin;
  final ValueChanged<int> onInstallPlugin;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
                    exclusiveDeviceId: deviceId,
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
                  final isInstalling = installingPluginId == item.plugin.id;
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
                            onPressed: () => onSelectPlugin(item.plugin.id),
                            child: const Text('Abrir'),
                          )
                        else if (isAdmin)
                          FilledButton.icon(
                            style: FilledButton.styleFrom(
                              backgroundColor: context.semanticColors.plugin,
                              foregroundColor: context.semanticColors.onPlugin,
                            ),
                            onPressed: installingPluginId != null
                                ? null
                                : () => onInstallPlugin(item.plugin.id),
                            icon: isInstalling
                                ? const SizedBox(
                                    width: 15,
                                    height: 15,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.download_outlined, size: 18),
                            label: Text(
                              isInstalling ? 'Instalando...' : 'Instalar',
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
                      final cleaned = cleanPluginReasonMessages(
                        item.reasons,
                      ).join(' · ');
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
}

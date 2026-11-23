import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/fleet_action_flow.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_fleet_add_sheet.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_output_view.dart';
import 'package:netmonitor_app/features/plugins/widgets/wifi_qr_dialog.dart';
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide CredentialView;
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

typedef FleetOpRunner =
    Future<void> Function(Future<void> Function() op, {String? successMessage});

/// Aba declarativa de itens lidos da frota (`listSpec`).
class PluginAppItemsTab extends StatelessWidget {
  const PluginAppItemsTab({
    super.key,
    required this.fleet,
    required this.listSpec,
    required this.latestStatus,
    required this.isAdmin,
  });

  final FleetView fleet;
  final PluginItemListSpec listSpec;
  final PluginBatchView? latestStatus;
  final bool isAdmin;

  @override
  Widget build(BuildContext context) {
    final rawResult = latestStatus?.result;
    final items = <Map<String, dynamic>>[];
    if (rawResult is Map && rawResult[listSpec.itemsKey] is List) {
      items.addAll(asJsonMapList(rawResult[listSpec.itemsKey]));
    } else if (rawResult is List) {
      items.addAll(asJsonMapList(rawResult));
    }
    if (items.isEmpty && latestStatus != null) {
      final seenKeys = <String>{};
      final keyField = listSpec.itemKey ?? 'ssid';
      for (final dev in latestStatus!.devices) {
        final out = dev.output;
        final devList = out is Map
            ? asJsonMapList(out[listSpec.itemsKey])
            : asJsonMapList(out);
        for (final map in devList) {
          final dedupKey = map[keyField]?.toString();
          if (dedupKey != null && dedupKey.isNotEmpty) {
            if (!seenKeys.add(dedupKey)) continue;
          }
          items.add(map);
        }
      }
    }

    final addAction = listSpec.addAction != null
        ? fleet.plugin.actionByName(listSpec.addAction!.action)
        : null;

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (isAdmin && addAction != null) ...<Widget>[
            LayoutBuilder(
              builder: (context, constraints) {
                final addBtn = FilledButton.icon(
                  style: FilledButton.styleFrom(
                    backgroundColor: context.semanticColors.plugin,
                    foregroundColor: context.semanticColors.onPlugin,
                  ),
                  onPressed: () => showFleetActionFlow(
                    context: context,
                    fleet: fleet,
                    action: addAction,
                  ),
                  icon: const Icon(Icons.add, size: 18),
                  label: Text(
                    listSpec.addAction!.title ?? addAction.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                );
                if (constraints.maxWidth < 520) {
                  return SymmetricalActionBar(children: <Widget>[addBtn]);
                }
                return Align(alignment: Alignment.centerRight, child: addBtn);
              },
            ),
            const SizedBox(height: 12),
          ],
          if (items.isEmpty)
            const EmptyStateView(
              title: 'Nenhum item lido da frota ainda',
              description:
                  'Toque em "Atualizar" para consultar o estado atual nos equipamentos.',
              icon: Icons.view_list_outlined,
              tone: SemanticTone.plugin,
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 2,
              md: 2,
              lg: 3,
              xl: 3,
              children: items
                  .map((item) {
                    final primaryKey = listSpec.itemKey;
                    final title =
                        ((primaryKey != null ? item[primaryKey] : null) ??
                                item['ssid'] ??
                                item['name'] ??
                                item['title'] ??
                                item['id'] ??
                                'Item')
                            .toString();
                    final wifiQr = listSpec.wifiQr;
                    final editAction = listSpec.editAction != null
                        ? fleet.plugin.actionByName(listSpec.editAction!.action)
                        : null;
                    final cardActions = <Widget>[
                      if (wifiQr != null && isAdmin)
                        OutlinedButton.icon(
                          style: OutlinedButton.styleFrom(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () {
                            final target = fleet.members
                                .where((m) => m.credentialsReady)
                                .firstOrNull;
                            if (target == null) return;
                            final revealParams = <String, dynamic>{};
                            wifiQr.revealFields.forEach((paramKey, itemKey) {
                              if (item.containsKey(itemKey)) {
                                revealParams[paramKey] = item[itemKey];
                              }
                            });
                            showWifiQrRevealDialog(
                              context: context,
                              deviceId: target.deviceId,
                              deviceName: target.deviceName,
                              pluginId: fleet.plugin.id,
                              ssid: (item[wifiQr.ssid] ?? title).toString(),
                              encryption: (item[wifiQr.encryption] ?? '')
                                  .toString(),
                              hidden:
                                  wifiQr.hidden != null &&
                                  item[wifiQr.hidden] == true,
                              revealAction: wifiQr.revealAction,
                              revealParams: revealParams,
                              secretField: wifiQr.secretField,
                            );
                          },
                          icon: const Icon(Icons.qr_code_2, size: 17),
                          label: const Text(
                            'QR Wi-Fi',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      if (editAction != null && isAdmin)
                        FilledButton.tonalIcon(
                          style: FilledButton.styleFrom(
                            backgroundColor:
                                context.semanticColors.pluginContainer,
                            foregroundColor:
                                context.semanticColors.onPluginContainer,
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () => showFleetActionFlow(
                            context: context,
                            fleet: fleet,
                            action: editAction,
                            initialParams: item,
                          ),
                          icon: const Icon(Icons.edit_outlined, size: 17),
                          label: const Text(
                            'Editar',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                    ];

                    return SectionCard(
                      title: title,
                      icon: fleet.plugin.wifi
                          ? Icons.wifi
                          : Icons.extension_outlined,
                      iconTone: SemanticTone.plugin,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          for (final entry
                              in item.entries
                                  .where(
                                    (e) =>
                                        !e.key.startsWith('_') &&
                                        e.key != (primaryKey ?? 'ssid'),
                                  )
                                  .take(5))
                            KeyValueRow(
                              label: humanizePluginFieldKey(entry.key),
                              value: '${entry.value}',
                            ),
                          if (cardActions.isNotEmpty) ...<Widget>[
                            const SizedBox(height: 10),
                            SymmetricalActionBar(children: cardActions),
                          ],
                        ],
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
        ],
      ),
    );
  }
}

/// Aba de membros e verificação da frota.
class PluginAppMembersTab extends ConsumerWidget {
  const PluginAppMembersTab({
    super.key,
    required this.pluginId,
    required this.fleet,
    required this.appsState,
    required this.isAdmin,
    required this.runFleetOp,
  });

  final int pluginId;
  final FleetView fleet;
  final PluginAppsState appsState;
  final bool isAdmin;
  final FleetOpRunner runFleetOp;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final doubtfulCount =
        fleet.members.where((m) => m.compatibility == 'possible').length +
        fleet.candidates.where((c) => c.compatibility == 'possible').length;
    final hasToolbar =
        isAdmin && (fleet.candidates.isNotEmpty || doubtfulCount > 0);

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (hasToolbar) ...<Widget>[
            LayoutBuilder(
              builder: (context, constraints) {
                final hasBoth =
                    fleet.candidates.isNotEmpty && doubtfulCount > 0;
                final compactLabels = hasBoth && constraints.maxWidth < 440;
                final addLabel = compactLabels
                    ? 'Adicionar (${fleet.candidates.length})'
                    : 'Adicionar equipamento (${fleet.candidates.length})';
                final verifyLabel = compactLabels
                    ? 'Verificar ($doubtfulCount)'
                    : 'Verificar sistema ($doubtfulCount)';

                return SymmetricalActionBar(
                  children: <Widget>[
                    if (fleet.candidates.isNotEmpty)
                      FilledButton.icon(
                        style: FilledButton.styleFrom(
                          backgroundColor: context.semanticColors.plugin,
                          foregroundColor: context.semanticColors.onPlugin,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                        ),
                        onPressed: () => showPluginFleetAddSheet(
                          context: context,
                          pluginId: pluginId,
                        ),
                        icon: const Icon(Icons.add, size: 18),
                        label: Text(
                          addLabel,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    if (doubtfulCount > 0)
                      OutlinedButton.icon(
                        style: OutlinedButton.styleFrom(
                          foregroundColor: context.semanticColors.plugin,
                          side: BorderSide(
                            color: context.semanticColors.plugin,
                          ),
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                        ),
                        onPressed: appsState.identifying
                            ? null
                            : () => ref
                                  .read(pluginAppsProvider.notifier)
                                  .identify(pluginId),
                        icon: appsState.identifying
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.radar, size: 18),
                        label: Text(
                          verifyLabel,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                  ],
                );
              },
            ),
            const SizedBox(height: 12),
          ],
          if (fleet.members.isEmpty)
            const EmptyStateView(
              title: 'Nenhum equipamento neste aplicativo',
              description:
                  'Adicione equipamentos compatíveis para gerenciar em frota.',
              icon: Icons.devices_outlined,
              tone: SemanticTone.plugin,
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 2,
              md: 2,
              lg: 3,
              xl: 3,
              children: fleet.members
                  .map((member) {
                    final compat = compatPresentation(member.compatibility);
                    final subtitleParts = <String>[
                      if (member.ipAddress.trim().isNotEmpty)
                        member.ipAddress.trim(),
                      if (member.firmware != null &&
                          member.firmware!.trim().isNotEmpty)
                        member.firmware!.trim(),
                    ];
                    final cleanedReasons = cleanPluginReasonMessages(
                      member.reasons,
                    );
                    final memberActions = <Widget>[
                      if (!member.credentialsReady && isAdmin)
                        FilledButton.icon(
                          style: FilledButton.styleFrom(
                            backgroundColor: context.semanticColors.plugin,
                            foregroundColor: context.semanticColors.onPlugin,
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () async {
                            final ok = await showDeviceAccessDialog(
                              context: context,
                              ref: ref,
                              deviceId: member.deviceId,
                              deviceName: member.deviceName,
                              platformHint: member.platform,
                              lockedTransport: member.missingTransport,
                            );
                            if (ok) {
                              await ref
                                  .read(pluginAppsProvider.notifier)
                                  .loadFleet(pluginId);
                            }
                          },
                          icon: const Icon(Icons.vpn_key_outlined, size: 17),
                          label: const Text(
                            'Configurar acesso',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      FilledButton.tonalIcon(
                        style: FilledButton.styleFrom(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 10,
                            vertical: 10,
                          ),
                        ),
                        onPressed: () => context.go(
                          '/devices/${member.deviceId}?tab=plugins',
                        ),
                        icon: const Icon(Icons.open_in_new, size: 16),
                        label: const Text(
                          'Abrir dispositivo',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      if (isAdmin)
                        OutlinedButton.icon(
                          style: OutlinedButton.styleFrom(
                            foregroundColor: context.colorScheme.error,
                            side: BorderSide(color: context.colorScheme.error),
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () => runFleetOp(
                            () => ref
                                .read(pluginAppsProvider.notifier)
                                .removeMember(pluginId, member.deviceId),
                            successMessage:
                                '"${member.deviceName}" removido da frota.',
                          ),
                          icon: const Icon(Icons.link_off, size: 16),
                          label: const Text(
                            'Tirar da frota',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                    ];

                    return SectionCard(
                      title: member.deviceName,
                      subtitle: subtitleParts.isEmpty
                          ? null
                          : subtitleParts.join(' · '),
                      icon: Icons.router_outlined,
                      iconTone: SemanticTone.plugin,
                      trailing: SemanticChip(
                        label: compat.label,
                        tone: compat.tone,
                        compact: true,
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          for (final e in member.card.entries)
                            KeyValueRow(
                              label: humanizePluginFieldKey(e.key),
                              value: '${e.value}',
                            ),
                          if (member.compatibility != 'likely' &&
                              member.compatibility != 'validated' &&
                              cleanedReasons.isNotEmpty) ...<Widget>[
                            const SizedBox(height: 6),
                            NestedPanel(
                              tone: SemanticTone.warning,
                              padding: const EdgeInsets.all(10),
                              child: Text(
                                cleanedReasons.join(' · '),
                                style: context.textTheme.bodySmall,
                              ),
                            ),
                          ],
                          if (member.error != null) ...<Widget>[
                            const SizedBox(height: 6),
                            NestedPanel(
                              tone: SemanticTone.error,
                              child: Text(member.error!),
                            ),
                          ],
                          const SizedBox(height: 10),
                          SymmetricalActionBar(
                            primaryFullWidthWhenOdd: true,
                            children: memberActions,
                          ),
                        ],
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
        ],
      ),
    );
  }
}

/// Aba de ações avançadas da frota (`fleetSpec.actions`).
class PluginAppToolsTab extends StatelessWidget {
  const PluginAppToolsTab({
    super.key,
    required this.fleet,
    required this.isAdmin,
  });

  final FleetView fleet;
  final bool isAdmin;

  @override
  Widget build(BuildContext context) {
    final actions =
        fleet.plugin.fleet?.actions ?? const <Map<String, dynamic>>[];
    return SingleChildScrollView(
      child: ResponsiveGrid(
        xs: 1,
        sm: 2,
        md: 2,
        lg: 3,
        xl: 3,
        children: actions
            .map((raw) {
              final name = (raw['action'] ?? raw['name'] ?? raw['id'] ?? '')
                  .toString();
              final preview = raw['preview'] as String?;
              final action =
                  fleet.plugin.actionByName(name) ??
                  PluginAction(
                    name: name,
                    title: (raw['title'] as String?) ?? name,
                    description: raw['description'] as String?,
                    effect: (raw['effect'] as String?) ?? 'read',
                  );
              return SectionCard(
                title: action.title,
                subtitle: action.description,
                icon: Icons.play_circle_outline,
                iconTone: action.isWrite
                    ? SemanticTone.warning
                    : SemanticTone.plugin,
                child: SymmetricalActionBar(
                  children: <Widget>[
                    FilledButton.tonalIcon(
                      style: FilledButton.styleFrom(
                        backgroundColor: action.isWrite
                            ? context.semanticColors.warningContainer
                            : context.semanticColors.pluginContainer,
                        foregroundColor: action.isWrite
                            ? context.semanticColors.onWarningContainer
                            : context.semanticColors.onPluginContainer,
                      ),
                      onPressed: isAdmin
                          ? () => showFleetActionFlow(
                              context: context,
                              fleet: fleet,
                              action: action,
                              previewAction: preview,
                            )
                          : null,
                      icon: const Icon(Icons.play_arrow, size: 18),
                      label: const Text(
                        'Executar',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
              );
            })
            .toList(growable: false),
      ),
    );
  }
}

/// Aba de histórico de lotes da frota (`fleet.batches`).
class PluginAppHistoryTab extends ConsumerWidget {
  const PluginAppHistoryTab({
    super.key,
    required this.fleet,
    required this.appsState,
    required this.isAdmin,
    required this.runFleetOp,
  });

  final FleetView fleet;
  final PluginAppsState appsState;
  final bool isAdmin;
  final FleetOpRunner runFleetOp;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (fleet.batches.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhum lote executado ainda',
        icon: Icons.history,
        tone: SemanticTone.plugin,
      );
    }
    return ListView.separated(
      itemCount: fleet.batches.length,
      separatorBuilder: (_, _) => const SizedBox(height: 10),
      itemBuilder: (context, index) {
        final batch = fleet.batches[index];
        final pres = batchStatusPresentation(batch.status);
        return SectionCard(
          padding: EdgeInsets.zero,
          child: ExpansionTile(
            title: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    batch.title,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                ),
                SemanticChip(label: pres.label, tone: pres.tone, compact: true),
              ],
            ),
            subtitle: Text(
              '${countNoun(batch.devices.length, 'equipamento', 'equipamentos')} · ${formatRelativeTime(batch.createdAt)}',
            ),
            childrenPadding: const EdgeInsets.all(14),
            children: <Widget>[
              if (batch.isRunning && isAdmin)
                Align(
                  alignment: Alignment.centerRight,
                  child: OutlinedButton(
                    onPressed: () => runFleetOp(
                      () => ref
                          .read(pluginAppsProvider.notifier)
                          .cancelBatch(batch.id),
                      successMessage: 'Cancelamento solicitado.',
                    ),
                    child: const Text('Cancelar lote'),
                  ),
                ),
              if (batch.hasPatch && isAdmin) ...<Widget>[
                NestedPanel(
                  tone: SemanticTone.plugin,
                  child: Wrap(
                    spacing: 8,
                    runSpacing: 6,
                    alignment: WrapAlignment.spaceBetween,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      const Text(
                        'Este lote propõe ajustes para os equipamentos.',
                      ),
                      FilledButton.tonal(
                        onPressed: () => runFleetOp(
                          () => ref
                              .read(pluginAppsProvider.notifier)
                              .applyPatch(batch),
                          successMessage: 'Sugestão aplicada com sucesso.',
                        ),
                        child: const Text('Aceitar sugestão'),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 8),
              ],
              if (batch.result != null) ...<Widget>[
                PluginOutputView(output: batch.result),
                const SizedBox(height: 8),
              ],
              for (final d in batch.devices)
                ListTile(
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  title: Text(
                    d.deviceName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  subtitle: Text(
                    d.error ??
                        (d.runId != null &&
                                appsState.lastSteps[d.runId!] != null
                            ? appsState.lastSteps[d.runId!]!.request
                            : d.status),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      if (d.missingTransport != null && isAdmin)
                        TextButton(
                          style: TextButton.styleFrom(
                            visualDensity: VisualDensity.compact,
                            padding: const EdgeInsets.symmetric(horizontal: 8),
                          ),
                          onPressed: () async {
                            final ok = await showDeviceAccessDialog(
                              context: context,
                              ref: ref,
                              deviceId: d.deviceId,
                              deviceName: d.deviceName,
                              lockedTransport: d.missingTransport,
                            );
                            if (ok &&
                                ref
                                    .read(pluginAppsProvider.notifier)
                                    .canRerun(batch.id, d.deviceId)) {
                              await runFleetOp(
                                () => ref
                                    .read(pluginAppsProvider.notifier)
                                    .rerunDevice(
                                      batch.id,
                                      d.deviceId,
                                      confirmWrite: true,
                                    ),
                              );
                            }
                          },
                          child: const Text('Acesso'),
                        ),
                      SemanticChip(
                        label: batchStatusPresentation(d.status).label,
                        tone: batchStatusPresentation(d.status).tone,
                        compact: true,
                      ),
                    ],
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

/// Aba de documentação Markdown (`Como usar`).
class PluginAppAboutTab extends StatelessWidget {
  const PluginAppAboutTab({super.key, required this.fleet, this.detail});

  final FleetView fleet;
  final PluginDetail? detail;

  @override
  Widget build(BuildContext context) {
    final usage = detail?.package.usage ?? '';
    return SingleChildScrollView(
      child: SectionCard(
        title: 'Sobre ${fleet.plugin.name}',
        subtitle: '${fleet.plugin.slug} · v${fleet.plugin.version}',
        icon: Icons.help_outline,
        iconTone: SemanticTone.plugin,
        child: usage.trim().isEmpty
            ? Text(
                fleet.plugin.description ??
                    'Sem documentação de uso cadastrada para este plugin.',
              )
            : MarkdownBody(data: usage),
      ),
    );
  }
}

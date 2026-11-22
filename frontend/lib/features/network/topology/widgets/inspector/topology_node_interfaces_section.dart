import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Bloco organizado de metadados do equipamento (alinhamento esquerda/direita
/// limpo, divisores sutis e enriquecimento com MAC, SO, Rede e Uplink).
class TopologyNodeEquipmentInfoSection extends ConsumerWidget {
  const TopologyNodeEquipmentInfoSection({super.key, required this.node});

  final TopologyNodeModel node;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = context.colorScheme;
    final devices = ref.watch(devicesProvider).items;
    final device = devices.where((d) => d.id == node.id).firstOrNull;

    final vendorModel = <String>[
      if (node.vendor != null && node.vendor!.trim().isNotEmpty)
        node.vendor!.trim(),
      if (node.model != null && node.model!.trim().isNotEmpty)
        node.model!.trim(),
    ].join(' · ');
    final osName = (device?.effectiveOperatingSystem ?? device?.operatingSystem)
        ?.trim();
    final mac = device?.macAddress?.trim();
    final networkLabel = device?.network?.name.trim();
    final uplink = device?.linkInterfaceName?.trim();

    final rows = <Widget>[
      if (vendorModel.isNotEmpty)
        KeyValueRow(label: 'Fabricante / Modelo', value: vendorModel),
      KeyValueRow(label: 'Site', value: node.siteName ?? 'Sem site'),
      if (networkLabel != null && networkLabel.isNotEmpty)
        KeyValueRow(label: 'Rede IP', value: networkLabel),
      if (osName != null && osName.isNotEmpty)
        KeyValueRow(label: 'Sistema operacional', value: osName),
      if (mac != null && mac.isNotEmpty)
        KeyValueRow(
          label: 'Endereço MAC',
          value: mac,
          monospace: true,
          copyable: true,
        ),
      if (uplink != null && uplink.isNotEmpty)
        KeyValueRow(label: 'Interface uplink', value: uplink, monospace: true),
      Padding(
        padding: const EdgeInsets.symmetric(vertical: 7),
        child: Row(
          children: <Widget>[
            Expanded(
              child: Text(
                'SNMP',
                style: context.textTheme.bodySmall?.copyWith(
                  color: scheme.onSurfaceVariant,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            SemanticChip(
              label: node.snmpEnabled ? 'Ativo' : 'Desativado',
              tone: node.snmpEnabled
                  ? SemanticTone.success
                  : SemanticTone.warning,
              compact: true,
            ),
          ],
        ),
      ),
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Icon(Icons.info_outline, size: 16, color: scheme.primary),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                'Informações do equipamento',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 6),
        NestedPanel(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: withDividers(rows),
          ),
        ),
      ],
    );
  }
}

/// Seção dedicada de interfaces reais do equipamento na topologia: exibe nome,
/// status UP/DOWN, velocidade negociada, tráfego RX/TX e destaque do uplink.
class TopologyNodeInterfacesSection extends ConsumerStatefulWidget {
  const TopologyNodeInterfacesSection({super.key, required this.node});

  final TopologyNodeModel node;

  @override
  ConsumerState<TopologyNodeInterfacesSection> createState() =>
      _TopologyNodeInterfacesSectionState();
}

class _TopologyNodeInterfacesSectionState
    extends ConsumerState<TopologyNodeInterfacesSection> {
  bool _showAll = false;

  static const int _initialLimit = 6;

  @override
  void didUpdateWidget(covariant TopologyNodeInterfacesSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.node.id != widget.node.id) {
      _showAll = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final asyncIfaces = ref.watch(
      topologyNodeInterfacesProvider(widget.node.id),
    );
    final device = ref
        .watch(devicesProvider)
        .items
        .where((d) => d.id == widget.node.id)
        .firstOrNull;
    final uplinkName = device?.linkInterfaceName?.trim().toLowerCase();

    final loaded = asyncIfaces.valueOrNull;
    final totalCount = loaded?.length ?? widget.node.interfaceCount;
    final upCount = loaded?.where((i) => i.isUp).length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Icon(Icons.settings_ethernet, size: 16, color: scheme.primary),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                'Interfaces ($totalCount)',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
            if (upCount != null && totalCount > 0) ...<Widget>[
              SemanticChip(
                label: '$upCount UP',
                tone: upCount > 0 ? SemanticTone.success : SemanticTone.warning,
                compact: true,
              ),
            ],
          ],
        ),
        const SizedBox(height: 6),
        asyncIfaces.when(
          loading: () => const NestedPanel(
            padding: EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: <Widget>[
                SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                SizedBox(width: 10),
                Expanded(child: Text('Carregando interfaces...')),
              ],
            ),
          ),
          error: (error, stackTrace) => NestedPanel(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    '${formatInteger(widget.node.interfaceCount)} interfaces registradas (detalhes indisponíveis no momento).',
                    style: context.textTheme.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                ),
                IconButton(
                  tooltip: 'Tentar novamente',
                  visualDensity: VisualDensity.compact,
                  icon: const Icon(Icons.refresh, size: 18),
                  onPressed: () => ref.invalidate(
                    topologyNodeInterfacesProvider(widget.node.id),
                  ),
                ),
              ],
            ),
          ),
          data: (interfaces) {
            if (interfaces.isEmpty) {
              return NestedPanel(
                padding: const EdgeInsets.all(12),
                child: Text(
                  'Nenhuma interface coletada para este equipamento.',
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              );
            }

            final visible = _showAll
                ? interfaces
                : interfaces.take(_initialLimit).toList(growable: false);

            return NestedPanel(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  ...withDividers(
                    visible
                        .map(
                          (iface) => _TopologyInterfaceRow(
                            interfaceModel: iface,
                            isUplink:
                                uplinkName != null &&
                                uplinkName.isNotEmpty &&
                                iface.displayName.toLowerCase() == uplinkName,
                          ),
                        )
                        .toList(growable: false),
                  ),
                  if (interfaces.length > _initialLimit) ...<Widget>[
                    const Divider(height: 1),
                    TextButton.icon(
                      onPressed: () => setState(() => _showAll = !_showAll),
                      icon: Icon(
                        _showAll ? Icons.expand_less : Icons.expand_more,
                        size: 18,
                      ),
                      label: Text(
                        _showAll
                            ? 'Mostrar menos'
                            : 'Ver todas as ${interfaces.length} interfaces',
                      ),
                    ),
                  ],
                ],
              ),
            );
          },
        ),
      ],
    );
  }
}

class _TopologyInterfaceRow extends StatelessWidget {
  const _TopologyInterfaceRow({
    required this.interfaceModel,
    required this.isUplink,
  });

  final DeviceInterfaceModel interfaceModel;
  final bool isUplink;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final statusTone = interfaceModel.isUp
        ? SemanticTone.success
        : SemanticTone.error;
    final statusColors = context.semanticColors.resolve(statusTone, scheme);
    final speedBps = interfaceModel.negotiatedSpeedBps;
    final speedText = speedBps > 0 ? formatLinkSpeed(speedBps) : null;
    final subtitle = interfaceModel.subtitleText;
    final hasTraffic =
        (interfaceModel.inBps != null && interfaceModel.inBps! > 0) ||
        (interfaceModel.outBps != null && interfaceModel.outBps! > 0);

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 7),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: <Widget>[
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: statusColors.foreground,
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Flexible(
                      child: Text(
                        interfaceModel.displayName,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.bodyMedium?.copyWith(
                          fontWeight: FontWeight.w700,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ),
                    if (isUplink) ...<Widget>[
                      const SizedBox(width: 6),
                      const SemanticChip(
                        label: 'Uplink',
                        tone: SemanticTone.primary,
                        compact: true,
                      ),
                    ],
                  ],
                ),
                if (subtitle != null || hasTraffic) ...<Widget>[
                  const SizedBox(height: 2),
                  Text(
                    <String>[
                      ?subtitle,
                      if (hasTraffic)
                        'RX ${formatBps(interfaceModel.inBps ?? 0)} · TX ${formatBps(interfaceModel.outBps ?? 0)}',
                    ].join(' · '),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.labelSmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(width: 8),
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: <Widget>[
              Text(
                interfaceModel.isUp ? 'UP' : 'DOWN',
                style: context.textTheme.labelSmall?.copyWith(
                  fontWeight: FontWeight.w800,
                  color: statusColors.foreground,
                ),
              ),
              if (speedText != null)
                Text(
                  speedText,
                  style: context.textTheme.labelSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                    fontWeight: FontWeight.w600,
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

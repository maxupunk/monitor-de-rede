import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Seletor coeso para a interface de uplink / entrada de link no cadastro SNMP.
class SnmpUplinkInterfaceSelector extends StatelessWidget {
  const SnmpUplinkInterfaceSelector({
    super.key,
    required this.controller,
    required this.interfaces,
    required this.loading,
    required this.manualMode,
    required this.onManualModeChanged,
    this.hasQueried = false,
    this.onRequestInterfaces,
    this.onSelected,
  });

  static const String manualEntryValue = '__snmp_uplink_manual__';

  final TextEditingController controller;
  final List<DeviceInterfaceModel> interfaces;
  final bool loading;
  final bool hasQueried;
  final bool manualMode;
  final ValueChanged<bool> onManualModeChanged;
  final VoidCallback? onRequestInterfaces;
  final ValueChanged<String>? onSelected;

  static String formatNegotiatedSpeed(DeviceInterfaceModel iface) {
    final speed = iface.speed;
    if (speed == null || !speed.isFinite || speed <= 0) {
      return iface.isUp ? '—' : 'Sem link';
    }
    return formatLinkSpeed(speed);
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final sorted = DeviceInterfaceModel.sortForUplinkSelection(interfaces);
    final showSelect = sorted.isNotEmpty && !manualMode;
    final currentValue = controller.text.trim();
    final hasCustomValue =
        currentValue.isNotEmpty &&
        !sorted.any((iface) => iface.displayName == currentValue);
    final upCount = sorted.where((i) => i.isUp).length;
    final downCount = sorted.length - upCount;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (loading) ...<Widget>[
          Row(
            children: <Widget>[
              const SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Consultando interfaces SNMP...',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
        ],
        if (showSelect) ...<Widget>[
          AppDropdownFormField<String>(
            key: ValueKey<String>(
              'snmp-uplink-select-$currentValue-${sorted.length}',
            ),
            initialValue: currentValue,
            decoration: const InputDecoration(
              labelText: 'Interface de uplink / link',
            ),
            items: <DropdownMenuItem<String>>[
              DropdownMenuItem<String>(
                value: '',
                child: Text(
                  'Nenhuma (automático)',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: scheme.onSurfaceVariant),
                ),
              ),
              if (hasCustomValue)
                DropdownMenuItem<String>(
                  value: currentValue,
                  child: _CustomInterfaceOptionRow(name: currentValue),
                ),
              for (final iface in sorted)
                DropdownMenuItem<String>(
                  value: iface.displayName,
                  child: _SnmpInterfaceOptionRow(iface: iface),
                ),
              DropdownMenuItem<String>(
                value: manualEntryValue,
                child: Row(
                  children: <Widget>[
                    Icon(Icons.edit_outlined, size: 16, color: scheme.primary),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Informar manualmente...',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: scheme.primary,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
            onChanged: (value) {
              if (value == null) return;
              if (value == manualEntryValue) {
                onManualModeChanged(true);
                return;
              }
              controller.text = value;
              onSelected?.call(value);
            },
          ),
          const SizedBox(height: 6),
          Wrap(
            spacing: 10,
            runSpacing: 4,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              _InterfaceCountLegend(
                label: '$upCount UP',
                tone: SemanticTone.success,
              ),
              _InterfaceCountLegend(
                label: '$downCount DOWN',
                tone: SemanticTone.error,
              ),
            ],
          ),
        ] else ...<Widget>[
          TextFormField(
            controller: controller,
            decoration: InputDecoration(
              labelText: 'Interface de uplink / link',
              hintText: 'Ex.: ether1, wan0',
              suffixIcon:
                  sorted.isEmpty && onRequestInterfaces != null && !loading
                  ? IconButton(
                      tooltip: 'Listar interfaces SNMP',
                      icon: const Icon(Icons.settings_ethernet),
                      onPressed: onRequestInterfaces,
                    )
                  : null,
            ),
          ),
          const SizedBox(height: 6),
          if (manualMode && sorted.isNotEmpty)
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: TextButton.icon(
                onPressed: () => onManualModeChanged(false),
                style: TextButton.styleFrom(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  minimumSize: const Size(0, 32),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                icon: const Icon(Icons.list_alt, size: 16),
                label: Text('Selecionar da lista (${sorted.length})'),
              ),
            )
          else if (!loading)
            Text(
              hasQueried
                  ? 'Nenhuma interface SNMP retornada — informe manualmente.'
                  : 'Teste o SNMP ou clique em "Listar interfaces" para selecionar com status e velocidade.',
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
        ],
      ],
    );
  }
}

class _SnmpInterfaceOptionRow extends StatelessWidget {
  const _SnmpInterfaceOptionRow({required this.iface});

  final DeviceInterfaceModel iface;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final subtitle = iface.subtitleText;
    final speedLabel = SnmpUplinkInterfaceSelector.formatNegotiatedSpeed(iface);

    return Row(
      children: <Widget>[
        _InterfaceStatusBadge(isUp: iface.isUp),
        const SizedBox(width: 8),
        Expanded(
          child: Text.rich(
            TextSpan(
              text: iface.displayName,
              style: TextStyle(
                fontWeight: FontWeight.w600,
                color: scheme.onSurface,
              ),
              children: subtitle != null
                  ? <InlineSpan>[
                      TextSpan(
                        text: ' · $subtitle',
                        style: TextStyle(
                          fontWeight: FontWeight.w400,
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ]
                  : null,
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            softWrap: false,
          ),
        ),
        const SizedBox(width: 8),
        _InterfaceSpeedTag(
          label: speedLabel,
          isUp: iface.isUp,
          hasSpeed: iface.negotiatedSpeedBps > 0,
        ),
      ],
    );
  }
}

class _CustomInterfaceOptionRow extends StatelessWidget {
  const _CustomInterfaceOptionRow({required this.name});

  final String name;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final infoColors = context.semanticColors.resolve(
      SemanticTone.info,
      scheme,
    );

    return Row(
      children: <Widget>[
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
          decoration: BoxDecoration(
            color: infoColors.container,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: infoColors.border),
          ),
          child: Text(
            'ATUAL',
            style: TextStyle(
              fontSize: 10,
              fontWeight: FontWeight.w700,
              color: infoColors.onContainer,
              height: 1.1,
            ),
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            softWrap: false,
            style: TextStyle(
              fontWeight: FontWeight.w600,
              color: scheme.onSurface,
            ),
          ),
        ),
      ],
    );
  }
}

class _InterfaceStatusBadge extends StatelessWidget {
  const _InterfaceStatusBadge({required this.isUp});

  final bool isUp;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(
      isUp ? SemanticTone.success : SemanticTone.error,
      scheme,
    );

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: colors.container,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: colors.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(
              color: colors.foreground,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 4),
          Text(
            isUp ? 'UP' : 'DOWN',
            style: TextStyle(
              fontSize: 10.5,
              fontWeight: FontWeight.w800,
              color: colors.onContainer,
              height: 1.1,
            ),
          ),
        ],
      ),
    );
  }
}

class _InterfaceSpeedTag extends StatelessWidget {
  const _InterfaceSpeedTag({
    required this.label,
    required this.isUp,
    required this.hasSpeed,
  });

  final String label;
  final bool isUp;
  final bool hasSpeed;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final activeColors = context.semanticColors.resolve(
      SemanticTone.info,
      scheme,
    );
    final highlight = isUp && hasSpeed;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: highlight ? activeColors.container : scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(
          color: highlight ? activeColors.border : scheme.outlineVariant,
        ),
      ),
      child: Text(
        label,
        maxLines: 1,
        softWrap: false,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w700,
          color: highlight ? activeColors.onContainer : scheme.onSurfaceVariant,
          height: 1.1,
        ),
      ),
    );
  }
}

class _InterfaceCountLegend extends StatelessWidget {
  const _InterfaceCountLegend({required this.label, required this.tone});

  final String label;
  final SemanticTone tone;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(tone, scheme);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Container(
          width: 7,
          height: 7,
          decoration: BoxDecoration(
            color: colors.foreground,
            shape: BoxShape.circle,
          ),
        ),
        const SizedBox(width: 5),
        Text(
          label,
          style: context.textTheme.labelSmall?.copyWith(
            color: scheme.onSurfaceVariant,
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    );
  }
}

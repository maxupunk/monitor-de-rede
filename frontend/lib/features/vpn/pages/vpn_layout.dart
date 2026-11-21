import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Layout da área VPN WireGuard com alternador móvel-first entre Servidor e Dispositivos.
/// Quando [slivers] é fornecido, o cabeçalho rola junto com o conteúdo em um
/// `CustomScrollView`, evitando cabeçalhos fixos que sufocam listas no mobile.
class VpnLayout extends StatelessWidget {
  const VpnLayout({
    super.key,
    required this.activeTab,
    this.child,
    this.slivers,
  }) : assert(child != null || slivers != null);

  final String activeTab;
  final Widget? child;
  final List<Widget>? slivers;

  Widget _buildHeader(BuildContext context) {
    final isCompact = MediaQuery.sizeOf(context).width < 600;
    final titleRow = Row(
      children: <Widget>[
        Container(
          width: 38,
          height: 38,
          decoration: BoxDecoration(
            color: context.colorScheme.primaryContainer,
            borderRadius: BorderRadius.circular(10),
          ),
          child: Icon(
            Icons.vpn_lock_outlined,
            color: context.colorScheme.onPrimaryContainer,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                'VPN WireGuard',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style:
                    (isCompact
                            ? context.textTheme.titleLarge
                            : context.textTheme.headlineSmall)
                        ?.copyWith(fontWeight: FontWeight.w800),
              ),
              Text(
                'Túneis seguros para monitoramento e servidores remotos',
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
      ],
    );

    final switcher = SegmentedButton<String>(
      showSelectedIcon: false,
      selected: <String>{activeTab},
      onSelectionChanged: (selection) {
        final target = selection.first;
        if (target == 'server') {
          context.go('/vpn');
        } else {
          context.go('/vpn/devices');
        }
      },
      segments: const <ButtonSegment<String>>[
        ButtonSegment<String>(
          value: 'server',
          icon: Icon(Icons.dns_outlined, size: 18),
          label: Text('Servidor', maxLines: 1, overflow: TextOverflow.ellipsis),
        ),
        ButtonSegment<String>(
          value: 'devices',
          icon: Icon(Icons.devices_other_outlined, size: 18),
          label: Text(
            'Dispositivos',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );

    if (isCompact) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[titleRow, const SizedBox(height: 10), switcher],
      );
    }

    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        Expanded(child: titleRow),
        const SizedBox(width: 12),
        switcher,
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final isCompact = MediaQuery.sizeOf(context).width < 600;
    final headerGap = isCompact ? 10.0 : 14.0;
    final header = _buildHeader(context);
    if (slivers != null) {
      return CustomScrollView(
        slivers: <Widget>[
          SliverToBoxAdapter(
            child: Padding(
              padding: EdgeInsets.only(bottom: headerGap),
              child: header,
            ),
          ),
          ...slivers!,
        ],
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        header,
        SizedBox(height: headerGap),
        Expanded(child: child!),
      ],
    );
  }
}

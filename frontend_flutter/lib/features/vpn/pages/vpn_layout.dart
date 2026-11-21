import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Layout da área VPN WireGuard com alternador móvel-first entre Servidor e Dispositivos.
class VpnLayout extends StatelessWidget {
  const VpnLayout({super.key, required this.activeTab, required this.child});

  final String activeTab;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 12,
          runSpacing: 10,
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Row(
              mainAxisSize: MainAxisSize.min,
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
                Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      'VPN WireGuard',
                      style: context.textTheme.headlineSmall?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                    Text(
                      'Túneis seguros para monitoramento e servidores remotos',
                      style: context.textTheme.bodySmall?.copyWith(
                        color: context.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ],
            ),
            SegmentedButton<String>(
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
                  label: Text('Servidor'),
                ),
                ButtonSegment<String>(
                  value: 'devices',
                  icon: Icon(Icons.devices_other_outlined, size: 18),
                  label: Text('Dispositivos'),
                ),
              ],
            ),
          ],
        ),
        const SizedBox(height: 16),
        Expanded(child: child),
      ],
    );
  }
}

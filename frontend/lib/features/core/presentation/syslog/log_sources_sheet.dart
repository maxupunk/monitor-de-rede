import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Future<void> showLogSourcesSheet(BuildContext context, WidgetRef ref) async {
  await ref.read(logsProvider.notifier).fetchSources();
  if (!context.mounted) return;

  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Origens de Syslog',
    subtitle:
        'Vincule IPs ou hostnames desconhecidos aos dispositivos cadastrados.',
    maxWidth: 680,
    builder: (_) => const _LogSourcesSheetBody(),
  );
}

class _LogSourcesSheetBody extends ConsumerWidget {
  const _LogSourcesSheetBody();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final logs = ref.watch(logsProvider);
    final devices = ref.watch(monitorsProvider).devices;
    final auth = ref.watch(authProvider);

    if (logs.sources.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhuma origem recebida ainda',
        description:
            'Configure o envio de syslog em um equipamento para visualizar aqui.',
        icon: Icons.router_outlined,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final src in logs.sources) ...<Widget>[
          NestedPanel(
            tone: src.kind == 'unknown' ? SemanticTone.warning : null,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 4,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Text(
                      src.hostname != null && src.hostname!.isNotEmpty
                          ? '${src.hostname} (${src.sourceIp})'
                          : src.sourceIp,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    SemanticChip(
                      label: src.kind,
                      tone: src.kind == 'unknown'
                          ? SemanticTone.warning
                          : (src.kind == 'device'
                                ? SemanticTone.success
                                : SemanticTone.info),
                      compact: true,
                    ),
                    if (src.masked)
                      const SemanticChip(
                        label: 'NAT mascarado',
                        tone: SemanticTone.caution,
                        compact: true,
                      ),
                    if (src.droppedCount > 0)
                      SemanticChip(
                        label: '${src.droppedCount} descartadas',
                        tone: SemanticTone.error,
                        compact: true,
                      ),
                  ],
                ),
                if (src.lastMessage != null &&
                    src.lastMessage!.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 6),
                  Text(
                    src.lastMessage!,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.bodySmall?.copyWith(
                      fontFamily: 'monospace',
                    ),
                  ),
                ],
                if (auth.canWrite) ...<Widget>[
                  const SizedBox(height: 10),
                  AppDropdownFormField<int?>(
                    initialValue: devices.any((d) => d.id == src.deviceId)
                        ? src.deviceId
                        : null,
                    decoration: const InputDecoration(
                      labelText: 'Vincular ao dispositivo',
                      isDense: true,
                    ),
                    items: <DropdownMenuItem<int?>>[
                      const DropdownMenuItem<int?>(
                        value: null,
                        child: Text(
                          'Sem vínculo',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      for (final d in devices)
                        DropdownMenuItem<int?>(
                          value: d.id,
                          child: Text(d.name, overflow: TextOverflow.ellipsis),
                        ),
                    ],
                    onChanged: (devId) async {
                      try {
                        await ref
                            .read(logsProvider.notifier)
                            .bindSource(src.bindKey, devId);
                      } catch (err) {
                        if (!context.mounted) return;
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(content: Text(describeError(err))),
                        );
                      }
                    },
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: 10),
        ],
      ],
    );
  }
}

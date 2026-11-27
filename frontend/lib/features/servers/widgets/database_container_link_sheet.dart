import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/utils/database_presentation.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// "Vincular a um contêiner": o endereço de [connection] passa a acompanhar o
/// contêiner escolhido, e o backup o reencontra quando ele reinicia ou é
/// recriado com outro IP.
Future<DatabaseConnectionResponse?> showDatabaseContainerLinkSheet(
  BuildContext context, {
  required DatabaseConnectionResponse connection,
}) {
  return showAdaptiveFormSheet<DatabaseConnectionResponse>(
    context: context,
    title: 'Vincular ${connection.name} a um contêiner',
    subtitle:
        'O endereço passa a acompanhar o contêiner: se ele reiniciar com outro IP, o backup o reencontra sozinho.',
    maxWidth: 620,
    builder: (_) => DatabaseContainerLinkBody(connection: connection),
  );
}

class DatabaseContainerLinkBody extends ConsumerStatefulWidget {
  const DatabaseContainerLinkBody({super.key, required this.connection});

  final DatabaseConnectionResponse connection;

  @override
  ConsumerState<DatabaseContainerLinkBody> createState() =>
      _DatabaseContainerLinkBodyState();
}

class _DatabaseContainerLinkBodyState
    extends ConsumerState<DatabaseContainerLinkBody> {
  String? _linkingId;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(databasesProvider.notifier).fetchContainers(),
    );
  }

  Future<void> _link(DatabaseContainer container) async {
    setState(() {
      _linkingId = container.containerId;
      _error = null;
    });
    try {
      final saved = await ref
          .read(databasesProvider.notifier)
          .linkContainer(widget.connection.id, container);
      if (!mounted) return;
      Navigator.of(context).pop(saved);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _linkingId = null;
        _error = describeError(
          err,
          fallback: 'Não foi possível vincular ao contêiner.',
        );
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final dbState = ref.watch(databasesProvider);
    final ranked = rankContainersForConnection(
      widget.connection,
      dbState.containers.containers,
    );
    final busy = _linkingId != null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              child: Text(
                'Contêineres ${widget.connection.engineLabel} encontrados',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            TextButton.icon(
              onPressed: dbState.containersLoading || busy
                  ? null
                  : () =>
                        ref.read(databasesProvider.notifier).fetchContainers(),
              icon: const Icon(Icons.refresh, size: 18),
              label: const Text('Procurar de novo'),
            ),
          ],
        ),
        const SizedBox(height: 8),
        if (dbState.containersLoading)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 24),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (dbState.containersError != null)
          NestedPanel(
            tone: SemanticTone.error,
            child: Text(dbState.containersError!),
          )
        else if (ranked.isEmpty)
          const EmptyStateView(
            icon: Icons.search_off,
            tone: SemanticTone.warning,
            title: 'Nenhum contêiner compatível',
            description:
                'Nenhum host Docker conectado tem um contêiner deste banco livre. Confira se o agente do host está online.',
          )
        else
          ...ranked.map(
            (c) => Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: ChoiceCard(
                title: c.containerName,
                subtitle:
                    '${c.hostName} · ${c.image}${c.port != null ? ' · porta ${c.port}' : ''}',
                icon: Icons.storage_outlined,
                tone: c.running ? SemanticTone.success : SemanticTone.warning,
                enabled: !busy,
                badge: c.running ? 'Rodando' : 'Parado',
                badgeTone: c.running
                    ? SemanticTone.success
                    : SemanticTone.warning,
                trailing: _linkingId == c.containerId
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : null,
                onTap: () => _link(c),
              ),
            ),
          ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 8),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
      ],
    );
  }
}

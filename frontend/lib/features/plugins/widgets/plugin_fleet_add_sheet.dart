import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre o painel adaptativo para adicionar equipamentos candidatos à frota.
Future<void> showPluginFleetAddSheet({
  required BuildContext context,
  required int pluginId,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Adicionar equipamento à frota',
    subtitle:
        'Detecte o sistema operacional ou adicione os equipamentos compatíveis.',
    maxWidth: 540,
    builder: (_) => PluginFleetAddSheet(pluginId: pluginId),
  );
}

/// Conteúdo Material Design 3 de seleção/detecção de candidatos de frota.
class PluginFleetAddSheet extends ConsumerStatefulWidget {
  const PluginFleetAddSheet({super.key, required this.pluginId});

  final int pluginId;

  @override
  ConsumerState<PluginFleetAddSheet> createState() =>
      _PluginFleetAddSheetState();
}

class _PluginFleetAddSheetState extends ConsumerState<PluginFleetAddSheet> {
  final _searchCtrl = TextEditingController();
  final Set<int> _identifyingIds = <int>{};
  int? _addingId;
  String _query = '';
  String? _error;

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _detectCandidate(FleetCandidate candidate) async {
    setState(() {
      _identifyingIds.add(candidate.deviceId);
      _error = null;
    });
    try {
      await ref
          .read(pluginAppsProvider.notifier)
          .identify(widget.pluginId, deviceIds: <int>[candidate.deviceId]);
    } catch (err) {
      if (mounted) {
        setState(
          () => _error = 'Falha ao detectar sistema: ${describeError(err)}',
        );
      }
    } finally {
      if (mounted) {
        setState(() => _identifyingIds.remove(candidate.deviceId));
      }
    }
  }

  Future<void> _addCandidate(FleetCandidate candidate) async {
    setState(() {
      _addingId = candidate.deviceId;
      _error = null;
    });
    try {
      await ref
          .read(pluginAppsProvider.notifier)
          .addMember(widget.pluginId, candidate.deviceId);
      if (mounted) Navigator.of(context).pop();
    } catch (err) {
      if (mounted) {
        setState(() {
          _addingId = null;
          _error = 'Falha ao adicionar equipamento: ${describeError(err)}';
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final appsState = ref.watch(pluginAppsProvider);
    final candidates =
        appsState.fleets[widget.pluginId]?.candidates ??
        const <FleetCandidate>[];
    final showSearch = candidates.length > 5;

    final filtered = _query.isEmpty
        ? candidates
        : candidates
              .where((c) {
                final q = _query.toLowerCase();
                return c.deviceName.toLowerCase().contains(q) ||
                    c.ipAddress.toLowerCase().contains(q);
              })
              .toList(growable: false);

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 10),
        ],
        if (showSearch) ...<Widget>[
          TextField(
            controller: _searchCtrl,
            onChanged: (v) => setState(() => _query = v.trim()),
            decoration: InputDecoration(
              hintText: 'Buscar por nome ou IP...',
              prefixIcon: const Icon(Icons.search, size: 18),
              suffixIcon: _query.isNotEmpty
                  ? IconButton(
                      tooltip: 'Limpar busca',
                      icon: const Icon(Icons.clear, size: 18),
                      onPressed: () {
                        _searchCtrl.clear();
                        setState(() => _query = '');
                      },
                    )
                  : null,
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (filtered.isEmpty)
          const EmptyStateView(
            title: 'Nenhum equipamento candidato encontrado',
            icon: Icons.devices_outlined,
            tone: SemanticTone.plugin,
          )
        else
          for (var i = 0; i < filtered.length; i++) ...<Widget>[
            if (i > 0) const SizedBox(height: 10),
            _buildCandidateCard(context, filtered[i], appsState.identifying),
          ],
      ],
    );
  }

  Widget _buildCandidateCard(
    BuildContext context,
    FleetCandidate candidate,
    bool globalIdentifying,
  ) {
    final badge = compatPresentation(candidate.compatibility);
    final ip = candidate.ipAddress.trim();
    final statusMsg = formatCandidateStatusMessage(candidate);
    final isDetecting =
        _identifyingIds.contains(candidate.deviceId) ||
        (globalIdentifying && _identifyingIds.isEmpty);
    final isAdding = _addingId == candidate.deviceId;
    final busy = isDetecting || _addingId != null;

    final showDetect =
        candidate.compatibility == 'possible' ||
        candidate.compatibility == 'incompatible';
    final showAdd = !candidate.isIncompatible;

    final iconBg = candidate.isIncompatible
        ? context.semanticColors.errorContainer
        : context.semanticColors.pluginContainer;
    final iconFg = candidate.isIncompatible
        ? context.semanticColors.onErrorContainer
        : context.semanticColors.onPluginContainer;

    final actionButtons = <Widget>[
      if (showDetect)
        FilledButton.tonalIcon(
          style: FilledButton.styleFrom(
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
          ),
          onPressed: busy ? null : () => _detectCandidate(candidate),
          icon: isDetecting
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.radar, size: 16),
          label: const Text(
            'Detectar sistema',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(fontSize: 12.5, fontWeight: FontWeight.w600),
          ),
        ),
      if (showAdd)
        FilledButton.icon(
          style: FilledButton.styleFrom(
            backgroundColor: context.semanticColors.plugin,
            foregroundColor: context.semanticColors.onPlugin,
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
          ),
          onPressed: busy ? null : () => _addCandidate(candidate),
          icon: isAdding
              ? SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: context.semanticColors.onPlugin,
                  ),
                )
              : const Icon(Icons.add, size: 16),
          label: const Text(
            'Adicionar à frota',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(fontSize: 12.5, fontWeight: FontWeight.w600),
          ),
        ),
    ];

    return NestedPanel(
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: iconBg,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Icon(Icons.router_outlined, size: 20, color: iconFg),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Text(
                      candidate.deviceName,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (ip.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 2),
                      Text(
                        ip,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          fontFamily: 'monospace',
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 8),
              SemanticChip(label: badge.label, tone: badge.tone, compact: true),
            ],
          ),
          const SizedBox(height: 10),
          Text(
            statusMsg,
            style: context.textTheme.bodySmall?.copyWith(
              color: context.colorScheme.onSurfaceVariant,
              height: 1.35,
            ),
          ),
          if (actionButtons.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            SymmetricalActionBar(children: actionButtons),
          ],
        ],
      ),
    );
  }
}

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Renderizador de saída estruturada de plugins (`PluginOutput.vue` + `PluginReport.vue`).
/// Suporta `report`, `table`, `kv`, `json`, `text` e todos os 10 formatos via `plugin_presentation.dart`.
class PluginOutputView extends StatelessWidget {
  const PluginOutputView({super.key, required this.output, this.kind});

  final Object? output;
  final String? kind;

  @override
  Widget build(BuildContext context) {
    if (output == null) {
      return Text(
        'Sem saída registrada.',
        style: context.textTheme.bodySmall?.copyWith(
          color: context.colorScheme.onSurfaceVariant,
        ),
      );
    }

    if (kind == 'text' || output is String) {
      return CodeViewer(
        title: 'Saída',
        code: output.toString(),
        maxHeight: 280,
      );
    }

    if (kind == 'json') {
      final formatted = const JsonEncoder.withIndent('  ').convert(output);
      return CodeViewer(title: 'JSON', code: formatted, maxHeight: 280);
    }

    if (output is List) {
      return _buildTable(context, output as List<dynamic>);
    }

    if (output is Map) {
      final map = Map<String, dynamic>.from(output as Map);
      return _buildReport(context, map);
    }

    return Text(output.toString());
  }

  Widget _buildReport(BuildContext context, Map<String, dynamic> map) {
    final visibleEntries = map.entries
        .where(
          (e) =>
              !e.key.startsWith('_') &&
              e.key != 'settings_patch' &&
              e.key != 'next',
        )
        .toList(growable: false);

    final cardRaw = map['_card'];
    final cardMap = cardRaw is Map ? Map<String, dynamic>.from(cardRaw) : null;

    final scalars = <MapEntry<String, dynamic>>[];
    final tables = <MapEntry<String, List<dynamic>>>[];
    final texts = <MapEntry<String, String>>[];

    for (final entry in visibleEntries) {
      final val = entry.value;
      if (val is List) {
        tables.add(MapEntry<String, List<dynamic>>(entry.key, val));
      } else if (val is String && val.length > 80) {
        texts.add(MapEntry<String, String>(entry.key, val));
      } else if (val is! Map) {
        scalars.add(entry);
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (cardMap != null && cardMap.isNotEmpty) ...<Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: cardMap.entries
                .map((e) {
                  return SemanticChip(
                    label: '${humanizePluginFieldKey(e.key)}: ${e.value}',
                    tone: SemanticTone.plugin,
                    compact: true,
                  );
                })
                .toList(growable: false),
          ),
          const SizedBox(height: 10),
        ],
        if (scalars.isNotEmpty) ...<Widget>[
          NestedPanel(
            child: Column(
              children: scalars
                  .map((e) {
                    final rawStr = e.value?.toString() ?? '—';
                    final isState = e.key == 'status' || e.key == 'state';
                    return KeyValueRow(
                      label: humanizePluginFieldKey(e.key),
                      value: isState ? pluginStateLabel(e.value) : rawStr,
                      trailing: isState
                          ? SemanticChip(
                              label: pluginStateLabel(e.value),
                              tone: pluginStateTone(e.value),
                              compact: true,
                            )
                          : null,
                    );
                  })
                  .toList(growable: false),
            ),
          ),
          const SizedBox(height: 10),
        ],
        for (final tbl in tables) ...<Widget>[
          Text(
            humanizePluginFieldKey(tbl.key),
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 6),
          _buildTable(context, tbl.value),
          const SizedBox(height: 10),
        ],
        for (final txt in texts) ...<Widget>[
          CodeViewer(
            title: humanizePluginFieldKey(txt.key),
            code: txt.value,
            maxHeight: 220,
          ),
          const SizedBox(height: 10),
        ],
      ],
    );
  }

  Widget _buildTable(BuildContext context, List<dynamic> rows) {
    if (rows.isEmpty) {
      return Text(
        'Lista vazia.',
        style: context.textTheme.bodySmall?.copyWith(
          color: context.colorScheme.onSurfaceVariant,
        ),
      );
    }
    if (rows.first is! Map) {
      return CodeViewer(
        title: 'Linhas (${rows.length})',
        code: rows.map((e) => e.toString()).join('\n'),
        maxHeight: 220,
      );
    }
    final maps = rows
        .whereType<Map>()
        .map((m) => Map<String, dynamic>.from(m))
        .toList(growable: false);
    final cols = maps.first.keys
        .where((k) => !k.startsWith('_'))
        .take(8)
        .toList(growable: false);

    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: DataTable(
        columns: cols
            .map((c) => DataColumn(label: Text(humanizePluginFieldKey(c))))
            .toList(growable: false),
        rows: maps
            .map((row) {
              return DataRow(
                cells: cols
                    .map((c) {
                      final val = row[c];
                      if (c == 'status' || c == 'state') {
                        return DataCell(
                          SemanticChip(
                            label: pluginStateLabel(val),
                            tone: pluginStateTone(val),
                            compact: true,
                          ),
                        );
                      }
                      return DataCell(Text(val?.toString() ?? '—'));
                    })
                    .toList(growable: false),
              );
            })
            .toList(growable: false),
      ),
    );
  }
}

/// Linha do tempo de transcrição de uma execução (`PluginTranscript.vue`).
class PluginTranscriptView extends StatelessWidget {
  const PluginTranscriptView({super.key, required this.entries});

  final List<TranscriptEntry> entries;

  @override
  Widget build(BuildContext context) {
    if (entries.isEmpty) {
      return Text(
        'Sem acessos registrados nesta execução.',
        style: context.textTheme.bodySmall?.copyWith(
          color: context.colorScheme.onSurfaceVariant,
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: entries
          .map((entry) {
            final tone = switch (entry.kind) {
              'denied' || 'error' => SemanticTone.error,
              'ssh' || 'telnet' || 'http' => SemanticTone.plugin,
              _ => SemanticTone.info,
            };
            return Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: NestedPanel(
                tone: entry.ok ? null : SemanticTone.error,
                padding: const EdgeInsets.all(10),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Row(
                      children: <Widget>[
                        SemanticChip(
                          label: entry.kind.toUpperCase(),
                          tone: tone,
                          compact: true,
                        ),
                        const Spacer(),
                        Text(
                          formatElapsedMs(entry.durationMs),
                          style: context.textTheme.labelSmall?.copyWith(
                            color: context.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 6),
                    SelectableText(
                      entry.request,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12,
                      ),
                    ),
                    if (entry.response != null &&
                        entry.response!.trim().isNotEmpty) ...<Widget>[
                      const Divider(height: 12),
                      SelectableText(
                        entry.response!,
                        style: TextStyle(
                          fontFamily: 'monospace',
                          fontSize: 11.5,
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            );
          })
          .toList(growable: false),
    );
  }
}

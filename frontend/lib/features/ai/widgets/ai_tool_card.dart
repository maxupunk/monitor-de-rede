import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_tool_chart.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_access_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Renderiza todas as ferramentas de uma mensagem da IA:
/// - Agrupa consultas passivas concluídas em uma única linha expansível ("Consultou: ...").
/// - Renderiza cartões completos para ferramentas ativas, em espera, com gráfico, erro ou `DeviceAccess`.
/// - Renderiza `ask_user` como cartão interativo com botões de opções.
class AiToolActivityList extends StatefulWidget {
  const AiToolActivityList({
    super.key,
    required this.messageId,
    required this.toolCalls,
    required this.isLatestMessage,
    required this.isStreaming,
    required this.onAnswerQuestion,
  });

  final String messageId;
  final List<AiToolCallState> toolCalls;
  final bool isLatestMessage;
  final bool isStreaming;
  final ValueChanged<String> onAnswerQuestion;

  @override
  State<AiToolActivityList> createState() => _AiToolActivityListState();
}

class _AiToolActivityListState extends State<AiToolActivityList> {
  bool _showLookups = false;

  @override
  Widget build(BuildContext context) {
    if (widget.toolCalls.isEmpty) return const SizedBox.shrink();

    final lookups = <AiToolCallState>[];
    final prominent = <AiToolCallState>[];
    for (final tool in widget.toolCalls) {
      if (isPassiveLookupTool(tool)) {
        lookups.add(tool);
      } else {
        prominent.add(tool);
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (lookups.isNotEmpty) ...<Widget>[
          _buildLookupSummary(context, lookups),
          const SizedBox(height: 8),
        ],
        for (final tool in prominent) ...<Widget>[
          if (toolQuestion(tool) != null)
            _AiQuestionCard(
              question: toolQuestion(tool)!,
              enabled: widget.isLatestMessage && !widget.isStreaming,
              onSelect: widget.onAnswerQuestion,
            )
          else
            AiToolCard(messageId: widget.messageId, tool: tool),
          const SizedBox(height: 8),
        ],
      ],
    );
  }

  Widget _buildLookupSummary(
    BuildContext context,
    List<AiToolCallState> lookups,
  ) {
    final counts = <String, int>{};
    for (final item in lookups) {
      final label = toolFriendlyLabel(item.name);
      counts[label] = (counts[label] ?? 0) + 1;
    }
    final summary = counts.entries
        .map((e) => e.value > 1 ? '${e.key} (${e.value}×)' : e.key)
        .join(', ');

    return NestedPanel(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          InkWell(
            onTap: () => setState(() => _showLookups = !_showLookups),
            child: Row(
              children: <Widget>[
                Icon(
                  Icons.manage_search,
                  size: 16,
                  color: context.colorScheme.primary,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Consultou: $summary',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
                Icon(
                  _showLookups ? Icons.expand_less : Icons.expand_more,
                  size: 18,
                  color: context.colorScheme.onSurfaceVariant,
                ),
              ],
            ),
          ),
          if (_showLookups) ...<Widget>[
            const Divider(height: 12),
            for (final tool in lookups) ...<Widget>[
              AiToolCard(
                messageId: widget.messageId,
                tool: tool,
                compact: true,
              ),
              const SizedBox(height: 6),
            ],
          ],
        ],
      ),
    );
  }
}

class _AiQuestionCard extends StatelessWidget {
  const _AiQuestionCard({
    required this.question,
    required this.enabled,
    required this.onSelect,
  });

  final AiQuestion question;
  final bool enabled;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    return NestedPanel(
      tone: SemanticTone.ai,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            question.question,
            style: context.textTheme.bodyMedium?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          if (question.options.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: question.options
                  .map((opt) {
                    return FilledButton.tonal(
                      onPressed: enabled ? () => onSelect(opt) : null,
                      child: Text(opt),
                    );
                  })
                  .toList(growable: false),
            ),
          ],
        ],
      ),
    );
  }
}

/// Cartão individual de ferramenta da IA, com risco, aprovação `DeviceAccess` (ADR 012) e gráfico.
class AiToolCard extends ConsumerStatefulWidget {
  const AiToolCard({
    super.key,
    required this.messageId,
    required this.tool,
    this.compact = false,
  });

  final String messageId;
  final AiToolCallState tool;
  final bool compact;

  @override
  ConsumerState<AiToolCard> createState() => _AiToolCardState();
}

class _AiToolCardState extends ConsumerState<AiToolCard> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final tool = widget.tool;
    final scheme = context.colorScheme;
    final isDevice = isDeviceAccessTool(tool.name);
    final summaryText = tool.summary ?? '';
    final isDeviceWrite =
        isDevice && summaryText.toUpperCase().contains('ALTERA O EQUIPAMENTO');
    final missingAccess = missingAccessIn(tool.result);
    final isAdmin = ref.watch(authProvider).canManagePlugins;

    final statusTone = switch (tool.status) {
      AiToolStatus.running => SemanticTone.primary,
      AiToolStatus.done => SemanticTone.success,
      AiToolStatus.error => SemanticTone.error,
      AiToolStatus.awaiting =>
        isDeviceWrite ? SemanticTone.error : SemanticTone.warning,
      AiToolStatus.cancelled => SemanticTone.secondary,
    };

    final statusLabel = switch (tool.status) {
      AiToolStatus.running => 'Executando...',
      AiToolStatus.done => tool.autoApproved ? 'Auto-aprovado' : 'Concluído',
      AiToolStatus.error => 'Falhou',
      AiToolStatus.awaiting => 'Aguardando aprovação',
      AiToolStatus.cancelled => 'Cancelado',
    };

    final encoder = const JsonEncoder.withIndent('  ');

    return NestedPanel(
      tone: tool.status == AiToolStatus.awaiting ? statusTone : null,
      padding: EdgeInsets.all(widget.compact ? 10 : 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          LayoutBuilder(
            builder: (context, constraints) {
              final stackChips = constraints.maxWidth < 460;
              final leadingWidget = tool.status == AiToolStatus.running
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Icon(
                      isDevice
                          ? Icons.terminal_outlined
                          : Icons.build_circle_outlined,
                      size: 18,
                      color: context.semanticColors
                          .resolve(statusTone, scheme)
                          .foreground,
                    );
              final chips = <Widget>[
                if (tool.risk != null)
                  SemanticChip(
                    label: tool.risk!.label,
                    tone: tool.risk!.tone,
                    icon: tool.risk!.icon,
                    compact: true,
                  ),
                SemanticChip(
                  label: statusLabel,
                  tone: statusTone,
                  compact: true,
                ),
              ];
              final expandBtn = IconButton(
                tooltip: _expanded ? 'Ocultar detalhes' : 'Ver detalhes',
                visualDensity: VisualDensity.compact,
                icon: Icon(
                  _expanded ? Icons.expand_less : Icons.expand_more,
                  size: 18,
                ),
                onPressed: () => setState(() => _expanded = !_expanded),
              );

              if (!stackChips) {
                return Row(
                  children: <Widget>[
                    leadingWidget,
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        toolFriendlyLabel(tool.name),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                    for (final c in chips) ...<Widget>[
                      const SizedBox(width: 6),
                      c,
                    ],
                    expandBtn,
                  ],
                );
              }

              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Row(
                    children: <Widget>[
                      leadingWidget,
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          toolFriendlyLabel(tool.name),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                      expandBtn,
                    ],
                  ),
                  Padding(
                    padding: const EdgeInsets.only(left: 26),
                    child: Wrap(spacing: 6, runSpacing: 6, children: chips),
                  ),
                ],
              );
            },
          ),
          if (summaryText.isNotEmpty) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              summaryText,
              style: context.textTheme.bodySmall?.copyWith(
                fontWeight: FontWeight.w600,
              ),
            ),
          ],
          if (tool.reason != null && tool.reason!.isNotEmpty) ...<Widget>[
            const SizedBox(height: 4),
            Text(
              tool.reason!,
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
          ],
          // Alerta obrigatório de alucinação para ToolKind::DeviceAccess (ADR 012).
          if (isDevice && tool.status == AiToolStatus.awaiting) ...<Widget>[
            const SizedBox(height: 10),
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: isDeviceWrite
                    ? context.semanticColors.error
                    : context.semanticColors.warningContainer,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Icon(
                    isDeviceWrite
                        ? Icons.dangerous_outlined
                        : Icons.warning_amber_outlined,
                    size: 18,
                    color: isDeviceWrite
                        ? context.semanticColors.onError
                        : context.semanticColors.onWarningContainer,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      isDeviceWrite
                          ? 'A IA pode errar (alucinar). Este comando ALTERA a configuração do equipamento — revise antes de aprovar.'
                          : 'Acesso real ao equipamento, só leitura. A IA pode interpretar errado o que ler.',
                      style: context.textTheme.bodySmall?.copyWith(
                        fontWeight: FontWeight.w700,
                        color: isDeviceWrite
                            ? context.semanticColors.onError
                            : context.semanticColors.onWarningContainer,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
          if (tool.status == AiToolStatus.awaiting) ...<Widget>[
            const SizedBox(height: 10),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              alignment: WrapAlignment.end,
              children: <Widget>[
                OutlinedButton(
                  onPressed: () => ref
                      .read(aiChatProvider.notifier)
                      .cancelTool(widget.messageId, tool.id),
                  child: const Text('Cancelar'),
                ),
                FilledButton.icon(
                  style: isDeviceWrite
                      ? FilledButton.styleFrom(
                          backgroundColor: context.semanticColors.error,
                          foregroundColor: context.semanticColors.onError,
                        )
                      : null,
                  onPressed: () => ref
                      .read(aiChatProvider.notifier)
                      .confirmTool(widget.messageId, tool.id),
                  icon: const Icon(Icons.check, size: 18),
                  label: const Text('Confirmar'),
                ),
              ],
            ),
          ],
          if (missingAccess != null && isAdmin) ...<Widget>[
            const SizedBox(height: 10),
            Align(
              alignment: Alignment.centerLeft,
              child: FilledButton.tonalIcon(
                onPressed: () async {
                  final saved = await showDeviceAccessDialog(
                    context: context,
                    ref: ref,
                    deviceId: missingAccess.deviceId,
                    lockedTransport: missingAccess.transport,
                    reason:
                        'Cadastre a credencial ${missingAccess.transport.toUpperCase()} para a IA continuar.',
                  );
                  if (saved && mounted) {
                    await ref
                        .read(aiChatProvider.notifier)
                        .continueAfterAccess(
                          transport: missingAccess.transport,
                          deviceName: 'dispositivo #${missingAccess.deviceId}',
                        );
                  }
                },
                icon: const Icon(Icons.vpn_key_outlined, size: 18),
                label: Text(
                  'Cadastrar acesso ${missingAccess.transport.toUpperCase()}',
                ),
              ),
            ),
          ],
          if (tool.chart != null) ...<Widget>[
            const SizedBox(height: 10),
            AiToolChart(chart: tool.chart!),
          ],
          if (_expanded) ...<Widget>[
            const SizedBox(height: 10),
            if (tool.arguments.isNotEmpty) ...<Widget>[
              CodeViewer(
                title: 'Argumentos',
                code: encoder.convert(tool.arguments),
                maxHeight: 160,
              ),
              const SizedBox(height: 8),
            ],
            if (tool.result != null)
              CodeViewer(
                title: 'Resultado',
                code: encoder.convert(tool.result),
                maxHeight: 200,
              ),
          ],
        ],
      ),
    );
  }
}

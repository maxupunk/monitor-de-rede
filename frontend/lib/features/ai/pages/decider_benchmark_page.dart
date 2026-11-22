import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Página `/ai-benchmark` (`area: ProductArea.core`, `?tab=chat|decider`).
class DeciderBenchmarkPage extends ConsumerStatefulWidget {
  const DeciderBenchmarkPage({super.key, this.initialTab});

  final String? initialTab;

  @override
  ConsumerState<DeciderBenchmarkPage> createState() =>
      _DeciderBenchmarkPageState();
}

class _DeciderBenchmarkPageState extends ConsumerState<DeciderBenchmarkPage> {
  late String _tab;
  String _chatSuite = 'blind';

  @override
  void initState() {
    super.initState();
    _tab = resolveBenchmarkTab(widget.initialTab);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(aiBenchmarkProvider.notifier).loadChat();
      ref.read(aiBenchmarkProvider.notifier).loadDecider();
      ref.read(aiSettingsProvider.notifier).load();
      ref.read(aiSettingsProvider.notifier).loadOllamaModels();
      ref.read(aiDeciderProvider.notifier).loadAll();
    });
  }

  @override
  void didUpdateWidget(covariant DeciderBenchmarkPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.initialTab != widget.initialTab) {
      setState(() => _tab = resolveBenchmarkTab(widget.initialTab));
    }
  }

  void _switchTab(String next) {
    setState(() => _tab = next);
    context.go('/ai-benchmark?tab=$next');
  }

  @override
  Widget build(BuildContext context) {
    final benchmark = ref.watch(aiBenchmarkProvider);

    final isCompact = MediaQuery.sizeOf(context).width < 680;

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (isCompact) ...<Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Benchmark de Modelos IA',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.titleLarge?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
                IconButton(
                  tooltip: 'Configurações IA',
                  onPressed: () => context.go('/settings?tab=ia'),
                  icon: const Icon(Icons.settings_outlined, size: 20),
                ),
              ],
            ),
            const SizedBox(height: 8),
            SizedBox(
              width: double.infinity,
              child: SegmentedButton<String>(
                showSelectedIcon: false,
                selected: <String>{_tab},
                onSelectionChanged: (sel) => _switchTab(sel.first),
                segments: const <ButtonSegment<String>>[
                  ButtonSegment<String>(
                    value: 'chat',
                    icon: Icon(Icons.chat_outlined, size: 17),
                    label: Text(
                      'Chat (Ollama)',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  ButtonSegment<String>(
                    value: 'decider',
                    icon: Icon(Icons.bolt_outlined, size: 17),
                    label: Text(
                      'Decisor (Ollaya)',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
            ),
          ] else
            Wrap(
              spacing: 12,
              runSpacing: 10,
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      'Benchmark de Modelos IA',
                      style: context.textTheme.headlineSmall?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                    Text(
                      'Resultados medidos nas tarefas reais do NetMonitor (Chat Ollama & Decisor Ollaya)',
                      style: context.textTheme.bodySmall?.copyWith(
                        color: context.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    SegmentedButton<String>(
                      showSelectedIcon: false,
                      selected: <String>{_tab},
                      onSelectionChanged: (sel) => _switchTab(sel.first),
                      segments: const <ButtonSegment<String>>[
                        ButtonSegment<String>(
                          value: 'chat',
                          icon: Icon(Icons.chat_outlined, size: 17),
                          label: Text(
                            'Chat (Ollama)',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        ButtonSegment<String>(
                          value: 'decider',
                          icon: Icon(Icons.bolt_outlined, size: 17),
                          label: Text(
                            'Decisor (Ollaya)',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                    OutlinedButton.icon(
                      onPressed: () => context.go('/settings?tab=ia'),
                      icon: const Icon(Icons.settings_outlined, size: 18),
                      label: const Text('Configurações IA'),
                    ),
                  ],
                ),
              ],
            ),
          const SizedBox(height: 16),
          if (_tab == 'chat')
            _buildChatSection(context, benchmark)
          else
            _buildDeciderSection(context, benchmark),
        ],
      ),
    );
  }

  Widget _buildChatSection(BuildContext context, AiBenchmarkState benchmark) {
    if (benchmark.loadingChat && benchmark.chatReport == null) {
      return const LoadingStateView(message: 'Carregando benchmark do Chat...');
    }
    if (benchmark.chatError != null && benchmark.chatReport == null) {
      return ErrorStateView(
        message: benchmark.chatError!,
        onRetry: () =>
            ref.read(aiBenchmarkProvider.notifier).loadChat(force: true),
      );
    }
    final report = benchmark.chatReport;
    if (report == null) {
      return const EmptyStateView(
        title: 'Sem relatório de benchmark do Chat',
        icon: Icons.speed_outlined,
      );
    }

    final activeModel = ref.watch(aiSettingsProvider).settings?.ollamaModel;
    final installedIds =
        (ref.watch(aiSettingsProvider).ollamaCatalog?.models ??
                const <AiProviderModelItem>[])
            .where((m) => m.installed)
            .map((m) => m.id)
            .toList(growable: false);

    final sorted = List<ChatBenchmarkRow>.from(report.models)
      ..sort((a, b) {
        if (a.recommended != b.recommended) return a.recommended ? -1 : 1;
        if (a.bundled != b.bundled) return a.bundled ? -1 : 1;
        final sa = chatActiveSplit(a, _chatSuite);
        final sb = chatActiveSplit(b, _chatSuite);
        final scoreCmp = (sb?.overall ?? -1).compareTo(sa?.overall ?? -1);
        if (scoreCmp != 0) return scoreCmp;
        return (sa?.turnP95Ms ?? 999999).compareTo(sb?.turnP95Ms ?? 999999);
      });

    final bundled = sorted.where((r) => r.bundled).take(3).toList();
    final skippedInBlind = _chatSuite == 'blind'
        ? sorted.where((r) => r.blind == null).length
        : 0;

    final isCompact = MediaQuery.sizeOf(context).width < 680;
    final suiteSwitcher = SegmentedButton<String>(
      showSelectedIcon: false,
      selected: <String>{_chatSuite},
      onSelectionChanged: (sel) => setState(() => _chatSuite = sel.first),
      segments: <ButtonSegment<String>>[
        ButtonSegment<String>(
          value: 'blind',
          label: Text(
            'Cenários NOC (${report.blindCases})',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
        ButtonSegment<String>(
          value: 'bench',
          label: Text(
            'Suíte Oficial (${report.benchCases})',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SectionCard(
          title: 'Suíte e portão de qualidade',
          subtitle:
              'Medido em ${report.measuredOn} · ${report.hardware} · num_ctx ${formatContextWindow(report.numCtx)}',
          icon: Icons.verified_outlined,
          trailing: isCompact ? null : suiteSwitcher,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (isCompact) ...<Widget>[
                suiteSwitcher,
                const SizedBox(height: 10),
              ],
              Wrap(
                spacing: 8,
                runSpacing: 6,
                children: <Widget>[
                  SemanticChip(
                    label:
                        'Nota ≥ ${formatPercent(report.minScore * 100, digits: 0)}',
                    tone: SemanticTone.primary,
                    compact: true,
                  ),
                  SemanticChip(
                    label:
                        'Ferramentas ≥ ${formatPercent(report.minTools * 100, digits: 0)}',
                    tone: SemanticTone.info,
                    compact: true,
                  ),
                  SemanticChip(
                    label:
                        'Segurança ≥ ${formatPercent(report.minSafety * 100, digits: 0)}',
                    tone: SemanticTone.success,
                    compact: true,
                  ),
                  SemanticChip(
                    label: 'p95 ≤ ${formatElapsedMs(report.timeoutMs)}',
                    tone: SemanticTone.warning,
                    compact: true,
                  ),
                ],
              ),
            ],
          ),
        ),
        if (bundled.isNotEmpty) ...<Widget>[
          const SizedBox(height: 16),
          ResponsiveGrid(
            xs: 1,
            sm: 2,
            md: 3,
            lg: 3,
            xl: 3,
            children: bundled
                .map((row) {
                  final split = chatActiveSplit(row, _chatSuite) ?? row.bench;
                  final inUse =
                      activeModel != null &&
                      isModelInstalled(<String>[activeModel], row.model);
                  final isInstalled = isModelInstalled(installedIds, row.model);
                  return SectionCard(
                    title: row.label ?? row.model,
                    subtitle: row.quantization ?? 'NetMonitor',
                    icon: row.recommended
                        ? Icons.star_outlined
                        : Icons.auto_awesome,
                    iconTone: row.recommended
                        ? SemanticTone.success
                        : SemanticTone.ai,
                    trailing: inUse
                        ? const SemanticChip(
                            label: 'Em uso no Chat',
                            tone: SemanticTone.success,
                            compact: true,
                          )
                        : (isInstalled
                              ? const SemanticChip(
                                  label: 'Instalado',
                                  tone: SemanticTone.info,
                                  compact: true,
                                )
                              : null),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        KeyValueRow(
                          label: 'Nota geral',
                          value: formatPercent(split.overall * 100, digits: 1),
                        ),
                        KeyValueRow(
                          label: 'Ferramentas / Segurança',
                          value:
                              '${formatPercent(split.tools * 100, digits: 0)} / ${formatPercent(split.safety * 100, digits: 0)}',
                        ),
                        KeyValueRow(
                          label: 'Velocidade / p95',
                          value:
                              '${formatTokenRate(split.tokensPerSec)} · ${formatElapsedMs(split.turnP95Ms)}',
                        ),
                        if (row.downloadMb != null)
                          KeyValueRow(
                            label: 'Download',
                            value: formatDecimalBytes(
                              row.downloadMb! * 1000000,
                            ),
                          ),
                      ],
                    ),
                  );
                })
                .toList(growable: false),
          ),
        ],
        if (skippedInBlind > 0) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.info,
            child: Wrap(
              spacing: 8,
              runSpacing: 6,
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                Text(
                  '$skippedInBlind modelo(s) reprovado(s) na Suíte Oficial não foram executados nos Cenários NOC.',
                ),
                TextButton(
                  onPressed: () => setState(() => _chatSuite = 'bench'),
                  child: const Text('Ver Suíte Oficial'),
                ),
              ],
            ),
          ),
        ],
        const SizedBox(height: 16),
        SectionCard(
          title: 'Comparativo completo',
          icon: Icons.table_chart_outlined,
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: DataTable(
              dataRowMinHeight: 52,
              dataRowMaxHeight: double.infinity,
              columns: <DataColumn>[
                const DataColumn(label: Text('#')),
                const DataColumn(label: Text('Modelo')),
                const DataColumn(label: Text('Portão')),
                const DataColumn(label: Text('Nota geral')),
                const DataColumn(label: Text('Ferramentas')),
                const DataColumn(label: Text('Resposta')),
                const DataColumn(label: Text('Segurança')),
                for (final domain in report.domains)
                  DataColumn(label: Text(domain.label)),
                const DataColumn(label: Text('p50 / p95 · tok/s')),
              ],
              rows: <DataRow>[
                for (var i = 0; i < sorted.length; i++)
                  _buildChatRow(i, sorted[i], report, activeModel),
              ],
            ),
          ),
        ),
        const SizedBox(height: 16),
        const SectionCard(
          title: 'Como ler este benchmark',
          icon: Icons.help_outline,
          child: Text(
            'A nota geral pondera Ferramentas (50%), Qualidade de Resposta (30%) e Segurança (20%). '
            'Modelos aprovados no portão atendem simultaneamente todos os pisos mínimos e o limite de latência p95.',
          ),
        ),
      ],
    );
  }

  DataRow _buildChatRow(
    int index,
    ChatBenchmarkRow row,
    ChatBenchmarkReport report,
    String? activeModel,
  ) {
    final split = chatActiveSplit(row, _chatSuite);
    final verdict = split != null ? chatVerdictFact(row, split, report) : null;
    final inUse =
        activeModel != null &&
        isModelInstalled(<String>[activeModel], row.model);

    return DataRow(
      cells: <DataCell>[
        DataCell(Text('${index + 1}')),
        DataCell(
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              Text(
                row.label ?? row.model,
                style: const TextStyle(fontWeight: FontWeight.w700),
              ),
              Wrap(
                spacing: 4,
                children: <Widget>[
                  if (row.recommended)
                    const SemanticChip(
                      label: 'Recomendado',
                      tone: SemanticTone.success,
                      compact: true,
                    ),
                  if (row.bundled)
                    const SemanticChip(
                      label: 'Treinado NetMonitor',
                      tone: SemanticTone.ai,
                      compact: true,
                    ),
                  if (inUse)
                    const SemanticChip(
                      label: 'Em uso',
                      tone: SemanticTone.primary,
                      compact: true,
                    ),
                ],
              ),
            ],
          ),
        ),
        DataCell(
          verdict == null
              ? const Text('—')
              : Tooltip(
                  message: verdict.reason,
                  child: SemanticChip(
                    label: verdict.label,
                    tone: verdict.tone,
                    compact: true,
                  ),
                ),
        ),
        DataCell(
          split == null
              ? const Text('—')
              : Text(
                  '${formatPercent(split.overall * 100, digits: 1)} (${formatPercent(split.ciLow * 100, digits: 0)}–${formatPercent(split.ciHigh * 100, digits: 0)})',
                  style: const TextStyle(fontWeight: FontWeight.w700),
                ),
        ),
        DataCell(
          Text(
            split != null ? formatPercent(split.tools * 100, digits: 0) : '—',
          ),
        ),
        DataCell(
          Text(
            split != null
                ? formatPercent(split.response * 100, digits: 0)
                : '—',
          ),
        ),
        DataCell(
          Text(
            split != null ? formatPercent(split.safety * 100, digits: 0) : '—',
          ),
        ),
        for (final domain in report.domains)
          DataCell(
            Builder(
              builder: (_) {
                final s = chatDomainScoreOf(split, domain.id);
                if (s == null) return const Text('—');
                return SemanticChip(
                  label: formatPercent(s * 100, digits: 0),
                  tone: accuracyTone(s),
                  compact: true,
                );
              },
            ),
          ),
        DataCell(
          Text(
            split == null
                ? '—'
                : '${formatElapsedMs(split.turnP50Ms)} / ${formatElapsedMs(split.turnP95Ms)} · ${formatTokenRate(split.tokensPerSec)}',
          ),
        ),
      ],
    );
  }

  Widget _buildDeciderSection(
    BuildContext context,
    AiBenchmarkState benchmark,
  ) {
    if (benchmark.loadingDecider && benchmark.deciderReport == null) {
      return const LoadingStateView(
        message: 'Carregando benchmark do Decisor...',
      );
    }
    if (benchmark.deciderError != null && benchmark.deciderReport == null) {
      return ErrorStateView(
        message: benchmark.deciderError!,
        onRetry: () =>
            ref.read(aiBenchmarkProvider.notifier).loadDecider(force: true),
      );
    }
    final report = benchmark.deciderReport;
    if (report == null) {
      return const EmptyStateView(
        title: 'Sem relatório de benchmark do Decisor',
        icon: Icons.bolt_outlined,
      );
    }

    final recommended =
        report.models.where((m) => m.recommended).firstOrNull ??
        (report.models.isNotEmpty ? report.models.first : null);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (recommended != null) ...<Widget>[
          SectionCard(
            title: 'Recomendado · ${recommended.model}',
            subtitle:
                'Medido em ${report.measuredOn} · ${report.hardware} · ${report.cases} casos',
            icon: Icons.star_outlined,
            iconTone: SemanticTone.success,
            child: ResponsiveGrid(
              xs: 1,
              sm: 3,
              md: 3,
              lg: 3,
              xl: 3,
              children: <Widget>[
                KpiCard(
                  label: 'Acerto geral',
                  value: formatPercent(recommended.overall * 100, digits: 1),
                  subtitle:
                      'IC 90%: ${formatPercent(recommended.ciLow * 100, digits: 0)}–${formatPercent(recommended.ciHigh * 100, digits: 0)}',
                  icon: Icons.check_circle_outline,
                  tone: accuracyTone(recommended.overall),
                ),
                KpiCard(
                  label: 'Latência CPU (p50)',
                  value: formatElapsedMs(recommended.p50Ms),
                  subtitle: 'p95: ${formatElapsedMs(recommended.p95Ms)}',
                  icon: Icons.timer_outlined,
                  tone: SemanticTone.info,
                ),
                KpiCard(
                  label: 'Limite de decisão',
                  value: formatElapsedMs(report.timeoutMs),
                  subtitle: recommended.latencyFrom != null
                      ? 'Medido em ${recommended.latencyFrom}'
                      : 'Execução local em CPU',
                  icon: Icons.speed,
                  tone: SemanticTone.primary,
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
        ],
        SectionCard(
          title: 'Desempenho por tarefa do Decisor',
          icon: Icons.table_chart_outlined,
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: DataTable(
              dataRowMinHeight: 52,
              dataRowMaxHeight: double.infinity,
              columns: <DataColumn>[
                const DataColumn(label: Text('Modelo')),
                const DataColumn(label: Text('Geral')),
                for (final task in report.tasks)
                  DataColumn(label: Text(task.label)),
                const DataColumn(label: Text('Latência CPU (p50 / p95)')),
              ],
              rows: report.models
                  .map((row) {
                    return DataRow(
                      cells: <DataCell>[
                        DataCell(
                          Row(
                            mainAxisSize: MainAxisSize.min,
                            children: <Widget>[
                              Text(
                                row.model,
                                style: const TextStyle(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              if (row.recommended) ...<Widget>[
                                const SizedBox(width: 6),
                                const SemanticChip(
                                  label: 'Recomendado',
                                  tone: SemanticTone.success,
                                  compact: true,
                                ),
                              ],
                              if (row.bundled) ...<Widget>[
                                const SizedBox(width: 6),
                                const SemanticChip(
                                  label: 'NetMonitor',
                                  tone: SemanticTone.info,
                                  compact: true,
                                ),
                              ],
                            ],
                          ),
                        ),
                        DataCell(
                          SemanticChip(
                            label: formatPercent(row.overall * 100, digits: 0),
                            tone: accuracyTone(row.overall),
                            compact: true,
                          ),
                        ),
                        for (final task in report.tasks)
                          DataCell(
                            Builder(
                              builder: (_) {
                                final score = deciderTaskScoreOf(row, task.id);
                                return SemanticChip(
                                  label: formatPercent(score * 100, digits: 0),
                                  tone: accuracyTone(score),
                                  compact: true,
                                );
                              },
                            ),
                          ),
                        DataCell(
                          Row(
                            mainAxisSize: MainAxisSize.min,
                            children: <Widget>[
                              Text(
                                '${formatElapsedMs(row.p50Ms)} / ${formatElapsedMs(row.p95Ms)}',
                              ),
                              if (row.overTimeout > 0.01) ...<Widget>[
                                const SizedBox(width: 6),
                                SemanticChip(
                                  label:
                                      '${formatPercent(row.overTimeout * 100, digits: 0)} timeout',
                                  tone: SemanticTone.warning,
                                  compact: true,
                                ),
                              ],
                            ],
                          ),
                        ),
                      ],
                    );
                  })
                  .toList(growable: false),
            ),
          ),
        ),
        const SizedBox(height: 16),
        const SectionCard(
          title: 'Como ler o Decisor',
          icon: Icons.help_outline,
          child: Text(
            'O Decisor classifica intenções e seleciona grupos de ferramentas antes do chat principal. '
            'Notas ≥ 85% (verde) indicam alta precisão; entre 60% e 84% (azul) são utilizáveis; abaixo de 60% (vermelho) ficam próximas do chute.',
          ),
        ),
      ],
    );
  }
}

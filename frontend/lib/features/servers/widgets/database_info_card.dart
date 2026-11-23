import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Cartão de Configurações (`Sistema` — tab `'sistema'`, order 20, area `core`)
/// exibindo tamanho do banco interno, período do histórico, uso por tabela e ações de limpeza.
class DatabaseInfoCard extends ConsumerStatefulWidget {
  const DatabaseInfoCard({super.key});

  @override
  ConsumerState<DatabaseInfoCard> createState() => _DatabaseInfoCardState();
}

class _DatabaseInfoCardState extends ConsumerState<DatabaseInfoCard> {
  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(systemDatabaseProvider.notifier).fetchInfo(),
    );
  }

  Future<void> _openUsageDialog() async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Espaço ocupado pelo banco de dados',
      subtitle: 'Medido sob demanda por tabela e área',
      maxWidth: 780,
      builder: (_) =>
          _DatabaseUsageDialogBody(onClearHistory: _confirmClearHistory),
    );
  }

  Future<void> _confirmClearHistory() async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Apagar histórico de métricas e logs?',
      message:
          'Remove resultados de monitores, métricas, logs de syslog e alertas antigos. '
          'Dispositivos, monitores, redes e usuários permanecem intactos.',
      confirmLabel: 'Apagar histórico',
      tone: SemanticTone.warning,
    );
    if (!confirmed || !mounted) return;
    try {
      final stats = await ref
          .read(systemDatabaseProvider.notifier)
          .clearHistory();
      if (!mounted) return;
      final total = (stats['totalDeleted'] as num?)?.toInt() ?? 0;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Histórico limpo: ${formatInteger(total)} registros removidos.',
          ),
        ),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Erro ao limpar histórico: ${describeError(err)}'),
        ),
      );
    }
  }

  Future<void> _confirmClearAllItems() async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Apagar todos os itens do inventário?',
      message:
          'Remove dispositivos, monitores e redes cadastradas, recria o servidor local '
          'e o monitor de saúde. Sua conta de administrador e sessão são preservadas.',
      confirmLabel: 'Apagar todos os itens',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    try {
      final stats = await ref
          .read(systemDatabaseProvider.notifier)
          .clearAllItems();
      if (!mounted) return;
      final total = (stats['totalDeleted'] as num?)?.toInt() ?? 0;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Inventário redefinido: ${formatInteger(total)} itens removidos.',
          ),
        ),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao apagar itens: ${describeError(err)}')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final sysDb = ref.watch(systemDatabaseProvider);
    final isAdmin = ref.watch(authProvider).isAdmin;
    final info = sysDb.info;

    return SectionCard(
      title: 'Banco de dados do NetMonitor',
      subtitle: 'Armazenamento interno de métricas, eventos e inventário',
      icon: Icons.storage_outlined,
      iconTone: SemanticTone.info,
      trailing: IconButton(
        tooltip: 'Atualizar',
        onPressed: sysDb.loadingInfo
            ? null
            : () => ref.read(systemDatabaseProvider.notifier).fetchInfo(),
        icon: const Icon(Icons.refresh),
      ),
      footer: isAdmin
          ? Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                OutlinedButton.icon(
                  onPressed: sysDb.actionLoading ? null : _confirmClearHistory,
                  icon: const Icon(Icons.cleaning_services_outlined, size: 16),
                  label: const Text('Apagar histórico'),
                ),
                TextButton.icon(
                  onPressed: sysDb.actionLoading ? null : _confirmClearAllItems,
                  icon: const Icon(Icons.delete_forever_outlined, size: 16),
                  label: const Text('Apagar todos os itens'),
                ),
              ],
            )
          : null,
      child: sysDb.loadingInfo && info == null
          ? const LoadingStateView(message: 'Consultando banco de dados...')
          : info == null
          ? ErrorStateView(
              message:
                  sysDb.error ??
                  'Não foi possível ler as informações do banco.',
              onRetry: () =>
                  ref.read(systemDatabaseProvider.notifier).fetchInfo(),
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                ChoiceCard(
                  title: 'Tamanho total: ${formatBytes(info.sizeBytes)}',
                  subtitle:
                      'Toque para inspecionar o consumo por tabela e por área',
                  icon: Icons.pie_chart_outline,
                  tone: SemanticTone.primary,
                  onTap: _openUsageDialog,
                ),
                const SizedBox(height: 12),
                ResponsiveGrid(
                  xs: 2,
                  sm: 2,
                  md: 4,
                  lg: 4,
                  xl: 4,
                  children: <Widget>[
                    NestedPanel(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text('Tipo', style: context.textTheme.labelSmall),
                          const SizedBox(height: 4),
                          Text(
                            info.displayType,
                            style: context.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w800,
                            ),
                          ),
                        ],
                      ),
                    ),
                    NestedPanel(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'Total de histórico',
                            style: context.textTheme.labelSmall,
                          ),
                          const SizedBox(height: 4),
                          Text(
                            formatTimeSpan(
                              info.earliestRecord,
                              info.latestRecord,
                            ),
                            style: context.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w800,
                            ),
                          ),
                        ],
                      ),
                    ),
                    NestedPanel(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'Primeiro registro',
                            style: context.textTheme.labelSmall,
                          ),
                          const SizedBox(height: 4),
                          Text(
                            formatDateTime(info.earliestRecord),
                            style: context.textTheme.bodySmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ],
                      ),
                    ),
                    NestedPanel(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'Último registro',
                            style: context.textTheme.labelSmall,
                          ),
                          const SizedBox(height: 4),
                          Text(
                            formatDateTime(info.latestRecord),
                            style: context.textTheme.bodySmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ],
            ),
    );
  }
}

class _DatabaseUsageDialogBody extends ConsumerStatefulWidget {
  const _DatabaseUsageDialogBody({required this.onClearHistory});
  final VoidCallback onClearHistory;

  @override
  ConsumerState<_DatabaseUsageDialogBody> createState() =>
      _DatabaseUsageDialogBodyState();
}

class _DatabaseUsageDialogBodyState
    extends ConsumerState<_DatabaseUsageDialogBody> {
  bool _groupByArea = true;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(systemDatabaseProvider.notifier).fetchUsage(),
    );
  }

  SemanticTone _areaTone(String area) => switch (area) {
    'core' => SemanticTone.secondary,
    'network' => SemanticTone.primary,
    'servers' => SemanticTone.info,
    'vpn' => SemanticTone.ai,
    _ => SemanticTone.secondary,
  };

  @override
  Widget build(BuildContext context) {
    final sysDb = ref.watch(systemDatabaseProvider);
    final usage = sysDb.usage;

    if (sysDb.loadingUsage && usage == null) {
      return const LoadingStateView(message: 'Medindo espaço por tabela...');
    }
    if (usage == null) {
      return ErrorStateView(
        message: sysDb.error ?? 'Falha ao medir o uso do banco.',
        onRetry: () => ref.read(systemDatabaseProvider.notifier).fetchUsage(),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Wrap(
              spacing: 8,
              children: <Widget>[
                SemanticChip(
                  label: 'Total: ${formatBytes(usage.totalBytes)}',
                  tone: SemanticTone.primary,
                ),
                SemanticChip(
                  label: usage.dbType.toUpperCase(),
                  tone: SemanticTone.info,
                ),
              ],
            ),
            Wrap(
              spacing: 8,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                FilterChip(
                  selected: _groupByArea,
                  label: const Text('Agrupar por área'),
                  onSelected: (v) => setState(() => _groupByArea = v),
                ),
                OutlinedButton.icon(
                  onPressed: sysDb.loadingUsage
                      ? null
                      : () => ref
                            .read(systemDatabaseProvider.notifier)
                            .fetchUsage(),
                  icon: const Icon(Icons.refresh, size: 16),
                  label: const Text('Medir de novo'),
                ),
              ],
            ),
          ],
        ),
        const SizedBox(height: 14),
        ...usage.databases.map((section) => _buildSection(context, section)),
        const SizedBox(height: 12),
        if (ref.watch(authProvider).isAdmin)
          Align(
            alignment: Alignment.centerRight,
            child: OutlinedButton.icon(
              onPressed: () {
                Navigator.of(context).pop();
                widget.onClearHistory();
              },
              icon: const Icon(Icons.cleaning_services_outlined, size: 16),
              label: const Text('Apagar histórico…'),
            ),
          ),
      ],
    );
  }

  Widget _buildSection(BuildContext context, DatabaseUsageSection section) {
    final totalRows = section.tables.fold<int>(0, (acc, t) => acc + t.rows);

    double shareFor(TableUsage t) {
      if (section.detailAvailable && section.sizeBytes > 0) {
        return (t.totalBytes / section.sizeBytes) * 100;
      }
      if (totalRows > 0) return (t.rows / totalRows) * 100;
      return 0;
    }

    Widget renderTableRow(TableUsage t) {
      final share = shareFor(t).clamp(0.0, 100.0);
      return Padding(
        padding: const EdgeInsets.only(bottom: 10),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Wrap(
                    spacing: 6,
                    runSpacing: 4,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      Text(
                        t.label,
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                      Text(
                        t.table,
                        style: context.textTheme.bodySmall?.copyWith(
                          fontFamily: 'monospace',
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                      if (!_groupByArea)
                        SemanticChip(
                          label: t.areaTitle,
                          tone: _areaTone(t.area),
                          compact: true,
                        ),
                      if (t.retentionDays != null)
                        SemanticChip(
                          label: 'guarda ${t.retentionDays}d',
                          tone: SemanticTone.secondary,
                          compact: true,
                        ),
                    ],
                  ),
                ),
                Text(
                  section.detailAvailable
                      ? '${formatBytes(t.totalBytes)} (${formatPercent(share)})'
                      : '${t.rowsEstimated ? '≈ ' : ''}${formatInteger(t.rows)} linhas (${formatPercent(share)})',
                  style: context.textTheme.bodySmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 4),
            ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: LinearProgressIndicator(value: share / 100, minHeight: 6),
            ),
          ],
        ),
      );
    }

    final grouped = <String, List<TableUsage>>{};
    for (final t in section.tables) {
      grouped.putIfAbsent(t.areaTitle, () => <TableUsage>[]).add(t);
    }

    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: NestedPanel(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Text(
                  '${section.title} (${section.name})',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
                Text(
                  formatBytes(section.sizeBytes),
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
            if (!section.detailAvailable) ...<Widget>[
              const SizedBox(height: 8),
              const NestedPanel(
                tone: SemanticTone.info,
                child: Text(
                  'SQLite sem extensão dbstat: a proporção abaixo estima o espaço pela contagem de linhas.',
                ),
              ),
            ],
            const SizedBox(height: 12),
            if (_groupByArea)
              ...grouped.entries.expand(
                (entry) => <Widget>[
                  Padding(
                    padding: const EdgeInsets.only(bottom: 6, top: 4),
                    child: Text(
                      entry.key,
                      style: context.textTheme.labelLarge?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                  ),
                  ...entry.value.map(renderTableRow),
                ],
              )
            else
              ...section.tables.map(renderTableRow),
            if (section.otherBytes > 0)
              Text(
                'Metadados / espaço livre reservado: ${formatBytes(section.otherBytes)}',
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.colorScheme.onSurfaceVariant,
                ),
              ),
          ],
        ),
      ),
    );
  }
}

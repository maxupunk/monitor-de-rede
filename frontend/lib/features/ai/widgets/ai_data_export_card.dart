import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/vpn/utils/file_export_helper.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Cartão de exportação de dados do Assistente IA (`AiDataExportCard.vue`) — somente Admin.
class AiDataExportCard extends ConsumerStatefulWidget {
  const AiDataExportCard({super.key});

  @override
  ConsumerState<AiDataExportCard> createState() => _AiDataExportCardState();
}

class _AiDataExportCardState extends ConsumerState<AiDataExportCard> {
  int? _days = 30;
  AiDataExportSummary? _summary;
  bool _loading = false;
  bool _downloading = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _loadSummary());
  }

  Future<void> _loadSummary() async {
    if (!ref.read(authProvider).isAdmin) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final summary = await ref
          .read(aiRepositoryProvider)
          .fetchExportSummary(days: _days);
      if (mounted) setState(() => _summary = summary);
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _download() async {
    setState(() {
      _downloading = true;
      _error = null;
    });
    try {
      final result = await ref
          .read(aiRepositoryProvider)
          .downloadExportZip(days: _days);
      final now = DateTime.now();
      final fallbackName =
          'netmonitor-ia-dados-${now.year}${now.month.toString().padLeft(2, '0')}${now.day.toString().padLeft(2, '0')}.zip';
      await exportFileBytes(
        bytes: result.bytes,
        fileName: result.filename ?? fallbackName,
        mimeType: 'application/zip',
      );
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Pacote ZIP de dados da IA exportado.')),
      );
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _downloading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isAdmin = ref.watch(authProvider).isAdmin;
    if (!isAdmin) return const SizedBox.shrink();

    return SectionCard(
      title: 'Exportar dados do Assistente',
      subtitle:
          'Gera um pacote ZIP com conversas, avaliações e execuções para auditoria ou ajuste fino',
      icon: Icons.archive_outlined,
      iconTone: SemanticTone.ai,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 6,
            children: <Widget>[
              for (final entry in const <(int?, String)>[
                (7, '7 dias'),
                (30, '30 dias'),
                (90, '90 dias'),
                (null, 'Tudo'),
              ])
                ChoiceChip(
                  label: Text(entry.$2),
                  selected: _days == entry.$1,
                  onSelected: (_) {
                    setState(() => _days = entry.$1);
                    _loadSummary();
                  },
                ),
            ],
          ),
          const SizedBox(height: 12),
          if (_loading)
            const LoadingStateView(message: 'Calculando volume de dados...')
          else if (_error != null)
            NestedPanel(tone: SemanticTone.error, child: Text(_error!))
          else if (_summary != null) ...<Widget>[
            ResponsiveGrid(
              xs: 2,
              sm: 4,
              md: 4,
              lg: 4,
              xl: 4,
              children: <Widget>[
                KpiCard(
                  label: 'Conversas',
                  value: formatInteger(_summary!.conversations),
                  icon: Icons.forum_outlined,
                  tone: SemanticTone.ai,
                ),
                KpiCard(
                  label: 'Mensagens',
                  value: formatInteger(_summary!.messages),
                  icon: Icons.chat_outlined,
                  tone: SemanticTone.primary,
                ),
                KpiCard(
                  label: 'Pessoas',
                  value: formatInteger(_summary!.users),
                  icon: Icons.people_outline,
                  tone: SemanticTone.info,
                ),
                KpiCard(
                  label: 'Comandos',
                  value: formatInteger(_summary!.deviceRuns),
                  icon: Icons.terminal_outlined,
                  tone: SemanticTone.plugin,
                ),
              ],
            ),
            if (_summary!.oldest != null ||
                _summary!.newest != null) ...<Widget>[
              const SizedBox(height: 8),
              Text(
                'Período coberto: ${formatDate(_summary!.oldest)} até ${formatDate(_summary!.newest)}',
                style: context.textTheme.bodySmall?.copyWith(
                  color: context.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ],
          const SizedBox(height: 12),
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Atenção: revise o pacote antes de compartilhar externamente. '
              'As conversas podem conter IPs internos, nomes de clientes ou trechos operacionais.',
            ),
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: _downloading ? null : _download,
              icon: _downloading
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.download_outlined, size: 18),
              label: const Text('Baixar pacote (.zip)'),
            ),
          ),
        ],
      ),
    );
  }
}

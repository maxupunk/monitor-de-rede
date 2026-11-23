import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Widget reutilizável de progresso ao vivo (via SSE `database_jobs:updated`).
class DatabaseJobProgressBanner extends StatelessWidget {
  const DatabaseJobProgressBanner({super.key, required this.job});

  final DatabaseJobSnapshot job;

  @override
  Widget build(BuildContext context) {
    final tone = switch (job.status) {
      'success' => SemanticTone.success,
      'failed' => SemanticTone.error,
      _ => SemanticTone.info,
    };
    final kindLabel = job.kind == 'restore' ? 'Restauração' : 'Backup';
    final progressValue = (job.databasesTotal > 0 && job.isRunning)
        ? (job.databasesDone / job.databasesTotal).clamp(0.0, 1.0)
        : null;

    return NestedPanel(
      tone: tone,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              if (job.isRunning)
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              else
                Icon(
                  job.status == 'success'
                      ? Icons.check_circle
                      : Icons.error_outline,
                  size: 18,
                ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  '$kindLabel · ${job.database ?? job.stage}',
                  style: context.textTheme.labelLarge?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
              Text(
                job.isRunning
                    ? 'Em andamento'
                    : (job.status == 'success' ? 'Concluído' : 'Falhou'),
                style: context.textTheme.labelSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ),
          if (job.isRunning) ...<Widget>[
            const SizedBox(height: 8),
            LinearProgressIndicator(value: progressValue),
          ],
          const SizedBox(height: 6),
          Text(
            job.message ??
                'Etapa: ${job.stage} · ${formatInteger(job.rows)} linhas · ${formatBytes(job.bytes)}'
                    '${job.databasesTotal > 0 ? ' · ${job.databasesDone}/${job.databasesTotal} bancos' : ''}',
            style: context.textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}

/// Diálogo reutilizável de restauração de backup de banco de dados.
/// Usado em `DatabasesBackupTab` (BackupPage) E em `DeployRunDialog` (DeploysPage).
Future<void> showDatabaseRestoreDialog(
  BuildContext context, {
  required int backupId,
  required String sourceDatabaseName,
  required int sourceConnectionId,
  String? sourceEngine,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Restaurar banco $sourceDatabaseName',
    subtitle: 'Backup #$backupId · progresso ao vivo por SSE',
    maxWidth: 620,
    builder: (_) => _DatabaseRestoreBody(
      backupId: backupId,
      sourceDatabaseName: sourceDatabaseName,
      sourceConnectionId: sourceConnectionId,
      sourceEngine: sourceEngine,
    ),
  );
}

class _DatabaseRestoreBody extends ConsumerStatefulWidget {
  const _DatabaseRestoreBody({
    required this.backupId,
    required this.sourceDatabaseName,
    required this.sourceConnectionId,
    this.sourceEngine,
  });

  final int backupId;
  final String sourceDatabaseName;
  final int sourceConnectionId;
  final String? sourceEngine;

  @override
  ConsumerState<_DatabaseRestoreBody> createState() =>
      _DatabaseRestoreBodyState();
}

class _DatabaseRestoreBodyState extends ConsumerState<_DatabaseRestoreBody> {
  int? _targetConnectionId;
  String _mode = 'new_database'; // 'new_database' | 'replace'
  late final TextEditingController _targetDbController;
  final TextEditingController _confirmController = TextEditingController();
  bool _submitting = false;
  String? _error;
  bool _started = false;

  @override
  void initState() {
    super.initState();
    final today = formatFileTimestamp().substring(0, 8);
    final baseSlug = sanitizeFileSlug(
      widget.sourceDatabaseName,
      fallback: 'banco',
    ).replaceAll(RegExp(r'[^a-z0-9_]'), '_');
    final sanitized = '${baseSlug}_restaurado_$today';
    _targetDbController = TextEditingController(
      text: sanitized.length > 63 ? sanitized.substring(0, 63) : sanitized,
    );
    _targetConnectionId = widget.sourceConnectionId;
    Future<void>.microtask(() async {
      final state = ref.read(databasesProvider);
      if (!state.loaded) {
        await ref.read(databasesProvider.notifier).fetchConnections();
      }
    });
  }

  @override
  void dispose() {
    _targetDbController.dispose();
    _confirmController.dispose();
    super.dispose();
  }

  int? _resolveEffectiveTargetId(List<DatabaseConnectionResponse> connections) {
    DatabaseConnectionResponse? sourceConn;
    for (final c in connections) {
      if (c.id == widget.sourceConnectionId) {
        sourceConn = c;
        break;
      }
    }
    final family =
        sourceConn?.protocolFamily ??
        (widget.sourceEngine == 'postgres' ? 'postgres' : 'mysql');
    final compatible = connections
        .where((c) => c.protocolFamily == family)
        .toList(growable: false);
    if (compatible.any((c) => c.id == _targetConnectionId)) {
      return _targetConnectionId;
    }
    return compatible.isNotEmpty ? compatible.first.id : null;
  }

  Future<void> _submit() async {
    if (_submitting) return;
    final targetConnId = _resolveEffectiveTargetId(
      ref.read(databasesProvider).connections,
    );
    if (targetConnId == null) {
      setState(() => _error = 'Nenhuma conexão compatível selecionada.');
      return;
    }
    final dbName = _mode == 'new_database'
        ? _targetDbController.text.trim()
        : widget.sourceDatabaseName;
    if (dbName.isEmpty) {
      setState(() => _error = 'Informe o nome do banco de destino.');
      return;
    }
    if (_mode == 'replace' && _confirmController.text.trim() != dbName) {
      setState(
        () => _error =
            'Digite exatamente "$dbName" para confirmar a substituição.',
      );
      return;
    }

    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      await ref
          .read(databasesProvider.notifier)
          .restore(
            widget.backupId,
            DatabaseRestoreInput(
              targetConnectionId: targetConnId,
              database: dbName,
              mode: _mode,
              confirmDatabase: _mode == 'replace'
                  ? _confirmController.text.trim()
                  : null,
            ),
          );
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _started = true;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = describeError(
          err,
          fallback: 'Falha ao iniciar a restauração do banco.',
        );
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final dbState = ref.watch(databasesProvider);
    DatabaseConnectionResponse? sourceConn;
    for (final c in dbState.connections) {
      if (c.id == widget.sourceConnectionId) {
        sourceConn = c;
        break;
      }
    }
    final family =
        sourceConn?.protocolFamily ??
        (widget.sourceEngine == 'postgres' ? 'postgres' : 'mysql');
    final compatible = dbState.connections
        .where((c) => c.protocolFamily == family)
        .toList(growable: false);

    final effectiveTargetId = compatible.any((c) => c.id == _targetConnectionId)
        ? _targetConnectionId
        : (compatible.isNotEmpty ? compatible.first.id : null);
    final activeJob = effectiveTargetId != null
        ? dbState.jobFor('restore', effectiveTargetId)
        : null;

    if (_started && activeJob != null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          DatabaseJobProgressBanner(job: activeJob),
          const SizedBox(height: 16),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Fechar'),
            ),
          ),
        ],
      );
    }

    final isPostgres = family == 'postgres';
    final canSubmit =
        !_submitting &&
        effectiveTargetId != null &&
        (_mode == 'new_database' ||
            _confirmController.text.trim() == widget.sourceDatabaseName);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (compatible.isEmpty)
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Nenhuma conexão compatível cadastrada para restaurar este backup.',
            ),
          )
        else
          AppDropdownFormField<int>(
            initialValue: effectiveTargetId,
            decoration: const InputDecoration(
              labelText: 'Conexão de destino',
              helperText:
                  'Somente conexões do mesmo protocolo (PostgreSQL ou MySQL/MariaDB).',
            ),
            items: compatible
                .map(
                  (c) => DropdownMenuItem<int>(
                    value: c.id,
                    child: Text(
                      '${c.name} (${c.engineLabel} · ${c.host}:${c.port})',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                )
                .toList(growable: false),
            onChanged: (v) => setState(() => _targetConnectionId = v),
          ),
        const SizedBox(height: 14),
        SegmentedButton<String>(
          showSelectedIcon: false,
          segments: const <ButtonSegment<String>>[
            ButtonSegment(
              value: 'new_database',
              label: Text(
                'Novo banco',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              icon: Icon(Icons.add_circle_outline, size: 17),
            ),
            ButtonSegment(
              value: 'replace',
              label: Text(
                'Substituir atual',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              icon: Icon(Icons.warning_amber_rounded, size: 17),
            ),
          ],
          selected: <String>{_mode},
          onSelectionChanged: (set) => setState(() => _mode = set.first),
        ),
        const SizedBox(height: 14),
        if (_mode == 'new_database') ...<Widget>[
          TextField(
            controller: _targetDbController,
            decoration: const InputDecoration(
              labelText: 'Nome do novo banco',
              helperText:
                  'Se a restauração falhar, o banco recém-criado é removido automaticamente.',
            ),
          ),
        ] else ...<Widget>[
          NestedPanel(
            tone: SemanticTone.error,
            child: Text(
              isPostgres
                  ? 'Atenção: o PostgreSQL fará substituição transacional do banco "${widget.sourceDatabaseName}". '
                        'Dados criados após o backup serão sobrescritos.'
                  : 'Atenção: no MySQL/MariaDB o banco "${widget.sourceDatabaseName}" será apagado antes de aplicar o dump!',
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _confirmController,
            onChanged: (_) => setState(() {}),
            decoration: InputDecoration(
              labelText: 'Digite "${widget.sourceDatabaseName}" para confirmar',
            ),
          ),
        ],
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: canSubmit ? _submit : null,
            icon: _submitting
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.settings_backup_restore),
            label: Text(
              _mode == 'replace' ? 'Substituir banco' : 'Restaurar agora',
            ),
          ),
        ),
      ],
    );
  }
}

import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Estado visual efetivo do card de uma conexão de banco de dados, combinando
/// o último resultado persistido com o job ao vivo vindo por SSE.
@immutable
class DatabaseBackupCardPresentation {
  const DatabaseBackupCardPresentation({
    required this.effectiveStatus,
    required this.iconTone,
    required this.badgeLabel,
    required this.badgeTone,
    required this.showLastBackupError,
    required this.showBackupJobBanner,
  });

  /// `'running' | 'success' | 'failed'` ou `null` quando nunca executou.
  final String? effectiveStatus;
  final SemanticTone iconTone;
  final String badgeLabel;
  final SemanticTone badgeTone;

  /// Exibe o painel persistido de `conn.lastBackupError` somente quando ele
  /// ainda representa o estado atual e não está superado nem duplicado pelo
  /// banner ao vivo de `activeBackupJob`.
  final bool showLastBackupError;

  /// Exibe o banner ao vivo de `activeBackupJob` enquanto estiver rodando ou
  /// for pelo menos tão recente quanto `conn.lastBackupAt`.
  final bool showBackupJobBanner;

  bool get isFailed => effectiveStatus == 'failed';
  bool get isRunning => effectiveStatus == 'running';
}

bool _isBackupJobCurrent(
  DatabaseConnectionResponse conn,
  DatabaseJobSnapshot job,
) {
  if (job.isRunning) return true;
  final lastAt = DateTime.tryParse(conn.lastBackupAt ?? '');
  final finishedAt = DateTime.tryParse(job.finishedAt ?? '');
  final jobAt = finishedAt ?? DateTime.tryParse(job.startedAt);
  if (lastAt == null || jobAt == null) return true;
  if (finishedAt != null) {
    return !lastAt.isAfter(finishedAt);
  }
  return !lastAt.isAfter(jobAt.add(const Duration(seconds: 5)));
}

/// Calcula o estado visual coerente de uma conexão de banco de dados.
///
/// Quando um novo backup inicia (`running`), o erro anterior (`lastBackupError`)
/// e o selo `Último falhou` são substituídos imediatamente pelo estado
/// `Em andamento`. Quando o job falha na sessão atual, evita duplicar o mesmo
/// texto de erro no card e no banner do job.
DatabaseBackupCardPresentation resolveDatabaseBackupCardPresentation(
  DatabaseConnectionResponse conn,
  DatabaseJobSnapshot? activeBackupJob,
) {
  final jobIsCurrent =
      activeBackupJob != null && _isBackupJobCurrent(conn, activeBackupJob);
  final effectiveStatus = jobIsCurrent
      ? activeBackupJob.status
      : conn.lastBackupStatus;

  final iconTone = switch (effectiveStatus) {
    'success' => SemanticTone.success,
    'failed' => SemanticTone.error,
    'running' => SemanticTone.info,
    _ =>
      conn.storageDestinationId == null
          ? SemanticTone.warning
          : SemanticTone.secondary,
  };

  final (String badgeLabel, SemanticTone badgeTone) = switch (effectiveStatus) {
    'running' => ('Em andamento', SemanticTone.info),
    'failed' => ('Último falhou', SemanticTone.error),
    _ =>
      conn.backupEnabled
          ? ('A cada ${conn.backupIntervalHours}h', SemanticTone.success)
          : ('Manual', SemanticTone.secondary),
  };

  final hasPersistedError =
      conn.lastBackupError != null && conn.lastBackupError!.trim().isNotEmpty;
  final bannerCoversError =
      jobIsCurrent &&
      activeBackupJob.status == 'failed' &&
      (activeBackupJob.message?.trim().isNotEmpty ?? false);

  final showLastBackupError =
      hasPersistedError && effectiveStatus == 'failed' && !bannerCoversError;

  return DatabaseBackupCardPresentation(
    effectiveStatus: effectiveStatus,
    iconTone: iconTone,
    badgeLabel: badgeLabel,
    badgeTone: badgeTone,
    showLastBackupError: showLastBackupError,
    showBackupJobBanner: jobIsCurrent,
  );
}

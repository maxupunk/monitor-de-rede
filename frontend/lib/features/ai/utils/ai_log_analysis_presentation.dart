import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_log_analysis_models.dart';

/// Nome de cada tipo de item, o mesmo na tela e no texto copiado.
String aiLogFindingKindLabel(AiLogFindingKind kind) {
  return switch (kind) {
    AiLogFindingKind.error => 'Erro',
    AiLogFindingKind.warning => 'Aviso',
    AiLogFindingKind.deprecation => 'Descontinuado',
    AiLogFindingKind.suggestion => 'Sugestão',
  };
}

/// "5 avisos", "1 descontinuação"…
String aiLogFindingCountLabel(AiLogFindingKind kind, int count) {
  return switch (kind) {
    AiLogFindingKind.error => countNoun(count, 'erro'),
    AiLogFindingKind.warning => countNoun(count, 'aviso'),
    AiLogFindingKind.deprecation => countNoun(
      count,
      'descontinuação',
      'descontinuações',
    ),
    AiLogFindingKind.suggestion => countNoun(count, 'sugestão', 'sugestões'),
  };
}

/// A análise em Markdown, para colar em outra IA, num chamado ou numa issue.
///
/// Leva só o que a tela mostra — os trechos já vêm com as credenciais
/// ocultadas pelo backend — e diz quando a IA não leu o console inteiro,
/// para quem recebe não tomar a lista como completa.
String formatAiLogAnalysisForCopy(
  AiLogAnalysisReport report, {
  required String source,
}) {
  final out = StringBuffer();
  final title = source.trim().isEmpty ? report.source.trim() : source.trim();
  out.writeln(
    title.isEmpty
        ? '# Análise do console pela IA'
        : '# Análise do console pela IA — $title',
  );

  out
    ..writeln()
    ..writeln('## Resumo')
    ..writeln()
    ..writeln(report.summary.trim().isEmpty ? '(sem resumo)' : report.summary);

  final counts = <String>[
    for (final kind in AiLogFindingKind.values)
      if (report.count(kind) > 0)
        aiLogFindingCountLabel(kind, report.count(kind)),
  ];
  if (counts.isNotEmpty) {
    out
      ..writeln()
      ..writeln(counts.join(' · '));
  }

  final notes = <String>[
    if (report.truncated)
      'O console tem ${countNoun(report.totalLines, 'linha')}; a IA leu '
          '${report.analyzedLines} — as que citam erro, aviso ou '
          'descontinuação e o final.',
    if (report.masked)
      'Credenciais encontradas no console foram ocultadas antes do envio.',
  ];
  if (notes.isNotEmpty) {
    out.writeln();
    for (final note in notes) {
      out.writeln('> $note');
    }
  }

  if (report.structured && report.findings.isNotEmpty) {
    out
      ..writeln()
      ..writeln('## Itens');
    for (final (index, finding) in report.findings.indexed) {
      final line = finding.line == null ? '' : ' — linha ${finding.line}';
      out
        ..writeln()
        ..writeln(
          '### ${index + 1}. ${aiLogFindingKindLabel(finding.kind)}$line',
        )
        ..writeln()
        ..writeln(finding.message);
      if (finding.excerpt.isNotEmpty && finding.excerpt != finding.message) {
        out
          ..writeln()
          ..writeln('```text')
          ..writeln(finding.excerpt)
          ..writeln('```');
      }
      if (finding.suggestion != null) {
        out
          ..writeln()
          ..writeln('**Como resolver:** ${finding.suggestion}');
      }
    }
  } else if (report.structured) {
    out
      ..writeln()
      ..writeln(
        'Nenhum problema encontrado: sem erros, avisos ou descontinuações.',
      );
  }

  final footer = <String>[
    if (report.model != null) 'Modelo ${report.model}',
    if (report.generatedAt != null)
      'gerado em ${formatDateTime(report.generatedAt)}',
  ];
  if (footer.isNotEmpty) {
    out
      ..writeln()
      ..writeln('_${footer.join(' · ')}_');
  }
  return out.toString().trimRight();
}

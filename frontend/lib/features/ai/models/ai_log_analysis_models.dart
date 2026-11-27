import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// O que um item da análise de console é (`LogFindingKind` no backend).
enum AiLogFindingKind {
  error,
  warning,
  deprecation,
  suggestion;

  static AiLogFindingKind parse(Object? raw) {
    return switch (raw?.toString()) {
      'error' => AiLogFindingKind.error,
      'warning' => AiLogFindingKind.warning,
      'deprecation' => AiLogFindingKind.deprecation,
      _ => AiLogFindingKind.suggestion,
    };
  }
}

/// Um problema apontado pela IA, com a linha do console de onde saiu.
@immutable
class AiLogFinding {
  const AiLogFinding({
    required this.kind,
    required this.message,
    this.line,
    this.excerpt = '',
    this.suggestion,
  });

  final AiLogFindingKind kind;
  final int? line;
  final String excerpt;
  final String message;
  final String? suggestion;

  factory AiLogFinding.fromJson(Map<String, dynamic> json) {
    final suggestion = json['suggestion']?.toString();
    return AiLogFinding(
      kind: AiLogFindingKind.parse(json['kind']),
      line: parseIntOrNull(json['line']),
      excerpt: (json['excerpt'] ?? '').toString(),
      message: (json['message'] ?? '').toString(),
      suggestion: suggestion == null || suggestion.isEmpty ? null : suggestion,
    );
  }
}

/// Resposta de `POST /api/ai/log-analysis` (`LogAnalysisReport`).
@immutable
class AiLogAnalysisReport {
  const AiLogAnalysisReport({
    required this.summary,
    required this.findings,
    this.source = '',
    this.structured = true,
    this.totalLines = 0,
    this.analyzedLines = 0,
    this.truncated = false,
    this.masked = false,
    this.model,
    this.generatedAt,
  });

  final String source;
  final String summary;
  final List<AiLogFinding> findings;

  /// A IA respondeu no formato pedido; falso, `summary` é o texto livre dela.
  final bool structured;
  final int totalLines;
  final int analyzedLines;
  final bool truncated;
  final bool masked;
  final String? model;
  final String? generatedAt;

  int count(AiLogFindingKind kind) =>
      findings.where((finding) => finding.kind == kind).length;

  factory AiLogAnalysisReport.fromJson(Map<String, dynamic> json) {
    return AiLogAnalysisReport(
      source: (json['source'] ?? '').toString(),
      summary: (json['summary'] ?? '').toString(),
      findings: asJsonMapList(
        json['findings'],
      ).map(AiLogFinding.fromJson).toList(growable: false),
      structured: parseBool(json['structured'], fallback: true),
      totalLines: parseIntOrNull(json['totalLines']) ?? 0,
      analyzedLines: parseIntOrNull(json['analyzedLines']) ?? 0,
      truncated: parseBool(json['truncated']),
      masked: parseBool(json['masked']),
      model: json['model'] as String?,
      generatedAt: json['generatedAt'] as String?,
    );
  }
}

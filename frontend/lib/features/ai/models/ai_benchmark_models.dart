import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class ChatBenchmarkDomain {
  const ChatBenchmarkDomain({required this.id, required this.label});

  final String id;
  final String label;

  factory ChatBenchmarkDomain.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkDomain(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
    );
  }
}

@immutable
class ChatBenchmarkDomainScore {
  const ChatBenchmarkDomainScore({required this.domain, required this.score});

  final String domain;
  final double score;

  factory ChatBenchmarkDomainScore.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkDomainScore(
      domain: (json['domain'] as String?) ?? '',
      score: parseDoubleOrNull(json['score']) ?? 0,
    );
  }
}

@immutable
class ChatBenchmarkSplitView {
  const ChatBenchmarkSplitView({
    required this.overall,
    required this.ciLow,
    required this.ciHigh,
    required this.tools,
    required this.response,
    required this.safety,
    required this.turnP50Ms,
    required this.turnP95Ms,
    required this.tokensPerSec,
    required this.perfectRatio,
    required this.errors,
    required this.fit,
    this.domains = const <ChatBenchmarkDomainScore>[],
  });

  final double overall;
  final double ciLow;
  final double ciHigh;
  final double tools;
  final double response;
  final double safety;
  final double turnP50Ms;
  final double turnP95Ms;
  final double tokensPerSec;
  final double perfectRatio;
  final int errors;
  final bool fit;
  final List<ChatBenchmarkDomainScore> domains;

  factory ChatBenchmarkSplitView.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkSplitView(
      overall: parseDoubleOrNull(json['overall']) ?? 0,
      ciLow: parseDoubleOrNull(json['overallLow'] ?? json['ciLow']) ?? 0,
      ciHigh: parseDoubleOrNull(json['overallHigh'] ?? json['ciHigh']) ?? 0,
      tools: parseDoubleOrNull(json['tools']) ?? 0,
      response: parseDoubleOrNull(json['answer'] ?? json['response']) ?? 0,
      safety: parseDoubleOrNull(json['safety']) ?? 0,
      turnP50Ms: parseDoubleOrNull(json['turnP50Ms']) ?? 0,
      turnP95Ms: parseDoubleOrNull(json['turnP95Ms']) ?? 0,
      tokensPerSec:
          parseDoubleOrNull(json['tokensPerS'] ?? json['tokensPerSec']) ?? 0,
      perfectRatio:
          parseDoubleOrNull(json['perfect'] ?? json['perfectRatio']) ?? 0,
      errors: parseIntOrNull(json['errors']) ?? 0,
      fit: parseBool(json['fit']),
      domains: asJsonList<ChatBenchmarkDomainScore>(
        json['domains'],
        ChatBenchmarkDomainScore.fromJson,
      ),
    );
  }
}

@immutable
class ChatBenchmarkRow {
  const ChatBenchmarkRow({
    required this.model,
    this.label,
    this.quantization,
    this.downloadMb,
    this.recommended = false,
    this.bundled = false,
    this.fit = false,
    required this.bench,
    this.blind,
  });

  final String model;
  final String? label;
  final String? quantization;
  final int? downloadMb;
  final bool recommended;
  final bool bundled;
  final bool fit;
  final ChatBenchmarkSplitView bench;
  final ChatBenchmarkSplitView? blind;

  factory ChatBenchmarkRow.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkRow(
      model: (json['model'] as String?) ?? '',
      label: json['label'] as String?,
      quantization: json['quantization'] as String?,
      downloadMb: parseIntOrNull(json['downloadMb']),
      recommended: parseBool(json['recommended']),
      bundled: parseBool(json['bundled']),
      fit: parseBool(json['fit']),
      bench: json['bench'] is Map
          ? ChatBenchmarkSplitView.fromJson(asJsonMap(json['bench']))
          : const ChatBenchmarkSplitView(
              overall: 0,
              ciLow: 0,
              ciHigh: 0,
              tools: 0,
              response: 0,
              safety: 0,
              turnP50Ms: 0,
              turnP95Ms: 0,
              tokensPerSec: 0,
              perfectRatio: 0,
              errors: 0,
              fit: false,
            ),
      blind: json['blind'] is Map
          ? ChatBenchmarkSplitView.fromJson(asJsonMap(json['blind']))
          : null,
    );
  }
}

@immutable
class ChatBenchmarkReport {
  const ChatBenchmarkReport({
    required this.measuredOn,
    required this.hardware,
    required this.numCtx,
    required this.timeoutMs,
    required this.minScore,
    required this.minSafety,
    required this.minTools,
    required this.benchCases,
    required this.blindCases,
    this.domains = const <ChatBenchmarkDomain>[],
    this.models = const <ChatBenchmarkRow>[],
  });

  final String measuredOn;
  final String hardware;
  final int numCtx;
  final int timeoutMs;
  final double minScore;
  final double minSafety;
  final double minTools;
  final int benchCases;
  final int blindCases;
  final List<ChatBenchmarkDomain> domains;
  final List<ChatBenchmarkRow> models;

  factory ChatBenchmarkReport.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkReport(
      measuredOn: (json['measuredOn'] as String?) ?? '',
      hardware: (json['hardware'] as String?) ?? '',
      numCtx: parseIntOrNull(json['numCtx']) ?? 16384,
      timeoutMs: parseIntOrNull(json['timeoutMs']) ?? 30000,
      minScore: parseDoubleOrNull(json['minScore']) ?? 0.85,
      minSafety: parseDoubleOrNull(json['minSafety']) ?? 0.9,
      minTools: parseDoubleOrNull(json['minTools']) ?? 0.85,
      benchCases: parseIntOrNull(json['benchCases']) ?? 0,
      blindCases: parseIntOrNull(json['blindCases']) ?? 0,
      domains: asJsonList<ChatBenchmarkDomain>(
        json['domains'],
        ChatBenchmarkDomain.fromJson,
      ),
      models: asJsonList<ChatBenchmarkRow>(
        json['models'],
        ChatBenchmarkRow.fromJson,
      ),
    );
  }
}

@immutable
class DeciderBenchmarkTask {
  const DeciderBenchmarkTask({required this.id, required this.label});

  final String id;
  final String label;

  factory DeciderBenchmarkTask.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkTask(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
    );
  }
}

@immutable
class DeciderBenchmarkTaskScore {
  const DeciderBenchmarkTaskScore({required this.task, required this.score});

  final String task;
  final double score;

  factory DeciderBenchmarkTaskScore.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkTaskScore(
      task: (json['task'] as String?) ?? '',
      score: parseDoubleOrNull(json['score']) ?? 0,
    );
  }
}

@immutable
class DeciderBenchmarkRow {
  const DeciderBenchmarkRow({
    required this.model,
    required this.overall,
    required this.ciLow,
    required this.ciHigh,
    required this.p50Ms,
    required this.p95Ms,
    required this.overTimeout,
    this.recommended = false,
    this.bundled = false,
    this.latencyFrom,
    this.tasks = const <DeciderBenchmarkTaskScore>[],
  });

  final String model;
  final double overall;
  final double ciLow;
  final double ciHigh;
  final double p50Ms;
  final double p95Ms;
  final double overTimeout;
  final bool recommended;
  final bool bundled;
  final String? latencyFrom;
  final List<DeciderBenchmarkTaskScore> tasks;

  factory DeciderBenchmarkRow.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkRow(
      model: (json['model'] as String?) ?? '',
      overall: parseDoubleOrNull(json['overall']) ?? 0,
      ciLow: parseDoubleOrNull(json['overallLow'] ?? json['ciLow']) ?? 0,
      ciHigh: parseDoubleOrNull(json['overallHigh'] ?? json['ciHigh']) ?? 0,
      p50Ms: parseDoubleOrNull(json['p50Ms']) ?? 0,
      p95Ms: parseDoubleOrNull(json['p95Ms']) ?? 0,
      overTimeout: parseDoubleOrNull(json['overTimeout']) ?? 0,
      recommended: parseBool(json['recommended']),
      bundled: parseBool(json['bundled']),
      latencyFrom: json['latencyFrom'] as String?,
      tasks: asJsonList<DeciderBenchmarkTaskScore>(
        json['tasks'],
        DeciderBenchmarkTaskScore.fromJson,
      ),
    );
  }
}

@immutable
class DeciderBenchmarkReport {
  const DeciderBenchmarkReport({
    required this.measuredOn,
    required this.hardware,
    required this.cases,
    required this.timeoutMs,
    this.tasks = const <DeciderBenchmarkTask>[],
    this.models = const <DeciderBenchmarkRow>[],
  });

  final String measuredOn;
  final String hardware;
  final int cases;
  final int timeoutMs;
  final List<DeciderBenchmarkTask> tasks;
  final List<DeciderBenchmarkRow> models;

  factory DeciderBenchmarkReport.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkReport(
      measuredOn: (json['measuredOn'] as String?) ?? '',
      hardware: (json['hardware'] as String?) ?? '',
      cases: parseIntOrNull(json['cases']) ?? 0,
      timeoutMs: parseIntOrNull(json['timeoutMs']) ?? 4000,
      tasks: asJsonList<DeciderBenchmarkTask>(
        json['tasks'],
        DeciderBenchmarkTask.fromJson,
      ),
      models: asJsonList<DeciderBenchmarkRow>(
        json['models'],
        DeciderBenchmarkRow.fromJson,
      ),
    );
  }
}

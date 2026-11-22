import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Modos de automação da IA (`AiAutomationMode`).
enum AiAutomationMode {
  off('off', 'Desligado'),
  confirm('confirm', 'Pedir confirmação'),
  auto('auto', 'Automático');

  const AiAutomationMode(this.id, this.label);

  final String id;
  final String label;

  static AiAutomationMode fromRaw(
    Object? raw, {
    AiAutomationMode fallback = AiAutomationMode.confirm,
  }) {
    final value = raw?.toString();
    for (final mode in AiAutomationMode.values) {
      if (mode.id == value) return mode;
    }
    return fallback;
  }
}

/// Estilo de resposta da IA (`AiResponseStyle`).
enum AiResponseStyle {
  concise('concise', 'Direto'),
  normal('normal', 'Normal');

  const AiResponseStyle(this.id, this.label);

  final String id;
  final String label;

  static AiResponseStyle fromRaw(Object? raw) => raw?.toString() == 'normal'
      ? AiResponseStyle.normal
      : AiResponseStyle.concise;
}

/// Configurações proativas da IA (`AiProactiveSettings`).
@immutable
class AiProactiveSettings {
  const AiProactiveSettings({
    this.incidentSummaries = false,
    this.incidentMinSeverity = 'critical',
    this.maxSummariesPerHour = 6,
    this.digest = 'off',
    this.digestHour = 8,
    this.skipQuietDigest = true,
  });

  final bool incidentSummaries;
  final String incidentMinSeverity;
  final int maxSummariesPerHour;
  final String digest; // 'off' | 'daily' | 'weekly'
  final int digestHour;
  final bool skipQuietDigest;

  AiProactiveSettings copyWith({
    bool? incidentSummaries,
    String? incidentMinSeverity,
    int? maxSummariesPerHour,
    String? digest,
    int? digestHour,
    bool? skipQuietDigest,
  }) {
    return AiProactiveSettings(
      incidentSummaries: incidentSummaries ?? this.incidentSummaries,
      incidentMinSeverity: incidentMinSeverity ?? this.incidentMinSeverity,
      maxSummariesPerHour: maxSummariesPerHour ?? this.maxSummariesPerHour,
      digest: digest ?? this.digest,
      digestHour: digestHour ?? this.digestHour,
      skipQuietDigest: skipQuietDigest ?? this.skipQuietDigest,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'incidentSummaries': incidentSummaries,
    'incidentMinSeverity': incidentMinSeverity,
    'maxSummariesPerHour': maxSummariesPerHour,
    'digest': digest,
    'digestHour': digestHour,
    'skipQuietDigest': skipQuietDigest,
  };

  factory AiProactiveSettings.fromJson(Map<String, dynamic> json) {
    return AiProactiveSettings(
      incidentSummaries: parseBool(json['incidentSummaries']),
      incidentMinSeverity:
          (json['incidentMinSeverity'] as String?) ?? 'critical',
      maxSummariesPerHour: parseIntOrNull(json['maxSummariesPerHour']) ?? 6,
      digest: (json['digest'] as String?) ?? 'off',
      digestHour: parseIntOrNull(json['digestHour']) ?? 8,
      skipQuietDigest: parseBool(json['skipQuietDigest'], fallback: true),
    );
  }
}

/// Progresso de download de modelo via SSE-over-POST (`ModelPullProgress`).
@immutable
class ModelPullProgress {
  const ModelPullProgress({
    required this.status,
    this.digest,
    this.total,
    this.completed,
    this.percentage,
    this.done = false,
    this.error,
  });

  final String status;
  final String? digest;
  final int? total;
  final int? completed;
  final double? percentage;
  final bool done;
  final String? error;

  factory ModelPullProgress.fromJson(Map<String, dynamic> json) {
    return ModelPullProgress(
      status: (json['status'] as String?) ?? '',
      digest: json['digest'] as String?,
      total: parseIntOrNull(json['total']),
      completed: parseIntOrNull(json['completed']),
      percentage: parseDoubleOrNull(json['percentage']),
      done: parseBool(json['done']),
      error: json['error'] as String?,
    );
  }
}

/// Modelo listado em provedores (Ollama / OpenRouter / OpenCode).
@immutable
class AiProviderModelItem {
  const AiProviderModelItem({
    required this.id,
    required this.name,
    this.description,
    this.contextLength,
    this.sizeBytes,
    this.isFree = false,
    this.supportsTools = true,
    this.installed = false,
    this.recommended = false,
    this.vendor,
  });

  final String id;
  final String name;
  final String? description;
  final int? contextLength;
  final int? sizeBytes;
  final bool isFree;
  final bool supportsTools;
  final bool installed;
  final bool recommended;
  final String? vendor;

  factory AiProviderModelItem.fromOllamaInstalled(Map<String, dynamic> json) {
    final name = (json['name'] ?? json['id'] ?? '').toString();
    return AiProviderModelItem(
      id: name,
      name: name,
      sizeBytes: parseIntOrNull(json['size']),
      contextLength: parseIntOrNull(json['contextLength']),
      isFree: true,
      supportsTools: parseBool(json['supportsTools'], fallback: true),
      installed: true,
    );
  }

  factory AiProviderModelItem.fromOllamaRecommended(Map<String, dynamic> json) {
    final name = (json['name'] ?? json['id'] ?? '').toString();
    return AiProviderModelItem(
      id: name,
      name: (json['label'] as String?) ?? name,
      description: json['description'] as String?,
      contextLength: parseIntOrNull(json['contextLength']),
      isFree: true,
      supportsTools: true,
      recommended: true,
    );
  }

  factory AiProviderModelItem.fromRemote(Map<String, dynamic> json) {
    final id = (json['id'] ?? json['name'] ?? '').toString();
    final parts = id.split('/');
    return AiProviderModelItem(
      id: id,
      name: (json['name'] as String?) ?? id,
      description: json['description'] as String?,
      contextLength: parseIntOrNull(json['contextLength']),
      isFree:
          parseBool(json['isFree']) ||
          id.endsWith(':free') ||
          id.contains('-free'),
      supportsTools: parseBool(json['supportsTools'], fallback: true),
      vendor: parts.length > 1 ? parts.first : null,
    );
  }
}

@immutable
class AiProviderModelsCatalog {
  const AiProviderModelsCatalog({
    this.models = const <AiProviderModelItem>[],
    this.isOnline = false,
    this.errorMessage,
  });

  final List<AiProviderModelItem> models;
  final bool isOnline;
  final String? errorMessage;
}

@immutable
class AiTestConnectionResponse {
  const AiTestConnectionResponse({
    required this.success,
    required this.latencyMs,
    required this.message,
    this.model,
  });

  final bool success;
  final int latencyMs;
  final String message;
  final String? model;

  factory AiTestConnectionResponse.fromJson(Map<String, dynamic> json) {
    return AiTestConnectionResponse(
      success: parseBool(json['success']),
      latencyMs: parseIntOrNull(json['latencyMs']) ?? 0,
      message: (json['message'] as String?) ?? '',
      model: json['model'] as String?,
    );
  }
}

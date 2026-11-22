import 'dart:convert';

import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Apresentação do Container Registry interno e severidades SBOM/OSV (ADR 022 / ADR 023).

bool isCommitTag(String tag) =>
    RegExp(r'^[0-9a-f]{40}$', caseSensitive: false).hasMatch(tag);

String shortRegistryTag(String tag) =>
    isCommitTag(tag) ? shortGitSha(tag) : tag;

String shortRegistryDigest(String digest) =>
    digest.length > 19 ? digest.substring(0, 19) : digest;

String registryHostFromUrl(String baseUrl) {
  final uri = Uri.tryParse(baseUrl);
  if (uri == null || uri.host.isEmpty) return baseUrl;
  final hasExplicitPort =
      uri.hasPort &&
      !((uri.scheme == 'http' && uri.port == 80) ||
          (uri.scheme == 'https' && uri.port == 443));
  return hasExplicitPort ? '${uri.host}:${uri.port}' : uri.host;
}

String registryDaemonJson(String host) =>
    const JsonEncoder.withIndent('  ').convert(<String, dynamic>{
      'insecure-registries': <String>[host],
    });

String registryScanSentence(RegistryScanSummary? scan) {
  if (scan == null) return 'Ainda não analisada';
  switch (scan.status) {
    case 'pending':
      return 'Análise em andamento';
    case 'no_sbom':
      return 'Sem lista de pacotes para analisar';
    case 'failed':
      return 'A base de vulnerabilidades não respondeu';
  }
  final serious = scan.critical + scan.high;
  if (serious == 0 && scan.medium + scan.low == 0) {
    return 'Nenhuma vulnerabilidade conhecida';
  }
  if (serious == 0) {
    final minor = scan.medium + scan.low;
    return '${countNoun(minor, 'vulnerabilidade')} ${minor == 1 ? 'média ou baixa' : 'médias ou baixas'}';
  }
  final parts = <String>[
    if (scan.critical > 0) countNoun(scan.critical, 'crítica'),
    if (scan.high > 0) countNoun(scan.high, 'alta'),
  ];
  return parts.join(' e ');
}

SemanticTone registrySeverityTone(String severity) {
  return switch (severity.toLowerCase()) {
    'critical' => SemanticTone.error,
    'high' => SemanticTone.caution,
    'medium' => SemanticTone.warning,
    'low' => SemanticTone.info,
    _ => SemanticTone.secondary,
  };
}

String registrySeverityLabel(String severity) {
  return switch (severity.toLowerCase()) {
    'critical' => 'Crítica',
    'high' => 'Alta',
    'medium' => 'Média',
    'low' => 'Baixa',
    _ => 'Sem nota',
  };
}

SemanticTone registryScanTone(RegistryScanSummary? scan) {
  if (scan == null || scan.status != 'done') return SemanticTone.secondary;
  if (scan.critical > 0) return SemanticTone.error;
  if (scan.high > 0) return SemanticTone.caution;
  if (scan.medium > 0) return SemanticTone.warning;
  return SemanticTone.success;
}

String registryActionLabel(String action) {
  return switch (action) {
    'pull' => 'baixar',
    'push' => 'enviar',
    'delete' => 'apagar',
    _ => action,
  };
}

String registryPlatformsLabel(List<String> platforms) {
  if (platforms.length <= 2) return platforms.join(', ');
  return '${platforms.take(2).join(', ')} e mais ${platforms.length - 2}';
}

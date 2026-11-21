import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Motivo de ferramenta interrompida sem resultado (`aiChatStream.ts`).
const String interruptedToolReason =
    'Interrompido antes do resultado; a ação pode ter sido executada no servidor';

/// Uso de janela a partir do qual o backend compacta automaticamente.
const double autoCompactRatio = 0.8;

const int _minGenerationMs = 250;
const int _maxDecisionLabel = 120;
const int _maxResultExcerpt = 400;

/// As 6 ferramentas de acesso direto a equipamento (`ToolKind::DeviceAccess`, ADR 012).
const Set<String> deviceAccessTools = <String>{
  'fingerprint_device',
  'device_ssh_exec',
  'device_http_request',
  'save_plugin_draft',
  'run_plugin_action',
  'validate_plugin',
};

bool isDeviceAccessTool(String toolName) =>
    deviceAccessTools.contains(toolName);

/// Ferramentas passivas de consulta colapsadas em uma linha única quando concluídas.
const Set<String> _passiveLookupTools = <String>{
  'list_alerts',
  'get_alerts',
  'list_devices',
  'get_device',
  'list_monitors',
  'get_monitor',
  'list_events',
  'search_logs',
  'get_topology',
  'list_containers',
  'list_vpn_peers',
  'get_metrics',
  'list_interfaces',
  'get_dashboard_summary',
};

bool isPassiveLookupTool(AiToolCallState tool) {
  if (tool.status != AiToolStatus.done) return false;
  if (tool.chart != null || tool.summary != null || tool.autoApproved) {
    return false;
  }
  if (isDeviceAccessTool(tool.name)) return false;
  return _passiveLookupTools.contains(tool.name) ||
      tool.name.startsWith('list_') ||
      tool.name.startsWith('get_') ||
      tool.name.startsWith('search_');
}

String toolFriendlyLabel(String name) {
  const labels = <String, String>{
    'list_alerts': 'Alertas',
    'get_alerts': 'Alertas',
    'list_devices': 'Dispositivos',
    'get_device': 'Dispositivo',
    'list_monitors': 'Monitores',
    'get_monitor': 'Monitor',
    'list_events': 'Eventos',
    'search_logs': 'Logs',
    'get_topology': 'Topologia',
    'list_containers': 'Containers',
    'list_vpn_peers': 'Peers VPN',
    'fingerprint_device': 'Identificar equipamento',
    'device_ssh_exec': 'Comando SSH',
    'device_http_request': 'Requisição HTTP no equipamento',
    'save_plugin_draft': 'Salvar rascunho de plugin',
    'run_plugin_action': 'Executar ação de plugin',
    'validate_plugin': 'Validar plugin',
    'ask_user': 'Pergunta',
  };
  return labels[name] ?? name.replaceAll('_', ' ');
}

/// Motivos fixos no feedback negativo (`AI_RATING_REASONS`).
const List<String> aiRatingReasons = <String>[
  'Resposta errada',
  'Não usou os dados da rede',
  'Entendeu errado o que eu pedi',
  'Ação ou ferramenta errada',
  'Incompleta',
  'Lenta demais',
];

AiRating? toggleRating(AiRating? current, String value, [String? reason]) {
  final trimmed = reason?.trim();
  if (current?.value == value &&
      (value == 'up' ||
          trimmed == null ||
          trimmed.isEmpty ||
          current?.reason == trimmed)) {
    return null;
  }
  if (value == 'down') {
    return AiRating(
      value: 'down',
      reason: trimmed != null && trimmed.isNotEmpty ? trimmed : null,
    );
  }
  return const AiRating(value: 'up');
}

/// Aplica um dos 10 tipos de frame SSE-over-POST à mensagem do assistente.
AiDisplayMessage applyChatEvent(
  AiDisplayMessage message,
  Map<String, dynamic> event,
) {
  final type = event['type']?.toString();
  final tools = List<AiToolCallState>.from(message.toolCalls);

  switch (type) {
    case 'textDelta':
      final delta = event['content'];
      if (delta is String) {
        return message.copyWith(content: message.content + delta);
      }
      return message;

    case 'toolCall':
      tools.add(
        AiToolCallState(
          id: (event['id'] ?? '').toString(),
          name: (event['name'] ?? '').toString(),
          arguments: event['arguments'] is Map
              ? Map<String, dynamic>.from(event['arguments'] as Map)
              : const <String, dynamic>{},
          status: AiToolStatus.running,
          autoApproved: event['autoApproved'] == true,
          risk: AiToolRisk.tryParse(event['risk']),
          reason:
              event['reason'] is String &&
                  (event['reason'] as String).isNotEmpty
              ? event['reason'] as String
              : null,
        ),
      );
      return message.copyWith(
        toolCalls: List<AiToolCallState>.unmodifiable(tools),
        roundStart: message.content.length,
      );

    case 'toolResult':
      final targetId = (event['id'] ?? '').toString();
      final index = tools.indexWhere((t) => t.id == targetId);
      if (index >= 0) {
        final result = event['result'] is Map
            ? Map<String, dynamic>.from(event['result'] as Map)
            : <String, dynamic>{};
        final chart = event['chart'] is Map
            ? AiChart.fromJson(Map<String, dynamic>.from(event['chart'] as Map))
            : null;
        tools[index] = tools[index].copyWith(
          result: result,
          chart: chart,
          status: result['error'] != null
              ? AiToolStatus.error
              : AiToolStatus.done,
        );
      }
      return message.copyWith(
        toolCalls: List<AiToolCallState>.unmodifiable(tools),
        roundStart: message.content.length,
      );

    case 'confirmationRequired':
      tools.add(
        AiToolCallState(
          id: (event['id'] ?? '').toString(),
          name: (event['name'] ?? '').toString(),
          arguments: event['arguments'] is Map
              ? Map<String, dynamic>.from(event['arguments'] as Map)
              : const <String, dynamic>{},
          summary: event['summary'] as String?,
          status: AiToolStatus.awaiting,
          risk: AiToolRisk.tryParse(event['risk']),
          reason:
              event['reason'] is String &&
                  (event['reason'] as String).isNotEmpty
              ? event['reason'] as String
              : null,
        ),
      );
      return message.copyWith(
        toolCalls: List<AiToolCallState>.unmodifiable(tools),
        roundStart: message.content.length,
      );

    case 'usage':
      return message.copyWith(usage: AiUsageInfo.fromJson(event));

    case 'notice':
      final msg = event['message'];
      if (msg is! String || msg.isEmpty) return message;
      final notices = List<AiNotice>.from(message.notices);
      final key = event['key'] is String && (event['key'] as String).isNotEmpty
          ? event['key'] as String
          : null;
      final notice = AiNotice(
        message: msg,
        settings: event['settings'] == true,
        key: key,
      );
      final existingIdx = key != null
          ? notices.indexWhere((item) => item.key == key)
          : -1;
      if (existingIdx >= 0) {
        notices[existingIdx] = notice;
      } else if (!notices.any((item) => item.message == msg)) {
        notices.add(notice);
      }
      return message.copyWith(notices: List<AiNotice>.unmodifiable(notices));

    case 'retract':
      final start = (message.roundStart ?? 0).clamp(0, message.content.length);
      return message.copyWith(content: message.content.substring(0, start));

    case 'error':
      return message.copyWith(
        error: event['message'] is String
            ? event['message'] as String
            : 'Erro no provedor de IA.',
        isStreaming: false,
      );

    case 'done':
      return message.copyWith(isStreaming: false);

    default:
      return message;
  }
}

/// Marca ferramentas ainda `running` como `error` quando o stream é interrompido.
AiDisplayMessage interruptRunningTools(
  AiDisplayMessage message, [
  String reason = interruptedToolReason,
]) {
  final updated = message.toolCalls
      .map((tool) {
        if (tool.status != AiToolStatus.running) return tool;
        return tool.copyWith(
          status: AiToolStatus.error,
          result: <String, dynamic>{'error': reason},
        );
      })
      .toList(growable: false);
  return message.copyWith(toolCalls: updated, isStreaming: false);
}

/// Aplica evento `contextCompacted` na lista de mensagens.
List<AiDisplayMessage> applyCompaction(
  List<AiDisplayMessage> messages,
  List<String> sentIds,
  Map<String, dynamic> event,
) {
  final folded = (event['foldedMessages'] as num?)?.toInt();
  final summary = event['summary'];
  if (folded == null || folded < 1 || folded > sentIds.length) return messages;
  if (summary is! String || summary.trim().isEmpty) return messages;

  final targetId = sentIds[folded - 1];
  return messages
      .map((msg) {
        if (msg.id != targetId) return msg;
        return msg.copyWith(
          compaction: AiCompaction(
            summary: summary,
            tokensBefore: (event['tokensBefore'] as num?)?.toInt() ?? 0,
            tokensAfter: (event['tokensAfter'] as num?)?.toInt() ?? 0,
            summarized: event['summarized'] != false,
          ),
        );
      })
      .toList(growable: false);
}

@immutable
class AiQuestion {
  const AiQuestion({required this.question, required this.options});

  final String question;
  final List<String> options;
}

AiQuestion? toolQuestion(AiToolCallState tool) {
  if (tool.name != 'ask_user') return null;
  final question = tool.result?['question'];
  if (question is! String) return null;
  final rawOptions = tool.result?['options'];
  final options = rawOptions is List
      ? rawOptions.whereType<String>().toList(growable: false)
      : const <String>[];
  return AiQuestion(question: question, options: options);
}

({int deviceId, String transport})? missingAccessIn(
  Map<String, dynamic>? result,
) {
  if (result == null) return null;
  final raw = result['missingAccess'];
  if (raw is! Map) return null;
  final deviceId = raw['deviceId'];
  final transport = raw['transport'];
  if (deviceId is num && transport is String && transport.isNotEmpty) {
    return (deviceId: deviceId.toInt(), transport: transport);
  }
  return null;
}

double? tokensPerSecond(AiUsageInfo? usage) {
  if (usage == null || usage.completionTokens <= 0) return null;
  final generation = usage.generationMs ?? 0;
  final elapsed = generation >= _minGenerationMs
      ? generation
      : (usage.durationMs ?? 0);
  if (elapsed <= 0) return null;
  return usage.completionTokens / (elapsed / 1000.0);
}

String? shortModelName(String? model) {
  if (model == null || model.isEmpty) return null;
  final slash = model.lastIndexOf('/');
  return slash >= 0 && slash + 1 < model.length
      ? model.substring(slash + 1)
      : model;
}

@immutable
class AiContextUsage {
  const AiContextUsage({
    required this.used,
    required this.window,
    required this.ratio,
    required this.reported,
  });

  final int used;
  final int window;
  final double ratio;
  final bool reported;
}

AiContextUsage? contextUsage(List<AiDisplayMessage> messages) {
  for (var i = messages.length - 1; i >= 0; i--) {
    final usage = messages[i].usage;
    if (usage?.contextTokens != null &&
        usage!.contextTokens! > 0 &&
        usage.contextWindow != null &&
        usage.contextWindow! > 0) {
      return AiContextUsage(
        used: usage.contextTokens!,
        window: usage.contextWindow!,
        ratio: math.min(1.0, usage.contextTokens! / usage.contextWindow!),
        reported: usage.contextWindowReported == true,
      );
    }
  }
  return null;
}

@immutable
class AiChatHistoryPayload {
  const AiChatHistoryPayload({
    required this.messages,
    required this.ids,
    this.summary,
    this.contextHint,
  });

  final List<Map<String, dynamic>> messages;
  final List<String> ids;
  final String? summary;
  final Map<String, dynamic>? contextHint;
}

String describeMention(AiMention mention) {
  final kind = mention.kind.label.toLowerCase();
  return mention.kind == AiMentionKind.source
      ? '$kind ${mention.label}'
      : '$kind "${mention.label}" (id ${mention.id})';
}

String _decisionLabel(AiToolCallState tool) {
  final first = (tool.summary ?? '').split('\n').first.trim();
  if (first.isEmpty) return tool.name;
  return first.length > _maxDecisionLabel
      ? '${first.substring(0, _maxDecisionLabel)}…'
      : first;
}

String _resultExcerpt(AiToolCallState tool) {
  if (tool.result == null) return '';
  final text = jsonEncode(tool.result);
  final short = text.length > _maxResultExcerpt
      ? '${text.substring(0, _maxResultExcerpt)}…'
      : text;
  return ' Resultado: $short';
}

String? _decisionNote(AiToolCallState tool) {
  final q = toolQuestion(tool);
  if (q != null) {
    final opts = q.options.isNotEmpty
        ? ' Opções: ${q.options.join(', ')}.'
        : '';
    return '[Perguntei ao usuário: ${q.question}$opts]';
  }
  final label = _decisionLabel(tool);
  if (tool.status == AiToolStatus.cancelled) {
    return '[Ação cancelada pelo usuário: $label]';
  }
  if (tool.status == AiToolStatus.awaiting) {
    return '[Ação ainda aguardando confirmação: $label]';
  }
  if (tool.summary != null && tool.status == AiToolStatus.done) {
    return '[Ação confirmada e executada: $label.${_resultExcerpt(tool)}]';
  }
  if (tool.summary != null && tool.status == AiToolStatus.error) {
    return '[Ação confirmada, mas falhou: $label.${_resultExcerpt(tool)}]';
  }
  if ((tool.risk != null || tool.autoApproved) &&
      tool.status == AiToolStatus.done) {
    return '[Ação executada automaticamente: $label]';
  }
  if ((tool.risk != null || tool.autoApproved) &&
      tool.status == AiToolStatus.error) {
    return '[Ação automática falhou ou foi interrompida: $label]';
  }
  return null;
}

AiChatHistoryPayload buildChatHistory(List<AiDisplayMessage> messages) {
  var start = 0;
  String? summary;
  for (var i = messages.length - 1; i >= 0; i--) {
    if (messages[i].compaction != null) {
      start = i + 1;
      summary = messages[i].compaction!.summary;
      break;
    }
  }
  final slice = messages.sublist(start);
  final last = slice.isNotEmpty ? slice.last : null;
  final apiMessages = <Map<String, dynamic>>[];
  final ids = <String>[];

  for (final msg in slice) {
    final notes = <String>[];
    for (final tool in msg.toolCalls) {
      final note = _decisionNote(tool);
      if (note != null) notes.add(note);
    }
    if (msg.mentions.isNotEmpty && !identical(msg, last)) {
      notes.add(
        '[Marcados com @: ${msg.mentions.map(describeMention).join('; ')}]',
      );
    }
    final text = msg.error != null ? '' : msg.content;
    final parts = <String>[
      text,
      ...notes,
    ].where((part) => part.trim().isNotEmpty).toList(growable: false);
    if (parts.isEmpty) continue;
    ids.add(msg.id);
    apiMessages.add(<String, dynamic>{
      'role': msg.role,
      'content': parts.join('\n'),
    });
  }

  Map<String, dynamic>? contextHint;
  for (var i = messages.length - 1; i >= 0; i--) {
    final usage = messages[i].usage;
    if (usage?.contextTokens != null && usage!.contextTokens! > 0) {
      contextHint = <String, dynamic>{
        'tokens': usage.contextTokens,
        'window': usage.contextWindow,
        'model': usage.model,
        'toolGroups': usage.toolGroups ?? const <String>[],
      };
      break;
    }
  }

  return AiChatHistoryPayload(
    messages: apiMessages,
    ids: ids,
    summary: summary,
    contextHint: contextHint,
  );
}

({List<AiDisplayMessage> kept, String content, List<AiMention> mentions})?
rewindTo(List<AiDisplayMessage> messages, String messageId) {
  final index = messages.indexWhere((m) => m.id == messageId);
  if (index < 0 || messages[index].role != 'user') return null;
  final question = messages[index];
  return (
    kept: messages.sublist(0, index),
    content: question.content,
    mentions: question.mentions,
  );
}

// --- @mentions ---

@immutable
class MentionQuery {
  const MentionQuery({required this.start, required this.query});

  final int start;
  final String query;
}

MentionQuery? findMentionQuery(String text, int caret) {
  final clamped = caret.clamp(0, text.length);
  final before = text.substring(0, clamped);
  final start = before.lastIndexOf('@');
  if (start < 0) return null;
  if (start > 0 && !RegExp(r'\s').hasMatch(before[start - 1])) return null;
  final query = before.substring(start + 1);
  if (query.length > 40 || query.contains('\n') || query.contains('@')) {
    return null;
  }
  if (RegExp(r'\s$').hasMatch(query)) return null;
  if (query.trim().isNotEmpty &&
      query.trim().split(RegExp(r'\s+')).length > 3) {
    return null;
  }
  return MentionQuery(start: start, query: query);
}

({String text, int caret}) insertMention(
  String text,
  MentionQuery query,
  int caret,
  String label,
) {
  final token = '@$label ';
  final tail = text
      .substring(caret.clamp(0, text.length))
      .replaceFirst(RegExp(r'^ '), '');
  final next = text.substring(0, query.start) + token + tail;
  return (text: next, caret: query.start + token.length);
}

List<AiMention> addMention(List<AiMention> list, AiMention mention) {
  final exists = list.any((m) => m.kind == mention.kind && m.id == mention.id);
  return exists ? list : <AiMention>[...list, mention];
}

List<AiMention> mentionsInText(String text, List<AiMention> mentions) {
  return mentions
      .where((m) => text.contains('@${m.label}'))
      .toList(growable: false);
}

// --- Screen context & disambiguation ---

@immutable
class AiSuggestion {
  const AiSuggestion({
    required this.title,
    required this.prompt,
    required this.icon,
    this.tone = SemanticTone.primary,
  });

  final String title;
  final String prompt;
  final IconData icon;
  final SemanticTone tone;
}

const List<AiSuggestion> generalAiSuggestions = <AiSuggestion>[
  AiSuggestion(
    title: 'Saúde geral da rede',
    prompt:
        'Faça um resumo da saúde atual da rede, alertas ativos e dispositivos fora do ar.',
    icon: Icons.monitor_heart_outlined,
    tone: SemanticTone.primary,
  ),
  AiSuggestion(
    title: 'Alertas críticos',
    prompt:
        'Quais são os alertas críticos ativos agora e qual a provável causa raiz?',
    icon: Icons.notifications_active_outlined,
    tone: SemanticTone.error,
  ),
  AiSuggestion(
    title: 'Latência e enlaces',
    prompt:
        'Quais monitores ou enlaces estão com maior latência ou instabilidade hoje?',
    icon: Icons.speed,
    tone: SemanticTone.warning,
  ),
  AiSuggestion(
    title: 'Logs recentes',
    prompt:
        'Analise os logs mais recentes com erro ou aviso e destaque anomalias.',
    icon: Icons.article_outlined,
    tone: SemanticTone.info,
  ),
  AiSuggestion(
    title: 'Túneis VPN',
    prompt: 'Qual é a situação atual dos peers da VPN WireGuard?',
    icon: Icons.vpn_lock_outlined,
    tone: SemanticTone.success,
  ),
  AiSuggestion(
    title: 'Containers e servidores',
    prompt:
        'Algum servidor remoto ou container Docker está com uso alto de recursos ou parado?',
    icon: Icons.dns_outlined,
    tone: SemanticTone.secondary,
  ),
];

List<AiSuggestion> mergeSuggestions(
  List<AiSuggestion> own,
  List<AiSuggestion> general, {
  int? limit,
}) {
  final seen = own.map((item) => item.prompt).toSet();
  final combined = <AiSuggestion>[
    ...own,
    ...general.where((item) => !seen.contains(item.prompt)),
  ];
  if (limit == null || combined.length <= limit) return combined;
  return combined.sublist(0, limit);
}

final RegExp _vaguePortugueseRegex = RegExp(
  r'(?:^|[^\p{L}])(isso|isto|aquilo|ess[ea]s?|aquel[ea]s?)(?:$|[^\p{L}])',
  caseSensitive: false,
  unicode: true,
);
final RegExp _herePortugueseRegex = RegExp(
  r'(?:^|[^\p{L}])(aqui|nesta tela|desta tela|dessa tela|nessa tela)(?:$|[^\p{L}])',
  caseSensitive: false,
  unicode: true,
);

/// Retorna `[telaAtual, telaAnterior]` quando a pergunta é ambígua após mudança de tela.
List<AskedScreen>? screenToConfirm({
  required AskedScreen currentScreen,
  required List<AiDisplayMessage> messages,
  required String text,
  required bool hasMentions,
}) {
  if (hasMentions) return null;
  if (!_vaguePortugueseRegex.hasMatch(text)) return null;
  if (_herePortugueseRegex.hasMatch(text)) return null;

  AskedScreen? previous;
  for (var i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role == 'user') {
      previous = messages[i].screen;
      break;
    }
  }
  if (previous == null || previous.id == currentScreen.id) return null;
  return <AskedScreen>[currentScreen, previous];
}

// --- Decider helpers ---

const Set<String> _localOllayaHosts = <String>{
  'ollaya',
  'localhost',
  'host.docker.internal',
  'host.containers.internal',
  '::1',
  '[::1]',
};

bool isLocalOllaya(String? url) {
  final raw = (url ?? '').trim();
  if (raw.isEmpty) return true;
  final parsed = Uri.tryParse(raw.contains('://') ? raw : 'http://$raw');
  if (parsed == null) return false;
  final host = parsed.host.toLowerCase();
  return _localOllayaHosts.contains(host) || host.startsWith('127.');
}

bool isModelInstalled(List<String> installed, String wanted) {
  final target = wanted.trim().toLowerCase();
  if (target.isEmpty) return false;
  final hasExplicitTag = target.contains(':');
  final bare = target.endsWith(':latest')
      ? target.substring(0, target.length - ':latest'.length)
      : target;
  return installed.any((name) {
    final current = name.toLowerCase();
    return current == target ||
        current == bare ||
        (!hasExplicitTag && current.startsWith('$bare:'));
  });
}

String? providerMissing(AiDeciderSettings form, DeciderProviderView? provider) {
  if (provider == null) return null;
  final missing = <String>[];
  if (form.baseUrl.trim().isEmpty && provider.defaultBaseUrl.isEmpty) {
    missing.add('a URL');
  }
  if (provider.requiresAccount && (form.accountId?.trim().isEmpty ?? true)) {
    missing.add('o ID da conta');
  }
  if (provider.requiresKey && (form.apiKey?.trim().isEmpty ?? true)) {
    missing.add('a chave de acesso');
  }
  if (form.model.trim().isEmpty) {
    missing.add('o modelo');
  }
  if (missing.isEmpty) return null;
  if (missing.length == 1) return 'Falta ${missing.first}';
  return 'Faltam ${missing.sublist(0, missing.length - 1).join(', ')} e ${missing.last}';
}

int timeoutForModel({
  required int current,
  int? previousSuggested,
  int? nextSuggested,
  int fallback = 4000,
}) {
  if (nextSuggested == null) return current;
  final untouched = current == (previousSuggested ?? fallback);
  return untouched ? nextSuggested : math.max(current, nextSuggested);
}

SemanticTone accuracyTone(double value) {
  if (value >= 0.85) return SemanticTone.success;
  if (value >= 0.60) return SemanticTone.info;
  return SemanticTone.error;
}

// --- Benchmark helpers ---

String resolveBenchmarkTab(String? raw) {
  if (raw != null && raw.trim().toLowerCase() == 'decider') {
    return 'decider';
  }
  return 'chat';
}

ChatBenchmarkSplitView? chatActiveSplit(ChatBenchmarkRow row, String suite) {
  return suite == 'blind' ? row.blind : row.bench;
}

double? chatDomainScoreOf(ChatBenchmarkSplitView? split, String domainId) {
  if (split == null) return null;
  for (final item in split.domains) {
    if (item.domain == domainId) return item.score;
  }
  return null;
}

({bool passed, String label, SemanticTone tone, String reason}) chatVerdictFact(
  ChatBenchmarkRow row,
  ChatBenchmarkSplitView split,
  ChatBenchmarkReport report,
) {
  if (row.fit && split.fit) {
    return (
      passed: true,
      label: 'Atende ao portão',
      tone: SemanticTone.success,
      reason:
          'Aprovado em todos os critérios de precisão, ferramentas, segurança e latência.',
    );
  }
  final reasons = <String>[];
  if (split.overall < report.minScore) {
    reasons.add(
      'nota ${formatPercent(split.overall * 100, digits: 0)} < ${formatPercent(report.minScore * 100, digits: 0)}',
    );
  }
  if (split.tools < report.minTools) {
    reasons.add(
      'ferramentas ${formatPercent(split.tools * 100, digits: 0)} < ${formatPercent(report.minTools * 100, digits: 0)}',
    );
  }
  if (split.safety < report.minSafety) {
    reasons.add(
      'segurança ${formatPercent(split.safety * 100, digits: 0)} < ${formatPercent(report.minSafety * 100, digits: 0)}',
    );
  }
  if (split.turnP95Ms > report.timeoutMs) {
    reasons.add(
      'p95 ${formatElapsedMs(split.turnP95Ms)} > ${formatElapsedMs(report.timeoutMs)}',
    );
  }
  if (split.errors > 0) {
    reasons.add('${split.errors} erro(s)/timeout(s)');
  }
  return (
    passed: false,
    label: 'Não atende',
    tone: SemanticTone.warning,
    reason: reasons.isNotEmpty
        ? 'Reprovado no portão: ${reasons.join(' · ')}.'
        : 'Não atingiu todos os critérios mínimos em uma das suítes.',
  );
}

double deciderTaskScoreOf(DeciderBenchmarkRow row, String taskId) {
  for (final item in row.tasks) {
    if (item.task == taskId) return item.score;
  }
  return 0;
}

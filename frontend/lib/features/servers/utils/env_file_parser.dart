import 'package:flutter/foundation.dart';

/// Parser e serializador lossless de arquivos `.env` (sem dependência de widgets).
/// Cada entrada carrega um `uid` estável para `ValueKey` em listas editáveis/filtráveis.

final RegExp _envKeyPattern = RegExp(r'^[A-Za-z_][A-Za-z0-9_]*$');
final RegExp _envSecretPattern = RegExp(
  r'(PASS|SECRET|TOKEN|KEY|PRIVATE|CREDENTIAL|AUTH)',
  caseSensitive: false,
);

int _nextEnvLineUid = 1;

@immutable
class EnvLine {
  EnvLine.variable({
    required this.key,
    required this.value,
    this.exported = false,
    int? uid,
  }) : isVariable = true,
       rawText = '',
       uid = uid ?? (_nextEnvLineUid++);

  EnvLine.other(this.rawText, {int? uid})
    : isVariable = false,
      key = '',
      value = '',
      exported = false,
      uid = uid ?? (_nextEnvLineUid++);

  final int uid;
  final bool isVariable;
  final String key;
  final String value;
  final bool exported;
  final String rawText;

  /// Preserva o `uid` para que `ValueKey(line.uid)` não perca o foco ao editar `key`/`value`.
  EnvLine copyWithVariable({String? key, String? value, bool? exported}) {
    return EnvLine.variable(
      key: key ?? this.key,
      value: value ?? this.value,
      exported: exported ?? this.exported,
      uid: uid,
    );
  }
}

List<EnvLine> parseEnvFile(String content) {
  if (content.isEmpty) return const <EnvLine>[];
  final lines = content.replaceAll('\r\n', '\n').split('\n');
  if (lines.isNotEmpty && lines.last.isEmpty) {
    lines.removeLast();
  }
  return lines
      .map((text) {
        final trimmed = text.trim();
        if (trimmed.isEmpty || trimmed.startsWith('#')) {
          return EnvLine.other(text);
        }
        final exported = trimmed.startsWith('export ');
        final body = exported ? trimmed.substring('export '.length) : trimmed;
        final eq = body.indexOf('=');
        final key = eq > 0 ? body.substring(0, eq).trim() : '';
        if (!_envKeyPattern.hasMatch(key)) {
          return EnvLine.other(text);
        }
        return EnvLine.variable(
          key: key,
          value: body.substring(eq + 1),
          exported: exported,
        );
      })
      .toList(growable: false);
}

String serializeEnvFile(List<EnvLine> lines) {
  if (lines.isEmpty) return '';
  final rendered = lines
      .map((line) {
        if (!line.isVariable) return line.rawText;
        return '${line.exported ? 'export ' : ''}${line.key}=${line.value}';
      })
      .join('\n');
  return '$rendered\n';
}

bool isValidEnvKey(String key) => _envKeyPattern.hasMatch(key);
bool looksLikeSecretKey(String key) => _envSecretPattern.hasMatch(key);

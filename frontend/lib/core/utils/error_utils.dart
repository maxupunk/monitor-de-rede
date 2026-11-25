import 'dart:async';

import 'package:netmonitor_app/core/network/api_client.dart';

final RegExp _technicalPrefixPattern = RegExp(
  r'^(Exception|Error|_Exception|StateError|Bad state|ArgumentError|Invalid argument\(s\)|FormatException):\s*',
);

/// Converte qualquer erro capturado em mensagem amigável em PT-BR para a UI,
/// sem vazar prefixos técnicos (`Exception: ...`, `Instance of ...`).
String describeError(
  Object? err, {
  String fallback = 'Não foi possível concluir a operação.',
}) {
  if (err == null) return fallback;

  if (err is ApiException) {
    final msg = err.message.trim();
    if (msg.isNotEmpty) return msg;
    if (err.fieldErrors.isNotEmpty) {
      return err.fieldErrors.map((e) => e.message).join(' ');
    }
    return fallback;
  }

  if (err is NetworkException) {
    final msg = err.message.trim();
    return msg.isNotEmpty ? msg : 'Falha de conexão com o servidor.';
  }

  if (err is TimeoutException) {
    return 'Tempo limite esgotado ao comunicar com o servidor.';
  }

  if (err is FormatException) {
    final msg = err.message.trim();
    return msg.isNotEmpty ? msg : 'Resposta em formato inválido.';
  }

  final raw = err.toString().trim();
  if (raw.isEmpty || raw.startsWith('Instance of ')) {
    return fallback;
  }

  final cleaned = raw.replaceFirst(_technicalPrefixPattern, '').trim();
  return cleaned.isNotEmpty ? cleaned : fallback;
}

/// Como [describeError], para uma **ação** (iniciar, parar, aplicar).
///
/// Sem resposta não quer dizer que a ação falhou: a central pode tê-la
/// concluído depois que o navegador desistiu de esperar. Dizer "falha" aí
/// levava o operador a repetir o comando em algo que já tinha mudado.
String describeActionError(
  Object? err, {
  String fallback = 'Não foi possível concluir a operação.',
}) {
  if (err is NetworkException && err.timedOut) {
    return '${err.message} A ação pode ter sido concluída mesmo assim: '
        'confira o estado na tela, que se atualiza sozinha.';
  }
  return describeError(err, fallback: fallback);
}

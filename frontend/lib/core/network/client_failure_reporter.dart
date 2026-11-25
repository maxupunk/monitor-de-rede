import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';

/// Avisa a central das requisições que ficaram sem resposta.
///
/// Quando o navegador desiste de esperar ou a conexão cai no caminho, a
/// central não fica sabendo — e o operador vê "falha de conexão" sem rastro
/// nenhum no log. O aviso entra no log da aplicação (Logs › Este servidor),
/// ao lado do "requisição lenta" que a central registra para o mesmo caminho.
///
/// Se o próprio aviso não chega (a rede caiu de vez), ele fica guardado e
/// segue junto com o próximo — até [maxPending], descartando os mais antigos.
class ClientFailureReporter {
  ClientFailureReporter(this._send, {String? Function()? currentPage})
    : _currentPage = currentPage ?? _browserPage;

  static const int maxPending = 20;

  final Future<void> Function(Map<String, dynamic> body) _send;
  final String? Function() _currentPage;
  final List<Map<String, dynamic>> _pending = <Map<String, dynamic>>[];
  bool _sending = false;

  @visibleForTesting
  int get pendingCount => _pending.length;

  void report(ClientFailure failure) {
    _pending.add(<String, dynamic>{
      'method': failure.method,
      'path': '/api${failure.path}',
      'kind': failure.timedOut ? 'timeout' : 'network',
      'elapsedMs': failure.elapsed.inMilliseconds,
      'page': _currentPage(),
    });
    if (_pending.length > maxPending) _pending.removeAt(0);
    unawaited(_flush());
  }

  Future<void> _flush() async {
    if (_sending) return;
    _sending = true;
    try {
      while (_pending.isNotEmpty) {
        final next = _pending.first;
        try {
          await _send(next);
        } on NetworkException {
          // A central continua fora de alcance: tenta no próximo aviso.
          return;
        } catch (_) {
          // A central respondeu e recusou: insistir não muda a resposta.
        }
        _pending.remove(next);
      }
    } finally {
      _sending = false;
    }
  }

  static String? _browserPage() => kIsWeb ? Uri.base.path : null;
}

final Provider<ClientFailureReporter> clientFailureReporterProvider =
    Provider<ClientFailureReporter>((ref) {
      return ClientFailureReporter((body) async {
        // Sem sessão, o aviso viraria um 401 — e o 401 encerra a sessão.
        final token = ref.read(sessionTokenProvider);
        if (token == null || token.isEmpty) return;
        await ref
            .read(apiClientProvider)
            .post<dynamic>(
              clientFailuresPath,
              data: body,
              timeout: const Duration(seconds: 10),
            );
      });
    });

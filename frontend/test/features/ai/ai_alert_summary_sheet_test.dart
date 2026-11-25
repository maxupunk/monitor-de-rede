import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_alert_summary_sheet.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Resumo controlado pelo teste: a resposta só chega quando ele mandar.
class _ControlledFeedback extends AiAlertFeedbackNotifier {
  _ControlledFeedback(this.reply);

  final Completer<AiIncidentSummary?> reply;
  Object? error;
  int requests = 0;

  @override
  Future<AiIncidentSummary?> requestSummary(int alertId) async {
    requests++;
    final failure = error;
    if (failure != null) throw failure;
    return reply.future;
  }
}

const AiAlertSummaryTarget _target = AiAlertSummaryTarget(
  alertId: 42,
  title: 'Túnel VPN caiu — hostinger-srv-1',
  deviceId: 7,
);

Future<_ControlledFeedback> _pump(
  WidgetTester tester, {
  AiAlertSummaryTarget target = _target,
  Completer<AiIncidentSummary?>? reply,
  List<AiIncidentSummary>? generated,
}) async {
  final feedback = _ControlledFeedback(
    reply ?? Completer<AiIncidentSummary?>(),
  );
  await _pumpWith(tester, feedback, target: target, generated: generated);
  return feedback;
}

Future<void> _pumpWith(
  WidgetTester tester,
  _ControlledFeedback feedback, {
  AiAlertSummaryTarget target = _target,
  List<AiIncidentSummary>? generated,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        aiAlertFeedbackProvider.overrideWith(() => feedback),
      ],
      child: MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: SingleChildScrollView(
            child: AiAlertSummaryBody(
              target: target,
              onGenerated: generated?.add,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

void main() {
  testWidgets('abre explicando e mostrando que a IA está trabalhando', (
    tester,
  ) async {
    final reply = Completer<AiIncidentSummary?>();
    final generated = <AiIncidentSummary>[];
    final feedback = await _pump(tester, reply: reply, generated: generated);

    expect(find.text('Como funciona'), findsOneWidget);
    expect(find.textContaining('A IA está investigando'), findsOneWidget);
    expect(feedback.requests, 1);

    reply.complete(
      const AiIncidentSummary(text: 'Peer sem handshake; reinicie o túnel.'),
    );
    await tester.pumpAndSettle();

    expect(find.text('Peer sem handshake; reinicie o túnel.'), findsOneWidget);
    expect(find.text('Continuar no chat'), findsOneWidget);
    expect(generated.single.text, 'Peer sem handshake; reinicie o túnel.');
  });

  testWidgets('alerta que já tem resumo mostra sem pedir de novo', (
    tester,
  ) async {
    final feedback = await _pump(
      tester,
      target: const AiAlertSummaryTarget(
        alertId: 42,
        title: 'Interface caiu',
        existing: AiIncidentSummary(text: 'PPPoE caiu no provedor.'),
      ),
    );
    expect(find.text('PPPoE caiu no provedor.'), findsOneWidget);
    expect(feedback.requests, 0);
  });

  testWidgets('sem nada a gerar, explica por quê', (tester) async {
    final reply = Completer<AiIncidentSummary?>()..complete(null);
    await _pump(tester, reply: reply);
    await tester.pumpAndSettle();
    expect(
      find.textContaining('Não há resumo novo para gerar'),
      findsOneWidget,
    );
  });

  testWidgets('falha mostra o motivo e deixa tentar de novo', (tester) async {
    final feedback = _ControlledFeedback(Completer<AiIncidentSummary?>())
      ..error = NetworkException.timeout(const Duration(seconds: 90));
    await _pumpWith(tester, feedback);
    await tester.pumpAndSettle();

    expect(find.textContaining('90 s'), findsOneWidget);
    await tester.tap(find.text('Tentar de novo'));
    await tester.pump();
    expect(feedback.requests, 2);
  });
}

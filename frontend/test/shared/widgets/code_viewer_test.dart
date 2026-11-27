import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

String _lines(int count) =>
    List<String>.generate(count, (i) => 'linha ${i + 1}').join('\n');

/// O console cresce quando o teste manda, como o SSE faria.
Future<ValueNotifier<String>> _pumpConsole(
  WidgetTester tester, {
  bool followTail = true,
  int initialLines = 80,
}) async {
  final code = ValueNotifier<String>(_lines(initialLines));
  addTearDown(code.dispose);
  await tester.pumpWidget(
    ProviderScope(
      child: MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: ValueListenableBuilder<String>(
            valueListenable: code,
            builder: (context, value, child) => CodeViewer(
              title: 'Console',
              code: value,
              maxHeight: 200,
              followTail: followTail,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return code;
}

ScrollPosition _verticalPosition(WidgetTester tester) {
  return tester
      .state<ScrollableState>(
        find
            .byWidgetPredicate(
              (w) => w is Scrollable && w.axisDirection == AxisDirection.down,
            )
            .first,
      )
      .position;
}

void main() {
  testWidgets('acompanha a última linha quando chega linha nova', (
    tester,
  ) async {
    final code = await _pumpConsole(tester);
    var position = _verticalPosition(tester);
    expect(position.maxScrollExtent, greaterThan(0));
    expect(position.pixels, position.maxScrollExtent);

    code.value = _lines(160);
    await tester.pumpAndSettle();
    position = _verticalPosition(tester);
    expect(position.pixels, position.maxScrollExtent);
    expect(find.text('Ir para o fim'), findsNothing);
  });

  testWidgets('quem rolou para cima não é arrancado; "Ir para o fim" retoma', (
    tester,
  ) async {
    final code = await _pumpConsole(tester);
    _verticalPosition(tester).jumpTo(0);
    await tester.pumpAndSettle();
    expect(find.text('Ir para o fim'), findsOneWidget);

    code.value = _lines(160);
    await tester.pumpAndSettle();
    final position = _verticalPosition(tester);
    expect(position.pixels, 0, reason: 'leitura não é interrompida');
    expect(find.text('Novas linhas · Ir para o fim'), findsOneWidget);

    await tester.tap(find.text('Novas linhas · Ir para o fim'));
    await tester.pumpAndSettle();
    expect(
      _verticalPosition(tester).pixels,
      _verticalPosition(tester).maxScrollExtent,
    );
    expect(find.textContaining('Ir para o fim'), findsNothing);

    code.value = _lines(240);
    await tester.pumpAndSettle();
    expect(
      _verticalPosition(tester).pixels,
      _verticalPosition(tester).maxScrollExtent,
      reason: 'acompanhamento retomado',
    );
  });

  testWidgets('sem followTail, código fica no topo', (tester) async {
    final code = await _pumpConsole(tester, followTail: false);
    expect(_verticalPosition(tester).pixels, 0);
    code.value = _lines(160);
    await tester.pumpAndSettle();
    expect(_verticalPosition(tester).pixels, 0);
    expect(find.textContaining('Ir para o fim'), findsNothing);
  });

  group('AiLogAnalysisButton no cabeçalho', () {
    Future<void> pumpButton(
      WidgetTester tester, {
      required String content,
      AiLogAnalysisLauncher? launcher,
    }) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            aiLogAnalysisLauncherProvider.overrideWith((ref) => launcher),
          ],
          child: MaterialApp(
            theme: AppTheme.light(),
            home: Scaffold(
              body: CodeViewer(
                title: 'Console',
                code: content.isEmpty ? '(Aguardando...)' : content,
                actions: <Widget>[
                  AiLogAnalysisButton(
                    source: 'Compilação #7',
                    content: content,
                    chatPrompt: 'Sobre a compilação #7',
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pump();
    }

    testWidgets('fica ao lado de Copiar e manda o console inteiro', (
      tester,
    ) async {
      final requests = <AiLogAnalysisRequest>[];
      await pumpButton(
        tester,
        content: 'npm ERR! boom\nfim',
        launcher: (_, request) async => requests.add(request),
      );
      expect(find.text('Copiar'), findsOneWidget);
      await tester.tap(find.text('Analisar com a IA'));
      await tester.pump();
      expect(requests.single.content, 'npm ERR! boom\nfim');
      expect(requests.single.source, 'Compilação #7');
      expect(requests.single.chatPrompt, 'Sobre a compilação #7');
    });

    testWidgets('console vazio deixa o botão desabilitado', (tester) async {
      final requests = <AiLogAnalysisRequest>[];
      await pumpButton(
        tester,
        content: '',
        launcher: (_, request) async => requests.add(request),
      );
      final button = tester.widget<TextButton>(
        find.ancestor(
          of: find.text('Analisar com a IA'),
          matching: find.byWidgetPredicate((w) => w is TextButton),
        ),
      );
      expect(button.onPressed, isNull);
    });

    testWidgets('sem o Assistente montado, o botão não aparece', (
      tester,
    ) async {
      await pumpButton(tester, content: 'linha');
      expect(find.text('Analisar com a IA'), findsNothing);
      expect(find.text('Copiar'), findsOneWidget);
    });
  });
}

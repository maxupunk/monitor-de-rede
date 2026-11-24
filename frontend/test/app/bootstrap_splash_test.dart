import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/app/app.dart';

/// Rotas que o app anunciou ao navegador (na Web, isso reescreve a URL).
List<Object?> _captureRouteAnnouncements(WidgetTester tester) {
  final announced = <Object?>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.navigation,
    (call) async {
      if (call.method == 'routeInformationUpdated') {
        announced.add(call.arguments);
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.navigation,
      null,
    ),
  );
  return announced;
}

void main() {
  testWidgets(
    'a espera do boot não anuncia rota: o link direto (?code= do GitHub) sobrevive',
    (tester) async {
      final announced = _captureRouteAnnouncements(tester);

      await tester.pumpWidget(
        const BootstrapSplashApp(themeMode: ThemeMode.light),
      );

      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      expect(find.byType(Navigator), findsNothing);
      expect(announced, isEmpty);
    },
  );

  testWidgets(
    'controle: um MaterialApp com home anuncia "/" e apagaria o link',
    (tester) async {
      final announced = _captureRouteAnnouncements(tester);

      await tester.pumpWidget(const MaterialApp(home: SizedBox()));

      expect(announced, isNotEmpty);
    },
  );
}

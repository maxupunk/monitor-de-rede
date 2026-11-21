import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/shared/widgets/semantic_chip.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  testWidgets('AppTheme provides high-contrast semantic colors including plugin token', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        darkTheme: AppTheme.dark(),
        themeMode: ThemeMode.dark,
        home: const Scaffold(
          body: Center(
            child: SemanticChip(
              label: 'Plugin Ativo',
              tone: SemanticTone.plugin,
              icon: Icons.extension_outlined,
            ),
          ),
        ),
      ),
    );

    expect(find.text('Plugin Ativo'), findsOneWidget);
    final element = tester.element(find.text('Plugin Ativo'));
    expect(element.semanticColors.plugin, const Color(0xFF1DE9B6));
  });
}

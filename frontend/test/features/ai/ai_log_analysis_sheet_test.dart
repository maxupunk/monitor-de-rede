import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_log_analysis_presentation.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_log_analysis_sheet.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const AiLogAnalysisReport _report = AiLogAnalysisReport(
  summary: 'A instalação das dependências falhou.',
  totalLines: 3000,
  analyzedLines: 640,
  truncated: true,
  masked: true,
  model: 'qwen3:8b',
  findings: <AiLogFinding>[
    AiLogFinding(
      kind: AiLogFindingKind.error,
      line: 2,
      excerpt: 'npm ERR! code ERESOLVE',
      message: 'Conflito de dependências no npm',
      suggestion: 'Alinhe as versões ou use --legacy-peer-deps',
    ),
    AiLogFinding(
      kind: AiLogFindingKind.deprecation,
      message: 'Pacote request descontinuado',
    ),
  ],
);

const AiLogAnalysisRequest _request = AiLogAnalysisRequest(
  source: 'Compilação #7 · api',
  content: '#1 load build definition\nnpm ERR! code ERESOLVE\nfim',
  chatPrompt: 'Sobre a compilação #7',
);

class _DisabledAi extends AiSettingsNotifier {
  @override
  AiSettingsState build() =>
      const AiSettingsState(settings: AiSettings(enabled: false));
}

/// Conta os pedidos e responde com o que o teste mandar.
class _Analyses {
  int requests = 0;
  Completer<AiLogAnalysisReport> reply = Completer<AiLogAnalysisReport>();
  Object? error;

  Future<AiLogAnalysisReport> call(AiLogAnalysisRequest request) async {
    requests++;
    final failure = error;
    if (failure != null) throw failure;
    return reply.future;
  }
}

Future<_Analyses> _pump(WidgetTester tester, {bool disabled = false}) async {
  final analyses = _Analyses();
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        aiLogAnalysisProvider.overrideWith(
          (ref, request) => analyses.call(request),
        ),
        if (disabled) aiSettingsProvider.overrideWith(_DisabledAi.new),
      ],
      child: MaterialApp(
        theme: AppTheme.light(),
        home: const Scaffold(
          body: SingleChildScrollView(
            child: AiLogAnalysisBody(request: _request),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
  return analyses;
}

void main() {
  testWidgets('mostra o progresso e depois os itens com linha e solução', (
    tester,
  ) async {
    final analyses = await _pump(tester);
    expect(find.textContaining('A IA está lendo o console'), findsOneWidget);
    expect(find.textContaining('3 linhas'), findsOneWidget);
    expect(analyses.requests, 1);
    expect(find.text('Copiar análise'), findsNothing);

    analyses.reply.complete(
      const AiLogAnalysisReport(
        summary: 'A instalação das dependências falhou.',
        totalLines: 3000,
        analyzedLines: 640,
        truncated: true,
        masked: true,
        findings: <AiLogFinding>[
          AiLogFinding(
            kind: AiLogFindingKind.error,
            line: 2,
            excerpt: 'npm ERR! code ERESOLVE',
            message: 'Conflito de dependências no npm',
            suggestion: 'Alinhe as versões ou use --legacy-peer-deps',
          ),
          AiLogFinding(
            kind: AiLogFindingKind.deprecation,
            message: 'Pacote request descontinuado',
          ),
        ],
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('A instalação das dependências falhou.'), findsOneWidget);
    expect(find.text('1 erro'), findsOneWidget);
    expect(find.text('1 descontinuação'), findsOneWidget);
    expect(find.text('Erro'), findsOneWidget);
    expect(find.text('Descontinuado'), findsOneWidget);
    expect(find.text('Linha 2'), findsOneWidget);
    expect(find.text('npm ERR! code ERESOLVE'), findsOneWidget);
    expect(find.textContaining('Como resolver'), findsOneWidget);
    expect(find.textContaining('a IA leu 640'), findsOneWidget);
    expect(find.textContaining('Credenciais encontradas'), findsOneWidget);
    expect(find.text('Continuar no chat'), findsOneWidget);
    expect(find.text('Analisar de novo'), findsOneWidget);
  });

  testWidgets('lista vazia diz que nenhum problema foi encontrado', (
    tester,
  ) async {
    final analyses = await _pump(tester);
    analyses.reply.complete(
      const AiLogAnalysisReport(
        summary: 'Compilação limpa.',
        findings: <AiLogFinding>[],
      ),
    );
    await tester.pumpAndSettle();
    expect(find.textContaining('Nenhum problema encontrado'), findsOneWidget);
    expect(find.textContaining('a IA leu'), findsNothing);
  });

  testWidgets('resposta fora do formato não finge que está tudo certo', (
    tester,
  ) async {
    final analyses = await _pump(tester);
    analyses.reply.complete(
      const AiLogAnalysisReport(
        summary: 'Parece que o npm falhou.',
        findings: <AiLogFinding>[],
        structured: false,
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Parece que o npm falhou.'), findsOneWidget);
    expect(find.textContaining('sem a lista de itens'), findsOneWidget);
    expect(find.textContaining('Nenhum problema encontrado'), findsNothing);
  });

  testWidgets('IA desligada explica e leva às configurações, sem pedir', (
    tester,
  ) async {
    final analyses = await _pump(tester, disabled: true);
    expect(find.textContaining('A IA está desligada'), findsOneWidget);
    expect(find.text('Abrir configurações de IA'), findsOneWidget);
    expect(find.text('Analisar de novo'), findsNothing);
    expect(find.text('Copiar análise'), findsNothing);
    expect(analyses.requests, 0);
  });

  testWidgets('falha mostra o motivo e deixa tentar de novo', (tester) async {
    final analyses = _Analyses()
      ..error = NetworkException.timeout(const Duration(seconds: 120));
    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[
          aiLogAnalysisProvider.overrideWith(
            (ref, request) => analyses.call(request),
          ),
        ],
        child: MaterialApp(
          theme: AppTheme.light(),
          home: const Scaffold(
            body: SingleChildScrollView(
              child: AiLogAnalysisBody(request: _request),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.textContaining('120 s'), findsOneWidget);
    expect(find.text('Abrir configurações de IA'), findsOneWidget);
    expect(find.text('Copiar análise'), findsNothing);
    await tester.tap(find.text('Tentar de novo'));
    await tester.pumpAndSettle();
    expect(analyses.requests, 2);
  });

  testWidgets('copia a análise em Markdown para usar em outra IA', (
    tester,
  ) async {
    String? copied;
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'Clipboard.setData') {
          copied = (call.arguments as Map)['text'] as String?;
        }
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      ),
    );

    final analyses = await _pump(tester);
    analyses.reply.complete(_report);
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.text('Copiar análise'));
    await tester.tap(find.text('Copiar análise'));
    await tester.pumpAndSettle();

    expect(
      copied,
      formatAiLogAnalysisForCopy(_report, source: _request.source),
    );
    expect(find.textContaining('Análise copiada'), findsOneWidget);
  });

  group('formatAiLogAnalysisForCopy', () {
    test('leva resumo, contagens, avisos e cada item com linha e solução', () {
      final text = formatAiLogAnalysisForCopy(
        _report,
        source: 'Compilação #7 · api',
      );
      expect(
        text,
        startsWith('# Análise do console pela IA — Compilação #7 · api'),
      );
      expect(text, contains('A instalação das dependências falhou.'));
      expect(text, contains('1 erro · 1 descontinuação'));
      expect(text, contains('> O console tem 3000 linhas; a IA leu 640'));
      expect(text, contains('> Credenciais encontradas'));
      expect(text, contains('### 1. Erro — linha 2'));
      expect(text, contains('```text\nnpm ERR! code ERESOLVE\n```'));
      expect(
        text,
        contains(
          '**Como resolver:** Alinhe as versões ou use --legacy-peer-deps',
        ),
      );
      expect(text, contains('### 2. Descontinuado\n'));
      expect(text, contains('_Modelo qwen3:8b_'));
    });

    test('sem itens diz que está limpo; fora do formato não afirma isso', () {
      const clean = AiLogAnalysisReport(
        summary: 'Compilação limpa.',
        findings: <AiLogFinding>[],
      );
      expect(
        formatAiLogAnalysisForCopy(clean, source: ''),
        allOf(
          startsWith('# Análise do console pela IA\n'),
          contains('Nenhum problema encontrado'),
          isNot(contains('> ')),
        ),
      );
      const free = AiLogAnalysisReport(
        summary: 'Parece que o npm falhou.',
        findings: <AiLogFinding>[],
        structured: false,
      );
      expect(
        formatAiLogAnalysisForCopy(free, source: 'x'),
        allOf(
          contains('Parece que o npm falhou.'),
          isNot(contains('Nenhum problema encontrado')),
        ),
      );
    });
  });
}

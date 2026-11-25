import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_installed_models_panel.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _AuthStub extends AuthNotifier {
  _AuthStub(this.role);

  final UserRole role;

  @override
  AuthState build() => AuthState(
    token: 'test-token',
    initialized: true,
    user: AuthUser(
      id: 1,
      email: 'admin@netmonitor.local',
      fullName: 'Admin',
      role: role,
    ),
  );
}

/// Ollama falso: três modelos, um deles o do chat. Anota o que foi apagado.
class _FakeAiRepository implements AiRepository {
  final List<String> deleted = <String>[];
  int listCalls = 0;

  List<Map<String, dynamic>> _installed = <Map<String, dynamic>>[
    <String, dynamic>{
      'name': 'lfm3-netmonitor:latest',
      'size': 1846237728,
      'modifiedAt': '2026-10-01T10:00:00Z',
      'inUse': true,
    },
    <String, dynamic>{
      'name': 'llama3.2:3b',
      'size': 2019393189,
      'modifiedAt': '2025-03-01T10:00:00Z',
    },
    <String, dynamic>{
      'name': 'llama3.2:latest',
      'size': 2019393189,
      'modifiedAt': '2025-03-01T10:00:00Z',
      'sharedWith': <String>['llama3.2:3b'],
    },
  ];

  @override
  Future<AiProviderModelsCatalog> fetchOllamaModels({String? baseUrl}) async {
    listCalls++;
    return AiProviderModelsCatalog(
      isOnline: true,
      models: _installed
          .map(AiProviderModelItem.fromOllamaInstalled)
          .toList(growable: false),
    );
  }

  @override
  Future<AiModelRemoval> deleteOllamaModel({
    required String model,
    String? baseUrl,
  }) async {
    deleted.add(model);
    _installed = _installed.where((m) => m['name'] != model).toList();
    return AiModelRemoval(name: model, freedBytes: 2019393189);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

Future<_FakeAiRepository> _pump(
  WidgetTester tester, {
  UserRole role = UserRole.admin,
}) async {
  final repo = _FakeAiRepository();
  final container = ProviderContainer(
    overrides: <Override>[
      aiRepositoryProvider.overrideWithValue(repo),
      authProvider.overrideWith(() => _AuthStub(role)),
    ],
  );
  addTearDown(container.dispose);
  await container.read(aiSettingsProvider.notifier).loadOllamaModels();

  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: SingleChildScrollView(
            child: AiInstalledModelsPanel(baseUrl: () => ''),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return repo;
}

void main() {
  testWidgets('lista os modelos com o espaço total e protege o modelo em uso', (
    tester,
  ) async {
    await _pump(tester);

    expect(find.text('Modelos baixados no servidor'), findsOneWidget);
    // Os dois nomes do llama3.2 dividem o arquivo: o total conta uma vez só.
    expect(find.textContaining('3 modelos · 3.6 GiB no disco'), findsOneWidget);
    expect(find.text('Em uso'), findsOneWidget);
    // Só os dois que não estão em uso têm o botão de apagar.
    expect(find.byTooltip('Apagar modelo'), findsNWidgets(2));
    expect(find.textContaining('mesmo arquivo de llama3.2:3b'), findsOneWidget);
  });

  testWidgets('apaga após confirmar, mostra o espaço liberado e recarrega', (
    tester,
  ) async {
    final repo = await _pump(tester);
    final callsBefore = repo.listCalls;

    await tester.tap(find.byTooltip('Apagar modelo').first);
    await tester.pumpAndSettle();
    expect(find.text('Apagar o modelo llama3.2:3b?'), findsOneWidget);
    expect(find.textContaining('libera 1.9 GiB'), findsOneWidget);

    await tester.tap(find.text('Apagar modelo'));
    await tester.pumpAndSettle();

    expect(repo.deleted, <String>['llama3.2:3b']);
    expect(repo.listCalls, callsBefore + 1);
    expect(find.textContaining('1.9 GiB liberados'), findsOneWidget);
    expect(find.text('llama3.2:3b'), findsNothing);
  });

  testWidgets('cancelar a confirmação não apaga nada', (tester) async {
    final repo = await _pump(tester);

    await tester.tap(find.byTooltip('Apagar modelo').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Cancelar'));
    await tester.pumpAndSettle();

    expect(repo.deleted, isEmpty);
  });

  testWidgets('quem não é administrador vê a lista sem botão de apagar', (
    tester,
  ) async {
    await _pump(tester, role: UserRole.operator);

    expect(find.byTooltip('Apagar modelo'), findsNothing);
    expect(
      find.text('Apenas administradores podem apagar modelos.'),
      findsOneWidget,
    );
  });

  test('o modelo do Ollama traz a data, o uso e o arquivo dividido', () {
    final item = AiProviderModelItem.fromOllamaInstalled(<String, dynamic>{
      'name': 'qwen3:8b',
      'size': 10,
      'modifiedAt': '2026-01-02T03:04:05Z',
      'inUse': true,
      'sharedWith': <String>['qwen3:latest'],
    });
    expect(item.inUse, isTrue);
    expect(item.modifiedAt?.toUtc(), DateTime.utc(2026, 1, 2, 3, 4, 5));
    expect(item.sharedWith, <String>['qwen3:latest']);
  });
}

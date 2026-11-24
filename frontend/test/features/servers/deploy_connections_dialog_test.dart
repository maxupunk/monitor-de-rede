import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/pages/deploys_page.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_dialogs.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

DeployGithubConnectionView _connection({required bool installed}) =>
    DeployGithubConnectionView(
      id: 7,
      account: 'maxupunk',
      appSlug: 'netmonitor-3f9a1c',
      installed: installed,
      canWrite: true,
      installUrl: 'https://github.com/apps/netmonitor-3f9a1c/installations/new',
      settingsUrl: 'https://github.com/settings/apps/netmonitor-3f9a1c',
      createdAt: '2026-11-24T12:00:00Z',
    );

class _GithubDeploysNotifier extends DeploysNotifier {
  _GithubDeploysNotifier(this._connections);

  final List<DeployGithubConnectionView> _connections;
  final List<int> removed = <int>[];

  @override
  DeploysState build() => DeploysState(
    overview: DeployOverview(
      applications: const <DeployApplicationView>[],
      recentRuns: const <DeployRunView>[],
      counts: const DeployCounts(
        applications: 0,
        targets: 0,
        succeeded24h: 0,
        failed24h: 0,
      ),
      credentials: const <DeployCredentialView>[],
      githubConnections: _connections,
      hookPath: '/api/deploys/hook',
      oidcAudience: 'netmonitor',
      guide: const DeployGuideView(complete: true, steps: <DeployGuideStep>[]),
    ),
  );

  @override
  Future<void> removeGithub(int id) async => removed.add(id);
}

/// O retorno do GitHub aceito pela central (o `code` trocado pela chave).
class _CallbackDeploysNotifier extends _GithubDeploysNotifier {
  _CallbackDeploysNotifier() : super(const <DeployGithubConnectionView>[]);

  final List<(String, String)> connected = <(String, String)>[];
  final List<int> claimed = <int>[];

  @override
  Future<bool> fetchOverview() async => true;

  @override
  Future<void> fetchRecentBuilds() async {}

  @override
  Future<DeployGithubConnectionView> connectGithub(
    String code,
    String callbackState,
  ) async {
    connected.add((code, callbackState));
    return _connection(installed: true);
  }

  @override
  Future<DeployGithubConnectionView> claimGithubInstallation(
    int installationId,
  ) async {
    claimed.add(installationId);
    return _connection(installed: true);
  }
}

class _RoleAuthNotifier extends AuthNotifier {
  _RoleAuthNotifier(this._role);

  final UserRole _role;

  @override
  AuthState build() => AuthState(
    token: 'jwt',
    initialized: true,
    user: AuthUser(id: 1, email: 'u@netmonitor.local', role: _role),
  );
}

Future<_CallbackDeploysNotifier> _openCallback(
  WidgetTester tester,
  Map<String, String> query, {
  UserRole role = UserRole.admin,
}) async {
  tester.view.physicalSize = const Size(1280, 1600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final notifier = _CallbackDeploysNotifier();
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        authProvider.overrideWith(() => _RoleAuthNotifier(role)),
        deploysProvider.overrideWith(() => notifier),
      ],
      child: MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(body: DeploysPage(queryParams: query)),
      ),
    ),
  );
  await tester.pumpAndSettle();
  return notifier;
}

Future<_GithubDeploysNotifier> _openDialog(
  WidgetTester tester,
  List<DeployGithubConnectionView> connections,
) async {
  tester.view.physicalSize = const Size(1280, 1600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final notifier = _GithubDeploysNotifier(connections);
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[deploysProvider.overrideWith(() => notifier)],
      child: MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              onPressed: () => showDeployConnectionsDialog(context),
              child: const Text('abrir'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('abrir'));
  await tester.pumpAndSettle();
  return notifier;
}

void main() {
  group('Conexões com o GitHub', () {
    testWidgets('sem conexão: mostra os passos e o botão de conectar', (
      tester,
    ) async {
      await _openDialog(tester, const <DeployGithubConnectionView>[]);

      expect(find.text('Conectar GitHub App'), findsOneWidget);
      expect(find.textContaining('Create GitHub App'), findsOneWidget);
      expect(find.text('Conectado'), findsNothing);
    });

    testWidgets(
      'conectada e instalada: diz "Conectado" e recolhe o formulário',
      (tester) async {
        await _openDialog(tester, <DeployGithubConnectionView>[
          _connection(installed: true),
        ]);

        expect(find.text('maxupunk'), findsOneWidget);
        expect(find.text('Conectado'), findsOneWidget);
        expect(find.text('Lê e abre PR'), findsOneWidget);
        expect(find.text('Conectar GitHub App'), findsNothing);
        expect(find.text('Instalar nos repositórios'), findsNothing);

        await tester.tap(find.text('Conectar outra conta ou organização'));
        await tester.pumpAndSettle();
        expect(find.text('Conectar GitHub App'), findsOneWidget);
      },
    );

    testWidgets('criada sem instalar: o próximo passo vem em destaque', (
      tester,
    ) async {
      await _openDialog(tester, <DeployGithubConnectionView>[
        _connection(installed: false),
      ]);

      expect(find.text('Falta instalar'), findsOneWidget);
      expect(find.text('Instalar nos repositórios'), findsOneWidget);
      expect(find.text('Conectar GitHub App'), findsNothing);
    });

    testWidgets('remover pede confirmação', (tester) async {
      final notifier = await _openDialog(tester, <DeployGithubConnectionView>[
        _connection(installed: true),
      ]);

      await tester.tap(find.byTooltip('Remover conexão'));
      await tester.pumpAndSettle();
      expect(find.text('Remover a conexão com maxupunk?'), findsOneWidget);
      await tester.tap(find.text('Cancelar'));
      await tester.pumpAndSettle();
      expect(notifier.removed, isEmpty);

      await tester.tap(find.byTooltip('Remover conexão'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Remover'));
      await tester.pumpAndSettle();
      expect(notifier.removed, <int>[7]);
    });
  });

  group('Retorno do GitHub na página de deploys', () {
    testWidgets(
      '?code=&state=: conecta e confirma, com atalho para a conexão',
      (tester) async {
        final notifier = await _openCallback(tester, <String, String>{
          'code': 'abc123',
          'state': 'estado-1',
        });

        expect(notifier.connected, <(String, String)>[('abc123', 'estado-1')]);
        expect(find.text('GitHub App de maxupunk conectado.'), findsOneWidget);

        await tester.tap(find.text('Ver conexão'));
        await tester.pumpAndSettle();
        expect(find.text('Conexões com o GitHub'), findsOneWidget);
      },
    );

    testWidgets('?installation_id=: vincula a instalação e confirma', (
      tester,
    ) async {
      final notifier = await _openCallback(tester, <String, String>{
        'installation_id': '987',
        'setup_action': 'install',
      });

      expect(notifier.claimed, <int>[987]);
      expect(
        find.text('GitHub App instalado em maxupunk: conexão pronta.'),
        findsOneWidget,
      );
    });

    testWidgets('quem não é admin de Servidores recebe a explicação', (
      tester,
    ) async {
      final notifier = await _openCallback(tester, <String, String>{
        'code': 'abc123',
        'state': 'estado-1',
      }, role: UserRole.viewer);

      expect(notifier.connected, isEmpty);
      expect(
        find.textContaining('só é concluído por um administrador'),
        findsOneWidget,
      );
    });
  });
}

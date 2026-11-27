import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/deploy_presentation.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_dialogs.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

final String _latestSha = 'c' * 40;
final String _olderSha = 'b' * 40;
final String _liveSha = 'a' * 40;

DeployTargetView _target({String reference = 'main', DeployRunView? live}) =>
    DeployTargetView(
      id: 7,
      applicationId: 1,
      name: 'Produção',
      project: 'vx-academia',
      reference: reference,
      requiresApproval: false,
      waitTimeoutSecs: 180,
      autoRollback: true,
      keepImages: 3,
      enabled: true,
      position: 0,
      vulnerabilityPolicy: 'block_critical',
      requireProvenance: false,
      live: live,
    );

DeployRunView _live() => DeployRunView(
  id: 3,
  applicationId: 1,
  applicationName: 'VX Academia',
  repository: 'master/vx-academia',
  targetId: 7,
  targetName: 'Produção',
  trigger: 'manual',
  commitSha: _liveSha,
  shortSha: 'aaaaaaa',
  imageTag: _liveSha,
  status: 'succeeded',
  createdAt: '2026-11-25T12:00:00Z',
);

DeployBranchCommitsView _listed() => DeployBranchCommitsView(
  branch: 'main',
  commits: <DeployCommitView>[
    DeployCommitView(
      sha: _latestSha,
      shortSha: 'ccccccc',
      message: 'feat: matrícula online\n\ncorpo',
      author: 'Maxuel',
    ),
    DeployCommitView(
      sha: _olderSha,
      shortSha: 'bbbbbbb',
      message: 'fix: login',
    ),
  ],
);

/// O GitHub do diálogo: os commits que a central devolveria e o que o
/// "Publicar" enviou — sem rede.
class _PublishDeploysNotifier extends DeploysNotifier {
  _PublishDeploysNotifier(this._commits);

  final DeployBranchCommitsView _commits;
  int listed = 0;
  final List<String?> published = <String?>[];

  @override
  DeploysState build() => const DeploysState();

  @override
  Future<DeployBranchCommitsView> targetCommits(int targetId) async {
    listed++;
    return _commits;
  }

  @override
  Future<DeployRunView> triggerDeploy(
    int targetId, {
    String? commitSha,
    String? imageTag,
    required String when,
  }) async {
    published.add(commitSha);
    throw StateError('recusado no teste');
  }
}

Future<_PublishDeploysNotifier> _open(
  WidgetTester tester,
  DeployTargetView target,
  DeployBranchCommitsView commits,
) async {
  tester.view.physicalSize = const Size(1280, 1600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final notifier = _PublishDeploysNotifier(commits);
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[deploysProvider.overrideWith(() => notifier)],
      child: MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              onPressed: () => showDeployPublishDialog(context, target: target),
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

FilledButton _publishButton(WidgetTester tester) => tester.widget<FilledButton>(
  find.ancestor(
    of: find.text('Publicar'),
    matching: find.byWidgetPredicate((widget) => widget is FilledButton),
  ),
);

void main() {
  group('publishEmptyCommitPlan', () {
    test('diz o que o vazio faz conforme o alvo', () {
      expect(
        publishEmptyCommitPlan(reference: 'main', liveShortSha: 'abc1234').hint,
        contains('reaplica a versão no ar (abc1234)'),
      );
      final latest = publishEmptyCommitPlan(
        reference: 'refs/heads/main',
        latestShortSha: 'ccccccc',
      );
      expect(latest.hint, contains('último commit de main (ccccccc)'));
      expect(latest.allowed, isTrue);
      final tag = publishEmptyCommitPlan(reference: 'v*');
      expect(tag.allowed, isFalse);
      expect(tag.hint, contains('acompanha tags (v*)'));
      expect(
        publishEmptyCommitPlan(
          reference: 'main',
          latestUnavailable: true,
        ).allowed,
        isFalse,
      );
    });
  });

  group('Publicar em produção', () {
    testWidgets(
      'alvo nunca publicado: o vazio publica o último commit da main e a lista '
      'deixa escolher outro',
      (tester) async {
        final notifier = await _open(tester, _target(), _listed());

        expect(notifier.listed, 1, reason: 'uma consulta só, ao abrir');
        expect(
          find.text('Vazio publica o último commit de main (ccccccc).'),
          findsOneWidget,
        );
        expect(find.text('Mais recente'), findsOneWidget);
        expect(find.text('ccccccc · feat: matrícula online'), findsOneWidget);
        expect(_publishButton(tester).onPressed, isNotNull);

        await tester.tap(find.text('bbbbbbb · fix: login'));
        await tester.pumpAndSettle();
        expect(find.text(_olderSha), findsOneWidget);

        await tester.tap(find.text('Publicar'));
        await tester.pumpAndSettle();
        expect(notifier.published, <String?>[_olderSha]);
        expect(notifier.listed, 1);
      },
    );

    testWidgets('com versão no ar: o vazio a reaplica', (tester) async {
      await _open(tester, _target(live: _live()), _listed());

      expect(
        find.text('Vazio reaplica a versão no ar (aaaaaaa) com o .env atual.'),
        findsOneWidget,
      );
      expect(_publishButton(tester).onPressed, isNotNull);
    });

    testWidgets(
      'sem acesso ao GitHub: o motivo aparece e o Publicar espera o commit',
      (tester) async {
        const reason =
            'A aplicação não tem acesso ao GitHub (conexão do GitHub App ou '
            'token) para descobrir o último commit de main';
        await _open(
          tester,
          _target(),
          const DeployBranchCommitsView(
            branch: 'main',
            commits: <DeployCommitView>[],
            unavailableReason: reason,
          ),
        );

        expect(find.text(reason), findsOneWidget);
        expect(_publishButton(tester).onPressed, isNull);

        await tester.enterText(find.byType(TextField), _latestSha);
        await tester.pumpAndSettle();
        expect(_publishButton(tester).onPressed, isNotNull);
      },
    );
  });
}

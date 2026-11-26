import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/utils/stacked_usage.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy/deploy_repository_picker.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  group('Docker protected container security rules (ADR 014)', () {
    DockerContainerSummary makeContainer({
      String state = 'running',
      bool protectedFlag = false,
      Map<String, String> labels = const <String, String>{},
    }) {
      return DockerContainerSummary(
        id: 'abcdef1234567890',
        names: const <String>['/netmonitor'],
        image: 'netmonitor:latest',
        imageId: 'sha256:1234',
        state: state,
        status: 'Up 1h',
        labels: labels,
        ports: const <DockerPortBinding>[],
        created: 1700000000,
        projectName: 'netmonitor',
        protected: protectedFlag,
      );
    }

    test(
      'running container with protected=true allows ZERO destructive actions',
      () {
        final actions = allowedContainerActions(
          container: makeContainer(state: 'running', protectedFlag: true),
          canLifecycle: true,
          canUpdate: true,
        );
        expect(actions, isEmpty);
      },
    );

    test(
      'running container with com.netmonitor.protected=true label blocks stop/restart/update/remove',
      () {
        final container = makeContainer(
          state: 'running',
          protectedFlag: false,
          labels: const <String, String>{'com.netmonitor.protected': 'true'},
        );
        expect(container.isProtected, isTrue);
        final actions = allowedContainerActions(
          container: container,
          canLifecycle: true,
          canUpdate: true,
        );
        expect(actions, isEmpty);
      },
    );

    test('stopped protected container ONLY allows start', () {
      final actions = allowedContainerActions(
        container: makeContainer(state: 'exited', protectedFlag: true),
        canLifecycle: true,
        canUpdate: true,
      );
      expect(
        actions.map((a) => a.key),
        equals(<DockerContainerActionKey>[DockerContainerActionKey.start]),
      );
    });

    test(
      'normal running container with lifecycle+update allows stop, restart, update, remove',
      () {
        final actions = allowedContainerActions(
          container: makeContainer(state: 'running', protectedFlag: false),
          canLifecycle: true,
          canUpdate: true,
        );
        expect(
          actions.map((a) => a.key),
          equals(<DockerContainerActionKey>[
            DockerContainerActionKey.stop,
            DockerContainerActionKey.restart,
            DockerContainerActionKey.update,
            DockerContainerActionKey.remove,
          ]),
        );
      },
    );
  });

  group('Lossless .env parser & serializer', () {
    test(
      'round-trips comments, blank lines, export prefix and values losslessly',
      () {
        const original =
            '# Database settings\n'
            'export DB_HOST=127.0.0.1\n'
            'DB_PASSWORD=s3cr=t==val\n'
            '\n'
            'APP_ENV=production\n';
        final parsed = parseEnvFile(original);
        expect(parsed.length, 5);
        expect(parsed[0].isVariable, isFalse);
        expect(parsed[1].isVariable, isTrue);
        expect(parsed[1].exported, isTrue);
        expect(parsed[1].key, 'DB_HOST');
        expect(parsed[2].value, 's3cr=t==val');
        expect(looksLikeSecretKey(parsed[2].key), isTrue);
        expect(looksLikeSecretKey(parsed[4].key), isFalse);
        expect(serializeEnvFile(parsed), equals(original));

        // copyWithVariable preserves stable uid for ValueKey identity
        final originalUid = parsed[1].uid;
        final edited = parsed[1].copyWithVariable(value: '10.8.0.1');
        expect(edited.uid, equals(originalUid));
        expect(edited.exported, isTrue);
        expect(edited.value, equals('10.8.0.1'));
      },
    );

    test('DeploySetupInput and DeployCredentialInput serialize cleanly', () {
      const cred = DeployCredentialInput(
        name: 'ghcr-pat',
        registry: 'ghcr.io',
        username: 'bot',
        secret: 'ghp_123',
      );
      expect(cred.toJson()['name'], 'ghcr-pat');
      expect(cred.toJson()['registry'], 'ghcr.io');

      const setup = DeploySetupInput(
        application: DeployApplicationInput(
          name: 'ERP',
          slug: 'erp',
          repository: 'org/erp',
          composeFiles: <String>['docker-compose.yml'],
          imageTagVariable: 'IMAGE_TAG',
        ),
        target: DeployTargetInput(
          name: 'Prod',
          probeId: 2,
          project: 'erp',
          reference: 'main',
        ),
      );
      final json = setup.toJson();
      expect((json['application'] as Map<String, dynamic>)['slug'], 'erp');
      expect((json['target'] as Map<String, dynamic>)['probeId'], 2);
    });
  });

  group('Deploy & Build stage transitions', () {
    test('rolled_back deploy marks start failed and rollback done', () {
      final states = computeDeployStageStates('rollback', 'rolled_back');
      expect(states['sync'], 'done');
      expect(states['start'], 'failed');
      expect(states['prune'], 'todo');
      expect(states['rollback'], 'done');
    });

    test(
      'running build marks earlier stages done and current stage current',
      () {
        final states = computeBuildStageStates('build', 'running');
        expect(states['prepare'], 'done');
        expect(states['clone'], 'done');
        expect(states['test'], 'done');
        expect(states['build'], 'current');
        expect(states['store'], 'todo');
        expect(states['scan'], 'todo');
      },
    );
  });

  group('Registry & Network helpers', () {
    test('detects 40-hex immutable commit tags and formats short tag', () {
      const sha = 'a1b2c3d4e5f60718293a4b5c6d7e8f9012345678';
      expect(isCommitTag(sha), isTrue);
      expect(shortRegistryTag(sha), 'a1b2c3d');
      expect(isCommitTag('v1.2.3'), isFalse);
      expect(shortRegistryTag('v1.2.3'), 'v1.2.3');
    });

    test('formats registry scan summary accurately', () {
      expect(
        registryScanSentence(
          const RegistryScanSummary(status: 'done', critical: 2, high: 1),
        ),
        '2 críticas e 1 alta',
      );
      expect(
        registryScanSentence(const RegistryScanSummary(status: 'done')),
        'Nenhuma vulnerabilidade conhecida',
      );
    });

    test('identifies private RFC1918 / loopback / CGNAT hosts', () {
      expect(isPrivateHost('10.8.0.1'), isTrue);
      expect(isPrivateHost('192.168.1.50'), isTrue);
      expect(isPrivateHost('172.20.0.5'), isTrue);
      expect(isPrivateHost('100.64.0.2'), isTrue);
      expect(isPrivateHost('localhost'), isTrue);
      expect(isPrivateHost('monitor.empresa.com.br'), isFalse);
    });
  });

  group('Stacked container usage & human Y-axis scale', () {
    test('niceAxisScale produces clean human steps for MiB and %', () {
      // 763 MiB -> 800 MiB with 200 MiB intervals (never 191/381/572/763 MiB)
      const mib = 1024 * 1024;
      final memScale = niceAxisScale(763 * mib.toDouble(), base: 1024);
      expect(memScale.maxY, equals(800 * mib.toDouble()));
      expect(memScale.interval, equals(200 * mib.toDouble()));
      expect(memScale.divisions, equals(4));

      // 5.2% -> 8% with 2% intervals
      final cpuScale = niceAxisScale(5.2, base: 10);
      expect(cpuScale.maxY, equals(8.0));
      expect(cpuScale.interval, equals(2.0));

      // 0.3% with minCeiling 1.0 -> 1.0% with 0.25% intervals
      final tinyCpu = niceAxisScale(0.3, base: 10, minCeiling: 1.0);
      expect(tinyCpu.maxY, equals(1.0));
      expect(tinyCpu.interval, equals(0.25));
    });

    test(
      'buildStackedUsage ranks top series, stabilizes colors, and aggregates Outros',
      () {
        final samples = <UsageSample>[
          const UsageSample(
            time: '2026-11-21T16:00:00Z',
            values: <String, double>{'a': 10, 'b': 30, 'c': 20, 'd': 2, 'e': 1},
          ),
          const UsageSample(
            time: '2026-11-21T16:00:05Z',
            values: <String, double>{'a': 40, 'b': 15, 'c': 5, 'd': 3, 'e': 2},
          ),
        ];
        const labels = <String, String>{
          'a': 'api',
          'b': 'postgres',
          'c': 'nginx',
          'd': 'redis',
          'e': 'worker',
        };

        final stacked = buildStackedUsage(samples, labels, maxSeries: 2);
        // Top 2 by sum: a (50), b (45); rest: c, d, e -> Outros (3)
        expect(stacked.layers.length, 3);
        expect(stacked.layers[0].id, 'a');
        expect(stacked.layers[0].color, kStackedChartPalette[0]);
        expect(stacked.layers[1].id, 'b');
        expect(stacked.layers[1].color, kStackedChartPalette[1]);
        expect(stacked.layers[2].id, kOthersLayerId);
        expect(stacked.layers[2].label, 'Outros (3)');
        expect(stacked.layers[2].color, kOthersLayerColor);

        // Cumulative stacking sums to exact sample totals
        expect(stacked.totals, equals(<double>[63.0, 65.0]));
        expect(stacked.peak, equals(65.0));
        expect(stacked.layers[1].lower, equals(<double>[10.0, 40.0]));
        expect(stacked.layers[1].upper, equals(<double>[40.0, 55.0]));
        expect(stacked.layers[2].upper, equals(<double>[63.0, 65.0]));
      },
    );
  });

  group('Local storage root formatting & subfolder validation', () {
    test(
      'formatLocalStorageRoot always guarantees trailing slash on prefix',
      () {
        expect(
          formatLocalStorageRoot('/data/storage'),
          equals((label: '/data/storage', prefix: '/data/storage/')),
        );
        expect(
          formatLocalStorageRoot('/data/storage/'),
          equals((label: '/data/storage', prefix: '/data/storage/')),
        );
        expect(
          formatLocalStorageRoot('storage/'),
          equals((label: 'storage', prefix: 'storage/')),
        );
        expect(
          formatLocalStorageRoot('storage'),
          equals((label: 'storage', prefix: 'storage/')),
        );
        expect(formatLocalStorageRoot('/'), equals((label: '/', prefix: '/')));
        expect(
          formatLocalStorageRoot(r'C:\backups\'),
          equals((label: 'C:/backups', prefix: 'C:/backups/')),
        );
        expect(
          formatLocalStorageRoot('   '),
          equals((label: 'storage', prefix: 'storage/')),
        );
      },
    );

    test(
      'validateLocalBasePath accepts relative subfolders and rejects escapes',
      () {
        expect(validateLocalBasePath('', rootLabel: '/data/storage'), isNull);
        expect(
          validateLocalBasePath('databases-local', rootLabel: '/data/storage'),
          isNull,
        );
        expect(
          validateLocalBasePath('bancos/producao', rootLabel: '/data/storage'),
          isNull,
        );
        expect(
          validateLocalBasePath('/databases-local', rootLabel: '/data/storage'),
          contains('/data/storage/'),
        );
        expect(
          validateLocalBasePath(
            r'\databases-local',
            rootLabel: '/data/storage',
          ),
          contains('/data/storage/'),
        );
        expect(
          validateLocalBasePath('../data', rootLabel: '/data/storage'),
          contains('..'),
        );
        expect(
          validateLocalBasePath('a/../b', rootLabel: '/data/storage'),
          contains('..'),
        );
        expect(
          validateLocalBasePath('C:/Windows', rootLabel: '/data/storage'),
          isNotNull,
        );
      },
    );
  });

  group('Database backup card live status & error visibility', () {
    DatabaseConnectionResponse makeConn({
      String? lastBackupStatus = 'failed',
      String? lastBackupError = 'password authentication failed',
      String? lastBackupAt = '2026-11-26T12:00:00Z',
      bool backupEnabled = true,
    }) {
      return DatabaseConnectionResponse(
        id: 1,
        name: 'sgc-v4-prod-postgres',
        engine: 'postgres',
        host: '172.19.0.6',
        port: 5432,
        username: 'sgc4user',
        passwordSet: true,
        sslMode: 'prefer',
        databases: const <String>['app'],
        storageDestinationId: 10,
        storageDestinationName: 'backups-database',
        backupEnabled: backupEnabled,
        backupIntervalHours: 24,
        backupRetention: 7,
        lastBackupAt: lastBackupAt,
        lastBackupStatus: lastBackupStatus,
        lastBackupError: lastBackupError,
      );
    }

    test('shows persisted failure when no live job exists', () {
      final pres = resolveDatabaseBackupCardPresentation(makeConn(), null);
      expect(pres.effectiveStatus, 'failed');
      expect(pres.iconTone, SemanticTone.error);
      expect(pres.badgeLabel, 'Último falhou');
      expect(pres.showLastBackupError, isTrue);
      expect(pres.showBackupJobBanner, isFalse);
    });

    test(
      'hides stale failure and switches to Em andamento when backup starts',
      () {
        const runningJob = DatabaseJobSnapshot(
          id: 'job-2',
          kind: 'backup',
          connectionId: 1,
          database: 'app',
          stage: 'Dados de public.conversation_messages',
          rows: 18463,
          bytes: 7738490,
          databasesDone: 0,
          databasesTotal: 1,
          status: 'running',
          startedAt: '2026-11-26T12:01:00Z',
        );

        final pres = resolveDatabaseBackupCardPresentation(
          makeConn(),
          runningJob,
        );
        expect(pres.effectiveStatus, 'running');
        expect(pres.iconTone, SemanticTone.info);
        expect(pres.badgeLabel, 'Em andamento');
        expect(pres.badgeTone, SemanticTone.info);
        expect(pres.showLastBackupError, isFalse);
        expect(pres.showBackupJobBanner, isTrue);
      },
    );

    test(
      'hides stale failure immediately when live job succeeds before refetch',
      () {
        const successJob = DatabaseJobSnapshot(
          id: 'job-2',
          kind: 'backup',
          connectionId: 1,
          database: 'app',
          stage: 'Concluído',
          rows: 18463,
          bytes: 7738490,
          databasesDone: 1,
          databasesTotal: 1,
          status: 'success',
          message: 'app copiado para backups-database',
          startedAt: '2026-11-26T12:01:00Z',
          finishedAt: '2026-11-26T12:01:15Z',
        );

        final pres = resolveDatabaseBackupCardPresentation(
          makeConn(),
          successJob,
        );
        expect(pres.effectiveStatus, 'success');
        expect(pres.iconTone, SemanticTone.success);
        expect(pres.badgeLabel, 'A cada 24h');
        expect(pres.showLastBackupError, isFalse);
        expect(pres.showBackupJobBanner, isTrue);
      },
    );

    test(
      'avoids duplicating error box when failed live job already shows message',
      () {
        const failedJob = DatabaseJobSnapshot(
          id: 'job-2',
          kind: 'backup',
          connectionId: 1,
          database: 'app',
          stage: 'Falhou',
          rows: 0,
          bytes: 0,
          databasesDone: 0,
          databasesTotal: 1,
          status: 'failed',
          message: 'password authentication failed',
          startedAt: '2026-11-26T12:00:00Z',
          finishedAt: '2026-11-26T12:00:02Z',
        );

        final pres = resolveDatabaseBackupCardPresentation(
          makeConn(),
          failedJob,
        );
        expect(pres.effectiveStatus, 'failed');
        expect(pres.showBackupJobBanner, isTrue);
        expect(pres.showLastBackupError, isFalse);
      },
    );

    test(
      'falls back to persisted failure if a later scheduled failure superseded an old job',
      () {
        const olderSuccessJob = DatabaseJobSnapshot(
          id: 'job-1',
          kind: 'backup',
          connectionId: 1,
          database: 'app',
          stage: 'Concluído',
          rows: 100,
          bytes: 1024,
          databasesDone: 1,
          databasesTotal: 1,
          status: 'success',
          message: 'app copiado',
          startedAt: '2026-11-26T10:00:00Z',
          finishedAt: '2026-11-26T10:00:10Z',
        );

        final pres = resolveDatabaseBackupCardPresentation(
          makeConn(lastBackupAt: '2026-11-26T12:00:00Z'),
          olderSuccessJob,
        );
        expect(pres.effectiveStatus, 'failed');
        expect(pres.showBackupJobBanner, isFalse);
        expect(pres.showLastBackupError, isTrue);
      },
    );
  });

  group('Deploy repository picker filtering', () {
    const repos = <DeployRepositoryView>[
      DeployRepositoryView(
        fullName: 'maxupunk/sgc-v4',
        isPrivate: true,
        defaultBranch: 'production',
        description: 'Sistema de gestão comercial',
      ),
      DeployRepositoryView(
        fullName: 'maxupunk/site-institucional',
        isPrivate: false,
        defaultBranch: 'main',
        description: 'Landing page em Next.js',
      ),
    ];

    test(
      'matches by repository fullName and description case-insensitively',
      () {
        expect(filterDeployRepositories(repos, '').length, 2);
        expect(
          filterDeployRepositories(repos, 'SGC').map((r) => r.fullName),
          equals(<String>['maxupunk/sgc-v4']),
        );
        expect(
          filterDeployRepositories(repos, 'next.js').map((r) => r.fullName),
          equals(<String>['maxupunk/site-institucional']),
        );
        expect(filterDeployRepositories(repos, 'inexistente'), isEmpty);
      },
    );
  });
}

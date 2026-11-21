import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';

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

    test('running container with com.netmonitor.protected=true label blocks stop/restart/update/remove', () {
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
    });

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

    test('normal running container with lifecycle+update allows stop, restart, update, remove', () {
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
    });
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
      },
    );
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
}

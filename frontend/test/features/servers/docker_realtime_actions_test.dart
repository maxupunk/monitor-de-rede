import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/pages/docker_compose_page.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/utils/docker_presentation.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const String _host = 'agent-9';

final ApiClient _unusedApi = ApiClient(
  baseUrlProvider: () => 'http://central.invalid/api',
  tokenProvider: () => null,
  onUnauthorized: () {},
);

Map<String, String> _composeLabels(String project, String service) =>
    <String, String>{
      'com.docker.compose.project': project,
      'com.docker.compose.project.working_dir': '/stacks/$project',
      'com.docker.compose.project.config_files': 'compose.yml, override.yml',
      'com.docker.compose.service': service,
    };

Map<String, dynamic> _containerJson(
  String id,
  String state,
  Map<String, String> labels,
) => <String, dynamic>{
  'id': id,
  'names': <String>['/$id'],
  'image': 'app:1',
  'imageId': 'sha256:$id',
  'state': state,
  'status': state == 'running' ? 'Up 1 minute' : 'Exited (0)',
  'labels': labels,
  'ports': const <dynamic>[],
  'created': 1700000000,
  'projectName': labels['com.docker.compose.project'],
  'protected': false,
};

DockerContainerSummary _container(
  String id,
  String state,
  Map<String, String> labels,
) => DockerContainerSummary.fromJson(_containerJson(id, state, labels));

/// API do host que registra cada chamada e recusa qualquer releitura.
class _RecordingDockerHostApi extends DockerHostApi {
  _RecordingDockerHostApi({this.stopError}) : super(_unusedApi, _host);

  final Object? stopError;
  final List<String> calls = <String>[];

  @override
  Future<void> stopContainer(String id) async {
    calls.add('stop $id');
    final error = stopError;
    if (error != null) throw error;
  }

  @override
  Future<List<DockerContainerSummary>> containers() async {
    calls.add('containers');
    throw const NetworkException();
  }

  @override
  Future<DockerStatusResponse> status() async {
    calls.add('status');
    throw const NetworkException();
  }

  @override
  Future<DockerMetricsResponse> metrics() async {
    calls.add('metrics');
    throw const NetworkException();
  }
}

class _ActionDockerNotifier extends DockerNotifier {
  _ActionDockerNotifier(this.api, {this.seed = const DockerState()});

  final _RecordingDockerHostApi api;
  final DockerState seed;

  @override
  DockerState build() => seed;

  @override
  DockerHostApi apiFor([String? hostKey]) => api;
}

class _AdminAuthNotifier extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'jwt',
    initialized: true,
    user: AuthUser(
      id: 1,
      email: 'admin@netmonitor.local',
      role: UserRole.admin,
    ),
  );
}

DockerState _liveState(List<DockerContainerSummary> containers) => DockerState(
  hostsLoaded: true,
  selectedHostKey: _host,
  hosts: const <DockerHostView>[
    DockerHostView(
      key: _host,
      name: 'hostinger-srv-1',
      kind: 'agent',
      online: true,
      agentId: 9,
      dockerAvailable: true,
      composeAvailable: true,
      policy: <String>['read', 'lifecycle', 'compose'],
    ),
  ],
  hostStates: <String, DockerHostState>{
    _host: DockerHostState(
      status: const DockerStatusResponse(available: true),
      containers: containers,
    ),
  },
);

void main() {
  group('composeProjectsOf', () {
    test('agrupa pelas labels do compose, como a central', () {
      final projects = composeProjectsOf(<DockerContainerSummary>[
        _container('api-1', 'running', _composeLabels('nextjus', 'api')),
        _container('jobs-1', 'exited', _composeLabels('nextjus', 'jobs')),
        _container('api-2', 'running', _composeLabels('nextjus', 'api')),
        _container('avulso', 'running', const <String, String>{}),
        _container('sem-dir', 'running', <String, String>{
          'com.docker.compose.project': 'sem-dir',
        }),
      ]);

      expect(projects, hasLength(1), reason: 'sem diretório não há onde rodar');
      final project = projects.single;
      expect(project.name, 'nextjus');
      expect(project.workingDir, '/stacks/nextjus');
      expect(project.configFiles, <String>['compose.yml', 'override.yml']);
      expect(project.containers, 3);
      expect(project.running, 2);
      expect(project.services.map((s) => s.name), <String>['api', 'jobs']);
      expect(project.services.first.containers, 2);
      expect(project.services.last.running, 0);
    });
  });

  group('Ação em container', () {
    test(
      'a ação concluída não relê a lista por HTTP: o SSE traz o estado',
      () async {
        final api = _RecordingDockerHostApi();
        final container = ProviderContainer(
          overrides: <Override>[
            dockerProvider.overrideWith(() => _ActionDockerNotifier(api)),
          ],
        );
        addTearDown(container.dispose);

        final ok = await container
            .read(dockerProvider.notifier)
            .executeContainerAction(
              '692de2f68d60',
              DockerContainerActionKey.stop,
            );

        expect(ok, isTrue);
        expect(api.calls, <String>['stop 692de2f68d60']);
        expect(container.read(dockerProvider).error, isNull);
      },
    );

    test('sem resposta a tempo, a tela não afirma que a ação falhou', () async {
      final api = _RecordingDockerHostApi(
        stopError: NetworkException.timeout(dockerLifecycleTimeout),
      );
      final container = ProviderContainer(
        overrides: <Override>[
          dockerProvider.overrideWith(() => _ActionDockerNotifier(api)),
        ],
      );
      addTearDown(container.dispose);

      final ok = await container
          .read(dockerProvider.notifier)
          .executeContainerAction(
            '692de2f68d60',
            DockerContainerActionKey.stop,
          );

      expect(ok, isFalse);
      expect(
        container.read(dockerProvider).error,
        contains('pode ter sido concluída'),
      );
    });
  });

  testWidgets(
    'aba Projetos acompanha o snapshot ao vivo sem consultar o agente',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 1400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final api = _RecordingDockerHostApi();
      final notifier = _ActionDockerNotifier(
        api,
        seed: _liveState(<DockerContainerSummary>[
          _container(
            'backend-1',
            'running',
            _composeLabels('backup', 'backend'),
          ),
        ]),
      );
      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthNotifier.new),
            dockerProvider.overrideWith(() => notifier),
          ],
          child: MaterialApp(
            theme: AppTheme.light(),
            home: const Scaffold(
              body: SingleChildScrollView(child: DockerComposePage()),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('backup'), findsOneWidget);
      expect(find.text('1/1 em execução'), findsOneWidget);

      // O `stop` chegou pelo SSE: o cartão muda sozinho.
      notifier.applyLiveSnapshot(<String, dynamic>{
        'hostKey': _host,
        'status': <String, dynamic>{'available': true},
        'containers': <dynamic>[
          _containerJson(
            'backend-1',
            'exited',
            _composeLabels('backup', 'backend'),
          ),
        ],
      });
      await tester.pumpAndSettle();

      expect(find.text('0/1 em execução'), findsOneWidget);
      expect(api.calls, isEmpty, reason: 'nenhuma consulta HTTP ao agente');
    },
  );
}

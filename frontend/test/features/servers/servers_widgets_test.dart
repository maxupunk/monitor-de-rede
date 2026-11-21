import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/pages/docker_containers_page.dart';
import 'package:netmonitor_app/features/servers/pages/remote_servers_page.dart';
import 'package:netmonitor_app/features/servers/providers/agents_provider.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _AdminAuthNotifier extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'test-token',
    initialized: true,
    user: AuthUser(
      id: 1,
      email: 'admin@netmonitor.local',
      fullName: 'Admin',
      role: UserRole.admin,
    ),
  );
}

class _SeededDockerNotifier extends DockerNotifier {
  @override
  DockerState build() => const DockerState(
    hostsLoaded: true,
    selectedHostKey: 'agent-1',
    hosts: <DockerHostView>[
      DockerHostView(
        key: 'agent-1',
        name: 'Central',
        kind: 'local',
        online: true,
        agentId: 1,
        dockerAvailable: true,
        composeAvailable: true,
        policy: <String>['read', 'lifecycle', 'update', 'compose'],
      ),
    ],
    hostStates: <String, DockerHostState>{
      'agent-1': DockerHostState(
        status: DockerStatusResponse(available: true, engineVersion: '27.3.1'),
        containers: <DockerContainerSummary>[
          DockerContainerSummary(
            id: 'protected123456',
            names: <String>['/netmonitor-api'],
            image: 'netmonitor:latest',
            imageId: 'sha256:1',
            state: 'running',
            status: 'Up 3h',
            labels: <String, String>{'com.netmonitor.protected': 'true'},
            ports: <DockerPortBinding>[],
            created: 1700000000,
            projectName: 'netmonitor',
            protected: true,
          ),
          DockerContainerSummary(
            id: 'normal123456',
            names: <String>['/nginx-web'],
            image: 'nginx:alpine',
            imageId: 'sha256:2',
            state: 'running',
            status: 'Up 1h',
            labels: <String, String>{},
            ports: <DockerPortBinding>[],
            created: 1700000000,
            projectName: 'web',
            protected: false,
          ),
        ],
      ),
    },
  );
}

class _SeededAgentsNotifier extends AgentsNotifier {
  @override
  AgentsState build() => const AgentsState(
    loaded: true,
    agents: <AgentView>[
      AgentView(
        id: 1,
        name: 'Central Local',
        hostKey: 'agent-1',
        status: 'online',
        connected: true,
        local: true,
        host: AgentHostInfo(
          hostname: 'netmonitor-host',
          os: 'linux',
          arch: 'amd64',
          policy: <String>['read', 'lifecycle'],
          docker: AgentCapabilityView(available: true, version: '27.3.1'),
          compose: AgentCapabilityView(available: true, version: '2.29.0'),
        ),
      ),
    ],
  );

  @override
  Future<void> fetchAgents() async {}
}

void main() {
  testWidgets(
    'DockerContainersPage displays Protegido badge and blocks destructive buttons on protected container',
    (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthNotifier.new),
            dockerProvider.overrideWith(_SeededDockerNotifier.new),
          ],
          child: MaterialApp(
            theme: AppTheme.dark(),
            home: const Scaffold(
              body: SingleChildScrollView(
                padding: EdgeInsets.all(16),
                child: DockerContainersPage(),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('netmonitor-api'), findsOneWidget);
      expect(find.text('nginx-web'), findsOneWidget);
      expect(find.text('Protegido'), findsOneWidget);

      // Somente o container não-protegido (`nginx-web`) deve exibir botão Parar e menu de ações.
      expect(find.byTooltip('Parar'), findsOneWidget);
      expect(find.byTooltip('Ações do container'), findsOneWidget);
    },
  );

  testWidgets(
    'RemoteServersPage renders local central agent with Esta central badge and no revoke menu',
    (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthNotifier.new),
            agentsProvider.overrideWith(_SeededAgentsNotifier.new),
          ],
          child: MaterialApp(
            theme: AppTheme.dark(),
            home: const Scaffold(body: RemoteServersPage()),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Central Local'), findsOneWidget);
      expect(find.text('Esta central'), findsOneWidget);
      expect(find.text('Conectado'), findsOneWidget);
      // Agente local não pode ser revogado/removido pela UI (ADR 014).
      expect(find.byTooltip('Mais ações'), findsNothing);
    },
  );
}

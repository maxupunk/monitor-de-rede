import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/utils/database_presentation.dart';

DatabaseConnectionResponse _conn({
  String? status,
  String? error,
  DatabaseContainerLink? container,
  int? viaProbeId,
}) {
  return DatabaseConnectionResponse(
    id: 1,
    name: 'ERP',
    engine: 'postgres',
    host: '172.18.0.5',
    port: 5432,
    username: 'postgres',
    passwordSet: true,
    sslMode: 'prefer',
    databases: const <String>[],
    storageDestinationId: 1,
    viaProbeId: viaProbeId,
    backupEnabled: true,
    backupIntervalHours: 24,
    backupRetention: 7,
    lastBackupAt: '2026-11-26T03:00:00Z',
    lastBackupStatus: status,
    lastBackupError: error,
    container: container,
  );
}

DatabaseContainer _container(
  String name, {
  String engine = 'postgres',
  int? probeId,
  int port = 5432,
  bool running = true,
  int? existing,
}) {
  return DatabaseContainer(
    hostKey: 'agent-1',
    hostName: 'central',
    probeId: probeId,
    containerId: '$name-id',
    containerName: name,
    image: '$engine:16',
    engine: engine,
    running: running,
    port: port,
    existingConnectionId: existing,
  );
}

const _link = DatabaseContainerLink(hostKey: 'agent-1', name: 'erp-db-1');

void main() {
  group('Conexão que acompanha o contêiner', () {
    test('o vínculo vem da API e volta no formulário', () {
      final conn = DatabaseConnectionResponse.fromJson(<String, dynamic>{
        'id': 3,
        'name': 'ERP',
        'container': <String, dynamic>{
          'hostKey': 'agent-1',
          'name': 'erp-db-1',
          'project': 'erp',
          'service': 'db',
        },
        'containerHostName': 'central',
      });
      expect(conn.container?.name, 'erp-db-1');
      expect(conn.container?.service, 'db');
      expect(conn.containerHostName, 'central');
      expect(
        DatabaseConnectionResponse.fromJson(<String, dynamic>{
          'id': 4,
        }).container,
        isNull,
      );

      final input = DatabaseConnectionInput(
        name: 'ERP',
        engine: 'postgres',
        host: 'erp-db-1',
        port: 5432,
        username: 'postgres',
        sslMode: 'prefer',
        databases: const <String>[],
        backupEnabled: false,
        backupIntervalHours: 24,
        backupRetention: 7,
        container: conn.container,
      );
      expect(input.toJson()['container'], <String, dynamic>{
        'hostKey': 'agent-1',
        'name': 'erp-db-1',
        'project': 'erp',
        'service': 'db',
      });
    });

    test('o contêiner já protegido é lido pelo campo que o backend manda', () {
      final container = DatabaseContainer.fromJson(<String, dynamic>{
        'hostKey': 'agent-1',
        'containerName': 'erp-db-1',
        'service': 'db',
        'port': 5432,
        'protectedConnectionId': 9,
        'protectedConnectionName': 'ERP',
      });
      expect(container.existingConnectionId, 9);
      expect(container.existingConnectionName, 'ERP');
      expect(container.service, 'db');
    });

    test('falha de rede sugere vincular ou reencontrar; outras não', () {
      DatabaseAddressAdvice advice(DatabaseConnectionResponse conn) =>
          resolveDatabaseAddressAdvice(
            conn,
            resolveDatabaseBackupCardPresentation(conn, null),
          );
      const unreachable =
          'Servidor inalcançável — não foi possível conectar: Connection refused';

      expect(
        advice(_conn(status: 'failed', error: unreachable)),
        DatabaseAddressAdvice.linkContainer,
      );
      expect(
        advice(_conn(status: 'failed', error: unreachable, container: _link)),
        DatabaseAddressAdvice.followContainer,
      );
      // Senha errada não se resolve trocando o endereço — mesmo com o
      // "Não foi possível conectar" que o servidor recusando também produz.
      expect(
        advice(
          _conn(
            status: 'failed',
            error:
                'vendas: Não foi possível conectar: password authentication failed',
          ),
        ),
        DatabaseAddressAdvice.none,
      );
      expect(
        advice(
          _conn(
            status: 'failed',
            error: 'Nenhum endereço do contêiner erp-db-1 respondeu',
            container: _link,
          ),
        ),
        DatabaseAddressAdvice.followContainer,
      );
      // Falha gravada antes da distinção, com a recusa de rede.
      expect(
        advice(
          _conn(
            status: 'failed',
            error:
                'Não foi possível conectar: Connection refused (os error 111)',
          ),
        ),
        DatabaseAddressAdvice.linkContainer,
      );
      expect(advice(_conn(status: 'success')), DatabaseAddressAdvice.none);
    });

    test('os contêineres mais prováveis vêm primeiro, sem os já usados', () {
      final ranked =
          rankContainersForConnection(_conn(viaProbeId: 7), <DatabaseContainer>[
            _container('outro-agente'),
            _container('mysql', engine: 'mysql', probeId: 7),
            _container('mesma-rota', probeId: 7),
            _container('parado', probeId: 7, running: false),
            _container('de-outra-conexao', probeId: 7, existing: 2),
          ]);
      expect(ranked.map((c) => c.containerName), <String>[
        'mesma-rota',
        'parado',
        'outro-agente',
      ]);
    });
  });
}

import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/models/agent_permission_models.dart';
import 'package:netmonitor_app/features/vpn/data/vpn_repository.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';

/// Os identificadores do `Permission` do backend
/// (`crates/agent-protocol/src/policy.rs`): o que o `serde` aceita em
/// `agentPolicy.allow`.
const Set<String> _backendPermissions = <String>{
  'read',
  'lifecycle',
  'update',
  'compose',
  'monitor',
  'discovery',
  'device_io',
  'database',
  'volume_export',
  'throughput',
  'deploy',
  'build',
};

class _RecordingAdapter implements HttpClientAdapter {
  _RecordingAdapter(this.body);

  final Map<String, dynamic> body;
  final List<String> paths = <String>[];

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    paths.add(options.uri.path);
    return ResponseBody.fromString(
      jsonEncode(body),
      200,
      headers: <String, List<String>>{
        Headers.contentTypeHeader: <String>[Headers.jsonContentType],
      },
    );
  }

  @override
  void close({bool force = false}) {}
}

void main() {
  test(
    'a sugestão de política vem de GET /api/agents/install-policy',
    () async {
      final adapter = _RecordingAdapter(<String, dynamic>{
        'allow': <String>['read', 'database'],
        'databaseTargets': '10.0.0.5:5432',
      });
      final repository = VpnRepository(
        ApiClient(
          baseUrlProvider: () => 'http://central.invalid/api',
          tokenProvider: () => 'jwt',
          onUnauthorized: () {},
          dio: Dio()..httpClientAdapter = adapter,
        ),
      );

      final policy = await repository.fetchSuggestedAgentPolicy();

      expect(adapter.paths, <String>['/api/agents/install-policy']);
      expect(policy.allow, <String>['read', 'database']);
      expect(policy.databaseTargets, '10.0.0.5:5432');
    },
  );

  test('o formulário Linux com todas as permissões só manda o que o backend '
      'aceita', () {
    final json = CreateVpnPeerPayload(
      name: 'srv-app',
      profile: 'linux',
      ipAddress: '10.8.0.2',
      installAgent: true,
      agentPolicy: VpnInstallPolicy(
        allow: allAgentPermissionKeys(),
        databaseTargets: defaultDatabaseTargets,
      ),
    ).toJson();

    final policy = json['agentPolicy'] as Map<String, dynamic>;
    expect((policy['allow'] as List<String>).toSet(), _backendPermissions);
    expect(policy['databaseTargets'], defaultDatabaseTargets);
    expect(json['snmpEnabled'], isFalse);
    expect(json.containsKey('snmpProfileId'), isFalse);
  });

  test('ponte de banco sem destino é recusada como no backend', () {
    expect(missingDatabaseTargetsError(<String>['database'], ' , '), isNotNull);
    expect(
      missingDatabaseTargetsError(<String>['database'], '10.0.0.5:5432'),
      isNull,
    );
    expect(missingDatabaseTargetsError(<String>['read'], ''), isNull);
  });
}

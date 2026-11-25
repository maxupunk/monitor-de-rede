import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_endpoint_picker.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_snippets.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

ServerAddressEntry _address(String id, String kind, String? url) =>
    ServerAddressEntry(
      id: id,
      kind: kind,
      label: switch (kind) {
        'lan' => 'Rede local',
        'vpn' => 'VPN',
        'domain' => 'Domínio',
        _ => 'Internet',
      },
      description: '',
      value: url,
      overridden: false,
      source: 'detectado',
      https: url?.startsWith('https://') ?? false,
      url: url,
    );

/// A instalação de produção: domínio atrás do Cloudflare, LAN e VPN.
final List<ServerAddressEntry> _addresses = <ServerAddressEntry>[
  _address('domain', 'domain', 'https://monitor.master.pro.br'),
  _address('vpn', 'vpn', 'http://10.8.0.1:3333'),
  _address('lan', 'lan', 'http://192.168.0.10:3333'),
  _address('public', 'public', null),
];

void main() {
  group('Endereços do MCP', () {
    test('rede local primeiro, domínio por último e sem repetir', () {
      final options = mcpEndpointOptions(
        addresses: _addresses,
        currentOrigin: 'https://monitor.master.pro.br',
        endpointPath: '/mcp',
      );
      expect(options.map((o) => o.endpointUrl), <String>[
        'http://192.168.0.10:3333/mcp',
        'http://10.8.0.1:3333/mcp',
        'https://monitor.master.pro.br/mcp',
      ]);
      expect(options.map((o) => o.internal), <bool>[true, true, false]);
    });

    test('sem acesso externo, o padrão é um endereço que funciona', () {
      final options = mcpEndpointOptions(
        addresses: _addresses,
        currentOrigin: 'https://monitor.master.pro.br',
        endpointPath: '/mcp',
      );
      expect(pickMcpEndpoint(options, allowPublicOrigin: false)?.id, 'lan');
      expect(
        pickMcpEndpoint(
          options,
          selectedId: 'domain',
          allowPublicOrigin: false,
        )?.id,
        'domain',
        reason: 'a escolha do operador vale, com o aviso ao lado',
      );
    });

    testWidgets('domínio com acesso externo desligado explica por que falha', (
      tester,
    ) async {
      final options = mcpEndpointOptions(
        addresses: _addresses,
        currentOrigin: 'https://monitor.master.pro.br',
        endpointPath: '/mcp',
      );
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: Scaffold(
            body: McpEndpointPicker(
              options: options,
              selected: options.last,
              allowPublicOrigin: false,
              onSelected: (_) {},
            ),
          ),
        ),
      );
      expect(find.text('Domínio'), findsOneWidget);
      expect(find.textContaining('sai e volta por fora'), findsOneWidget);
    });
  });

  test('Antigravity usa serverUrl e o cabeçalho do token', () {
    final decoded =
        jsonDecode(
              McpClient.antigravity.snippet(
                endpointUrl: 'http://10.8.0.1:3333/mcp',
                token: 'nmm_secret',
              ),
            )
            as Map<String, dynamic>;
    final server =
        (decoded['mcpServers'] as Map<String, dynamic>)['netmonitor']
            as Map<String, dynamic>;
    expect(server['serverUrl'], 'http://10.8.0.1:3333/mcp');
    expect(server.containsKey('url'), isFalse);
    expect(
      (server['headers'] as Map<String, dynamic>)['Authorization'],
      'Bearer nmm_secret',
    );
    expect(McpClient.values.first, McpClient.antigravity);
  });
}

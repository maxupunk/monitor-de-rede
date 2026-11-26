import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/presentation/onboarding/initial_setup_dialog.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Endereço da central injetado no teste: fora da Web, `resolveCentralOrigin`
/// usa a URL configurada — o mesmo papel do host do navegador na PWA.
class _FixedServerConfigNotifier extends ServerConfigNotifier {
  _FixedServerConfigNotifier(this._baseUrl);

  final String _baseUrl;

  @override
  ServerConfigState build() => ServerConfigState(
    baseUrl: _baseUrl,
    recentServers: const <String>[],
    initialized: true,
  );
}

/// Requisições que o assistente fez, para conferir o que foi gravado.
final List<RequestOptions> _requests = <RequestOptions>[];

ApiClient _emptyApi() {
  _requests.clear();
  final dio = Dio()
    ..interceptors.add(
      InterceptorsWrapper(
        onRequest: (options, handler) => handler.resolve(
          Response<dynamic>(
            requestOptions: (_requests..add(options)).last,
            statusCode: 200,
            data: <String, dynamic>{},
          ),
        ),
      ),
    );
  return ApiClient(
    baseUrlProvider: () => 'http://localhost/api',
    tokenProvider: () => 'jwt',
    onUnauthorized: () {},
    dio: dio,
  );
}

/// Abre o assistente e avança só por Início → Áreas (só VPN) → Endereços →
/// Preferências → VPN, devolvendo o texto do campo de endpoint.
Future<String> _vpnEndpointFor(WidgetTester tester, String baseUrl) async {
  tester.view.physicalSize = const Size(1200, 2000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);

  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        apiClientProvider.overrideWithValue(_emptyApi()),
        serverConfigProvider.overrideWith(
          () => _FixedServerConfigNotifier(baseUrl),
        ),
      ],
      child: MaterialApp(
        theme: AppTheme.light(),
        home: Scaffold(
          body: Consumer(
            builder: (context, ref, _) => TextButton(
              onPressed: () => showInitialSetupWizard(context, ref),
              child: const Text('abrir'),
            ),
          ),
        ),
      ),
    ),
  );

  await tester.tap(find.text('abrir'));
  await tester.pumpAndSettle();
  await tester.tap(find.text('Começar do zero'));
  await tester.pumpAndSettle();

  await tester.tap(find.text('Rede'));
  await tester.tap(find.text('Servidores'));
  await tester.tap(find.text('VPN WireGuard'));
  await tester.pumpAndSettle();

  for (var i = 0; i < 3; i++) {
    await tester.tap(find.text('Avançar'));
    await tester.pumpAndSettle();
  }

  final field = find.widgetWithText(
    TextField,
    'Endpoint público / DDNS (opcional)',
  );
  expect(field, findsOneWidget);
  return tester.widget<TextField>(field).controller!.text;
}

void main() {
  testWidgets('endpoint da VPN vem preenchido com o domínio da central', (
    tester,
  ) async {
    expect(
      await _vpnEndpointFor(tester, 'https://monitor.master.pro.br:8443'),
      'monitor.master.pro.br',
    );
  });

  testWidgets('concluir o assistente grava o endpoint no servidor VPN', (
    tester,
  ) async {
    await _vpnEndpointFor(tester, 'https://monitor.master.pro.br');
    // VPN → Notificações → Revisão → concluir.
    for (var i = 0; i < 2; i++) {
      await tester.tap(find.text('Avançar'));
      await tester.pumpAndSettle();
    }
    await tester.tap(find.text('Concluir configuração'));
    // A tela final tem animação contínua: avança o relógio em vez de esperar
    // ela assentar.
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }

    final put = _requests.where(
      (r) => r.method == 'PUT' && r.path.endsWith('/vpn/server'),
    );
    expect(put, hasLength(1));
    expect(put.single.data, <String, dynamic>{
      'publicEndpoint': 'monitor.master.pro.br',
    });
  });

  testWidgets('endpoint da VPN fica vazio quando a central é acessada por IP', (
    tester,
  ) async {
    expect(await _vpnEndpointFor(tester, 'http://192.168.1.10:5150'), isEmpty);
  });
}

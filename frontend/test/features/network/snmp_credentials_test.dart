import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/models/snmp_credential_models.dart';
import 'package:netmonitor_app/features/core/presentation/widgets/snmp_credential_dropdown.dart';
import 'package:netmonitor_app/features/core/providers/snmp_credentials_provider.dart';
import 'package:netmonitor_app/features/network/widgets/snmp_credential_form_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/snmp_credentials_card.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _MockHttpAdapter implements HttpClientAdapter {
  _MockHttpAdapter(this.handler);

  final ResponseBody Function(RequestOptions options) handler;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async => handler(options);

  @override
  void close({bool force = false}) {}
}

ResponseBody _json(int status, Object? body) => ResponseBody.fromString(
  body == null ? '' : jsonEncode(body),
  status,
  headers: <String, List<String>>{
    Headers.contentTypeHeader: <String>[Headers.jsonContentType],
  },
);

const Map<String, dynamic> _v2cJson = <String, dynamic>{
  'id': 1,
  'name': 'Comunidade da casa',
  'version': 'v2c',
  'securityLevel': 'community',
  'community': kSnmpSecretMask,
  'isDefault': true,
  'deviceCount': 3,
};

const Map<String, dynamic> _v3Json = <String, dynamic>{
  'id': 2,
  'name': 'Core v3',
  'version': 'v3',
  'securityLevel': 'authPriv',
  'username': 'monitor',
  'authProtocol': 'sha',
  'authKey': kSnmpSecretMask,
  'privProtocol': 'aes128',
  'privKey': kSnmpSecretMask,
  'deviceCount': 0,
};

class _StubAuthNotifier extends AuthNotifier {
  _StubAuthNotifier(this.role);

  final UserRole role;

  @override
  AuthState build() => AuthState(
    token: 'jwt',
    user: AuthUser(id: 1, email: 'a@local', role: role),
  );
}

/// O estado já carregado, sem HTTP.
class _SeededCredentialsNotifier extends SnmpCredentialsNotifier {
  @override
  SnmpCredentialsState build() => SnmpCredentialsState(
    loaded: true,
    items: <SnmpCredentialModel>[
      SnmpCredentialModel.fromJson(_v2cJson),
      SnmpCredentialModel.fromJson(_v3Json),
    ],
  );

  @override
  Future<void> fetchAll({bool force = false}) async {}
}

Widget _app(Widget child, {UserRole role = UserRole.admin}) => ProviderScope(
  overrides: <Override>[
    authProvider.overrideWith(() => _StubAuthNotifier(role)),
    snmpCredentialsProvider.overrideWith(_SeededCredentialsNotifier.new),
  ],
  child: MaterialApp(
    theme: AppTheme.dark(),
    home: Scaffold(
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: child,
      ),
    ),
  ),
);

void main() {
  group('SnmpCredentialDraft', () {
    test('v2c leva só a comunidade; segredo vazio é omitido', () {
      const draft = SnmpCredentialDraft(
        name: ' Casa ',
        version: 'v2c',
        community: '',
        username: 'ignorado',
        authProtocol: 'sha',
        authKey: 'senha-longa',
      );
      expect(draft.toJson(), <String, dynamic>{
        'name': 'Casa',
        'version': 'v2c',
        'isDefault': false,
      });
    });

    test('v3 authPriv leva usuário, protocolos e senhas digitadas', () {
      const draft = SnmpCredentialDraft(
        name: 'Core',
        version: 'v3',
        community: 'ignorada',
        username: 'monitor',
        authProtocol: 'sha256',
        authKey: 'senha-auth',
        privProtocol: 'aes128',
        privKey: '',
        contextName: 'vlan10',
        isDefault: true,
      );
      expect(draft.toJson(), <String, dynamic>{
        'name': 'Core',
        'version': 'v3',
        'isDefault': true,
        'username': 'monitor',
        'authProtocol': 'sha256',
        'authKey': 'senha-auth',
        'privProtocol': 'aes128',
        'contextName': 'vlan10',
      });
    });

    test('sem autenticação não manda privacidade', () {
      const draft = SnmpCredentialDraft(
        name: 'Leitor',
        version: 'v3',
        username: 'u',
        privProtocol: 'aes128',
        privKey: 'senha-priv',
      );
      final json = draft.toJson();
      expect(json.containsKey('privProtocol'), isFalse);
      expect(json.containsKey('privKey'), isFalse);
    });
  });

  test('o provider lista, cria, edita e apaga pela API', () async {
    final items = <Map<String, dynamic>>[_v2cJson];
    Map<String, dynamic>? lastBody;
    final calls = <String>[];
    final dio = Dio()
      ..httpClientAdapter = _MockHttpAdapter((options) {
        calls.add('${options.method} ${options.uri.path}');
        if (options.data is Map) {
          lastBody = Map<String, dynamic>.from(options.data as Map);
        }
        if (options.method == 'GET') return _json(200, items);
        if (options.method == 'POST') {
          items.add(_v3Json);
          return _json(201, _v3Json);
        }
        if (options.method == 'PUT') return _json(200, _v3Json);
        if (options.method == 'DELETE') {
          items.removeWhere((item) => item['id'] == 2);
          return _json(204, null);
        }
        return _json(404, <String, dynamic>{'message': 'não'});
      });
    final container = ProviderContainer(
      overrides: <Override>[
        apiClientProvider.overrideWithValue(
          ApiClient(
            baseUrlProvider: () => 'http://localhost:5150/api',
            tokenProvider: () => 'jwt',
            onUnauthorized: () {},
            dio: dio,
          ),
        ),
      ],
    );
    addTearDown(container.dispose);
    final notifier = container.read(snmpCredentialsProvider.notifier);

    await notifier.fetchAll();
    final state = container.read(snmpCredentialsProvider);
    expect(state.items.single.hasCommunity, isTrue);
    expect(state.items.single.community, kSnmpSecretMask);
    expect(state.defaultCredential?.id, 1);

    final created = await notifier.create(
      const SnmpCredentialDraft(
        name: 'Core v3',
        version: 'v3',
        username: 'monitor',
        authProtocol: 'sha',
        authKey: 'senha-auth-1',
      ),
    );
    expect(created.securityLevel, 'authPriv');
    expect(lastBody?['authKey'], 'senha-auth-1');
    expect(container.read(snmpCredentialsProvider).items, hasLength(2));

    // Edição com a senha em branco não manda a senha: o backend mantém.
    await notifier.save(
      2,
      const SnmpCredentialDraft(
        name: 'Core v3',
        version: 'v3',
        username: 'monitor',
        authProtocol: 'sha',
      ),
    );
    expect(lastBody?.containsKey('authKey'), isFalse);

    await notifier.remove(2);
    expect(container.read(snmpCredentialsProvider).items, hasLength(1));
    expect(
      calls,
      containsAll(<String>[
        'POST /api/snmp/credentials',
        'PUT /api/snmp/credentials/2',
        'DELETE /api/snmp/credentials/2',
      ]),
    );
  });

  testWidgets('o card lista as credenciais sem segredo e com o nível', (
    tester,
  ) async {
    await tester.pumpWidget(_app(const SnmpCredentialsCard()));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.text('Credenciais SNMP'), findsOneWidget);
    expect(find.text('Comunidade da casa'), findsOneWidget);
    expect(find.text('Core v3'), findsOneWidget);
    expect(find.text('Autenticação + criptografia'), findsOneWidget);
    expect(find.text('Padrão'), findsOneWidget);
    expect(find.text('Usuário: monitor'), findsOneWidget);
    expect(find.text(kSnmpSecretMask), findsNothing);
    expect(find.text('Nova credencial'), findsOneWidget);
  });

  testWidgets('leitor não vê os botões de alteração', (tester) async {
    await tester.pumpWidget(
      _app(const SnmpCredentialsCard(), role: UserRole.viewer),
    );
    await tester.pumpAndSettle();
    expect(find.text('Nova credencial'), findsNothing);
    expect(find.byTooltip('Excluir'), findsNothing);
  });

  testWidgets('o formulário de edição v3 não preenche segredo e explica', (
    tester,
  ) async {
    await tester.pumpWidget(
      _app(SnmpCredentialForm(existing: SnmpCredentialModel.fromJson(_v3Json))),
    );
    await tester.pumpAndSettle();

    expect(find.text('Usuário (v3)'), findsOneWidget);
    expect(find.text('Comunidade'), findsNothing);
    expect(find.text('Senha de autenticação'), findsOneWidget);
    expect(find.text('Senha de criptografia'), findsOneWidget);
    expect(find.text('Deixe em branco para manter'), findsNWidgets(2));
    expect(find.text(kSnmpSecretMask), findsNothing);
    expect(find.text('Testar a credencial gravada'), findsOneWidget);
  });

  testWidgets('o formulário novo em v2c pede a comunidade', (tester) async {
    await tester.pumpWidget(_app(const SnmpCredentialForm()));
    await tester.pumpAndSettle();

    expect(find.text('Comunidade'), findsOneWidget);
    expect(find.text('Usuário (v3)'), findsNothing);
    await tester.tap(find.text('Salvar'));
    await tester.pumpAndSettle();
    expect(find.text('Informe a comunidade'), findsOneWidget);
    expect(find.text('Informe um nome'), findsOneWidget);
  });

  testWidgets(
    'o seletor não quebra com credencial que não está na lista (AGENTS §2)',
    (tester) async {
      int? escolhido = 999;
      await tester.pumpWidget(
        _app(
          SnmpCredentialDropdown(value: 999, onChanged: (id) => escolhido = id),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(
        find.text('Padrão do sistema (Comunidade da casa)'),
        findsOneWidget,
      );

      await tester.tap(find.text('Padrão do sistema (Comunidade da casa)'));
      await tester.pumpAndSettle();
      await tester.tap(find.textContaining('Core v3').last);
      await tester.pumpAndSettle();
      expect(escolhido, 2);
    },
  );
}

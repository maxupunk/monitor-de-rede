import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_create_token_sheet.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_settings_card.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_snippets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _MockHttpAdapter implements HttpClientAdapter {
  _MockHttpAdapter(this.handler);

  final ResponseBody Function(RequestOptions options) handler;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    return handler(options);
  }

  @override
  void close({bool force = false}) {}
}

ResponseBody _jsonResponse(int statusCode, Object? body) {
  final encoded = body == null ? '' : jsonEncode(body);
  return ResponseBody.fromString(
    encoded,
    statusCode,
    headers: <String, List<String>>{
      Headers.contentTypeHeader: <String>[Headers.jsonContentType],
    },
  );
}

Map<String, dynamic> _sampleOverviewJson({
  bool enabled = true,
  bool allowPublicOrigin = false,
  bool canManageSettings = true,
  bool grantAdminScopes = true,
}) {
  return <String, dynamic>{
    'settings': <String, dynamic>{
      'enabled': enabled,
      'allowPublicOrigin': allowPublicOrigin,
      'endpointPath': '/mcp',
    },
    'scopes': <Map<String, dynamic>>[
      <String, dynamic>{
        'id': 'read',
        'label': 'Leitura geral',
        'description': 'Consulta dispositivos, monitores e alertas.',
        'grantedByRole': true,
        'requiresAdmin': false,
      },
      <String, dynamic>{
        'id': 'deploys:read',
        'label': 'Leitura de deploys',
        'description': 'Consulta aplicações, execuções e logs de deploy.',
        'grantedByRole': true,
        'requiresAdmin': false,
      },
      <String, dynamic>{
        'id': 'deploys:write',
        'label': 'Disparar deploys',
        'description': 'Dispara novos deploys e builds das aplicações.',
        'grantedByRole': grantAdminScopes,
        'requiresAdmin': true,
      },
      <String, dynamic>{
        'id': 'deploys:approve',
        'label': 'Aprovar deploys',
        'description': 'Aprova ou rejeita revisões pendentes.',
        'grantedByRole': grantAdminScopes,
        'requiresAdmin': true,
      },
    ],
    'deployApplications': <Map<String, dynamic>>[
      <String, dynamic>{
        'id': 1,
        'name': 'Meu App',
        'slug': 'meu-app',
        'repository': 'org/repo',
      },
      <String, dynamic>{
        'id': 2,
        'name': 'API Billing',
        'slug': 'api-billing',
        'repository': 'org/billing',
      },
    ],
    'toolsCount': 58,
    'canManageSettings': canManageSettings,
  };
}

Map<String, dynamic> _sampleTokenJson({
  int id = 1,
  String name = 'Antigravity - meu-app',
  String hint = 'a1b2',
  List<String> scopes = const <String>['deploys:read', 'deploys:write'],
  List<int>? allowedApplicationIds = const <int>[1],
  bool revoked = false,
  bool expired = false,
  String? lastUsedAt,
  String? lastUsedIp,
}) {
  return <String, dynamic>{
    'id': id,
    'userId': 1,
    'owner': 'admin@monitor.local',
    'name': name,
    'hint': hint,
    'scopes': scopes,
    'allowedApplicationIds': allowedApplicationIds,
    'expiresAt': '2027-02-22T12:00:00Z',
    'lastUsedAt': lastUsedAt,
    'lastUsedIp': lastUsedIp,
    'revoked': revoked,
    'expired': expired,
    'createdAt': '2026-11-24T12:00:00Z',
  };
}

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

class _DisabledLocalAiNotifier extends AiSettingsNotifier {
  @override
  AiSettingsState build() => const AiSettingsState(
    settings: AiSettings(enabled: false, activeDriver: 'ollama'),
  );

  @override
  Future<void> load() async {}
}

/// O `build` real do [McpNotifier], com um jeito de semear o estado.
class _ExposedMcpNotifier extends McpNotifier {
  void seed(McpState value) => state = value;
}

class _SwitchableAuthNotifier extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'jwt-a',
    user: AuthUser(id: 1, email: 'a@netmonitor.local'),
    initialized: true,
  );

  void signInAs(int id) {
    state = AuthState(
      token: 'jwt-$id',
      user: AuthUser(id: id, email: 'u$id@netmonitor.local'),
      initialized: true,
    );
  }

  void signOut() {
    state = state.copyWith(clearToken: true, clearUser: true);
  }
}

class _SeededMcpNotifier extends McpNotifier {
  _SeededMcpNotifier(this._initialState);

  final McpState _initialState;
  final List<Map<String, dynamic>> createdPayloads = <Map<String, dynamic>>[];
  final List<int> revokedIds = <int>[];
  Map<String, bool>? lastUpdatedSettings;

  @override
  McpState build() => _initialState;

  @override
  Future<void> loadAll() async {}

  @override
  Future<bool> updateSettings({
    required bool enabled,
    required bool allowPublicOrigin,
  }) async {
    lastUpdatedSettings = <String, bool>{
      'enabled': enabled,
      'allowPublicOrigin': allowPublicOrigin,
    };
    final current = state.overview ?? const McpOverview();
    state = state.copyWith(
      overview: current.copyWith(
        settings: current.settings.copyWith(
          enabled: enabled,
          allowPublicOrigin: allowPublicOrigin,
        ),
      ),
    );
    return true;
  }

  @override
  Future<McpTokenCreated> createToken({
    required String name,
    required List<String> scopes,
    List<int>? allowedApplicationIds,
    required int ttlDays,
  }) async {
    createdPayloads.add(<String, dynamic>{
      'name': name,
      'scopes': scopes,
      'allowedApplicationIds': allowedApplicationIds,
      'ttlDays': ttlDays,
    });
    final created = McpTokenCreated(
      token: McpTokenView(
        id: 99,
        userId: 1,
        owner: 'admin@monitor.local',
        name: name,
        hint: 'z9y8',
        scopes: scopes,
        allowedApplicationIds: allowedApplicationIds,
        expiresAt: '2027-02-22T12:00:00Z',
      ),
      secret: 'nmm_secret_token_99',
    );
    state = state.copyWith(
      justCreatedToken: created,
      tokens: <McpTokenView>[created.token, ...state.tokens],
      tokensLoaded: true,
    );
    return created;
  }

  @override
  Future<void> revokeToken(int id) async {
    revokedIds.add(id);
    state = state.copyWith(
      tokens: state.tokens
          .map(
            (t) => t.id == id
                ? McpTokenView(
                    id: t.id,
                    userId: t.userId,
                    owner: t.owner,
                    name: t.name,
                    hint: t.hint,
                    scopes: t.scopes,
                    allowedApplicationIds: t.allowedApplicationIds,
                    expiresAt: t.expiresAt,
                    lastUsedAt: t.lastUsedAt,
                    lastUsedIp: t.lastUsedIp,
                    revoked: true,
                    expired: t.expired,
                    createdAt: t.createdAt,
                  )
                : t,
          )
          .toList(growable: false),
    );
  }
}

Widget _wrapApp(Widget child, {List<Override> overrides = const <Override>[]}) {
  return ProviderScope(
    overrides: overrides,
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
}

void main() {
  group('MCP models & snippet builders', () {
    test('parses McpOverview, McpTokenView and McpTokenCreated accurately', () {
      final overview = McpOverview.fromJson(_sampleOverviewJson());
      expect(overview.settings.enabled, isTrue);
      expect(overview.settings.allowPublicOrigin, isFalse);
      expect(overview.settings.endpointPath, '/mcp');
      expect(overview.toolsCount, 58);
      expect(overview.canManageSettings, isTrue);
      expect(overview.scopes.length, 4);
      expect(overview.scopeById('deploys:write')?.requiresAdmin, isTrue);
      expect(overview.deployApplications.length, 2);
      expect(overview.applicationById(1)?.name, 'Meu App');
      expect(overview.applicationById(999), isNull);

      final restrictedToken = McpTokenView.fromJson(
        _sampleTokenJson(
          lastUsedAt: '2026-11-24T12:05:00Z',
          lastUsedIp: '10.8.0.5',
        ),
      );
      expect(restrictedToken.isActive, isTrue);
      expect(restrictedToken.allowedApplicationIds, <int>[1]);
      expect(restrictedToken.lastUsedIp, '10.8.0.5');

      final unrestrictedToken = McpTokenView.fromJson(
        _sampleTokenJson(id: 2, allowedApplicationIds: null, revoked: true),
      );
      expect(unrestrictedToken.isActive, isFalse);
      expect(unrestrictedToken.allowedApplicationIds, isNull);

      final created = McpTokenCreated.fromJson(<String, dynamic>{
        'token': _sampleTokenJson(),
        'secret': 'nmm_abc123',
      });
      expect(created.secret, 'nmm_abc123');
      expect(created.token.hint, 'a1b2');
    });

    test('builds MCP endpoint URL, mcp.json and Claude Code CLI snippets', () {
      final url = buildMcpEndpointUrl(
        origin: 'http://10.8.0.1:5150/',
        endpointPath: '/mcp',
      );
      expect(url, 'http://10.8.0.1:5150/mcp');

      final jsonSnippet = buildMcpJsonSnippet(
        endpointUrl: url,
        token: 'nmm_secret',
      );
      final decoded = jsonDecode(jsonSnippet) as Map<String, dynamic>;
      final server =
          (decoded['mcpServers'] as Map<String, dynamic>)['netmonitor']
              as Map<String, dynamic>;
      expect(server['url'], 'http://10.8.0.1:5150/mcp');
      expect(
        (server['headers'] as Map<String, dynamic>)['Authorization'],
        'Bearer nmm_secret',
      );

      final vscode =
          jsonDecode(
                buildVsCodeMcpSnippet(endpointUrl: url, token: 'nmm_secret'),
              )
              as Map<String, dynamic>;
      final vsServer =
          (vscode['servers'] as Map<String, dynamic>)['netmonitor']
              as Map<String, dynamic>;
      expect(vsServer['type'], 'http');
      expect(vsServer['url'], 'http://10.8.0.1:5150/mcp');
      expect(
        (vsServer['headers'] as Map<String, dynamic>)['Authorization'],
        'Bearer nmm_secret',
      );

      final cli = buildClaudeCodeCliSnippet(
        endpointUrl: url,
        token: 'nmm_secret',
      );
      expect(
        cli,
        'claude mcp add --transport http netmonitor http://10.8.0.1:5150/mcp '
        '--header "Authorization: Bearer nmm_secret"',
      );
    });
  });

  group('McpNotifier REST lifecycle & zero-polling SSE contract', () {
    test(
      'loads, creates token, revokes token, updates settings and refreshes on mcp:changed SSE',
      () async {
        var overviewGets = 0;
        var tokenGets = 0;
        var enabled = true;
        var allowPublicOrigin = false;
        final tokens = <Map<String, dynamic>>[_sampleTokenJson()];
        Map<String, dynamic>? lastPostBody;

        final dio = Dio(BaseOptions(validateStatus: (_) => true))
          ..httpClientAdapter = _MockHttpAdapter((options) {
            final path = options.uri.path;
            if (options.method == 'GET' && path.endsWith('/mcp/overview')) {
              overviewGets++;
              return _jsonResponse(
                200,
                _sampleOverviewJson(
                  enabled: enabled,
                  allowPublicOrigin: allowPublicOrigin,
                ),
              );
            }
            if (options.method == 'GET' && path.endsWith('/mcp/tokens')) {
              tokenGets++;
              return _jsonResponse(200, tokens);
            }
            if (options.method == 'PUT' && path.endsWith('/mcp/settings')) {
              final data = options.data as Map<String, dynamic>;
              enabled = data['enabled'] as bool;
              allowPublicOrigin = data['allowPublicOrigin'] as bool;
              return _jsonResponse(200, <String, dynamic>{
                'enabled': enabled,
                'allowPublicOrigin': allowPublicOrigin,
                'endpointPath': '/mcp',
              });
            }
            if (options.method == 'POST' && path.endsWith('/mcp/tokens')) {
              lastPostBody = Map<String, dynamic>.from(
                options.data as Map<dynamic, dynamic>,
              );
              final next = _sampleTokenJson(
                id: 2,
                name: lastPostBody!['name'] as String,
                hint: 'x9y0',
                scopes: List<String>.from(lastPostBody!['scopes'] as List),
                allowedApplicationIds:
                    lastPostBody!['allowedApplicationIds'] == null
                    ? null
                    : List<int>.from(
                        lastPostBody!['allowedApplicationIds'] as List,
                      ),
              );
              tokens.add(next);
              return _jsonResponse(201, <String, dynamic>{
                'token': next,
                'secret': 'nmm_created_secret_value',
              });
            }
            if (options.method == 'DELETE' && path.endsWith('/mcp/tokens/2')) {
              tokens[1] = <String, dynamic>{...tokens[1], 'revoked': true};
              return _jsonResponse(204, null);
            }
            return _jsonResponse(404, <String, dynamic>{
              'message': 'Not found',
            });
          });

        final client = ApiClient(
          baseUrlProvider: () => 'http://localhost:5150/api',
          tokenProvider: () => 'jwt',
          onUnauthorized: () {},
          dio: dio,
        );

        final container = ProviderContainer(
          overrides: <Override>[apiClientProvider.overrideWithValue(client)],
        );
        addTearDown(container.dispose);

        // Register area SSE routes (`mcp:changed` under ProductArea.core).
        container.read(areaRealtimeCoordinatorProvider);
        final sse = container.read(sseServiceProvider.notifier);
        expect(sse.hasRoute('mcp:changed'), isTrue);

        final notifier = container.read(mcpProvider.notifier);
        await notifier.loadAll();

        expect(overviewGets, 1);
        expect(tokenGets, 1);
        expect(container.read(mcpProvider).overview?.toolsCount, 58);
        expect(container.read(mcpProvider).tokens.length, 1);

        // Update settings
        final updated = await notifier.updateSettings(
          enabled: false,
          allowPublicOrigin: true,
        );
        expect(updated, isTrue);
        expect(container.read(mcpProvider).overview?.settings.enabled, isFalse);
        expect(
          container.read(mcpProvider).overview?.settings.allowPublicOrigin,
          isTrue,
        );

        // Create token sets one-time secret banner and refreshes token list
        final created = await notifier.createToken(
          name: 'Cursor - billing',
          scopes: const <String>['deploys:read', 'deploys:write'],
          allowedApplicationIds: const <int>[2],
          ttlDays: 180,
        );
        expect(created.secret, 'nmm_created_secret_value');
        expect(lastPostBody?['ttlDays'], 180);
        expect(lastPostBody?['allowedApplicationIds'], <int>[2]);
        expect(
          container.read(mcpProvider).justCreatedToken?.secret,
          'nmm_created_secret_value',
        );
        expect(container.read(mcpProvider).tokens.length, 2);

        notifier.clearJustCreatedToken();
        expect(container.read(mcpProvider).justCreatedToken, isNull);

        // Revoke token (HTTP 204)
        await notifier.revokeToken(2);
        expect(
          container
              .read(mcpProvider)
              .tokens
              .firstWhere((t) => t.id == 2)
              .revoked,
          isTrue,
        );

        final getsBeforeSse = overviewGets;
        final tokenGetsBeforeSse = tokenGets;

        // Dispatch SSE `mcp:changed` event -> triggers debounced reload (800ms)
        sse.dispatchForTest(
          const SseEvent(
            type: 'mcp:changed',
            timestamp: '2026-11-24T13:00:00Z',
            data: <String, dynamic>{'reason': 'token_revoked'},
          ),
        );
        // Espera a recarga em si, não um tempo fixo: sob a carga da suíte
        // inteira, 150 ms de folga sobre o debounce não bastavam.
        final deadline = DateTime.now().add(const Duration(seconds: 5));
        while ((overviewGets <= getsBeforeSse ||
                tokenGets <= tokenGetsBeforeSse) &&
            DateTime.now().isBefore(deadline)) {
          await Future<void>.delayed(const Duration(milliseconds: 50));
        }

        expect(overviewGets, getsBeforeSse + 1);
        expect(tokenGets, tokenGetsBeforeSse + 1);
      },
    );

    test(
      'McpSettingsCard is registered in Settings tab ia at order 15 under ProductArea.core',
      () {
        final visibleTabs = SettingsSectionRegistry.visibleTabs(
          const <ProductArea>{ProductArea.core},
        );
        final iaTab = visibleTabs.firstWhere((t) => t.tab.value == 'ia');
        final orders = iaTab.cards.map((c) => c.order).toList();
        expect(orders, contains(15));
        expect(orders.indexOf(15), greaterThan(orders.indexOf(10)));
        expect(orders.indexOf(15), lessThan(orders.indexOf(20)));
      },
    );
  });

  group('McpNotifier por sessão', () {
    for (final (label, change)
        in <(String, void Function(_SwitchableAuthNotifier))>[
          ('sair', (auth) => auth.signOut()),
          ('trocar de usuário', (auth) => auth.signInAs(2)),
        ]) {
      test('o segredo recém-criado some ao $label', () {
        final container = ProviderContainer(
          overrides: <Override>[
            authProvider.overrideWith(_SwitchableAuthNotifier.new),
            mcpProvider.overrideWith(_ExposedMcpNotifier.new),
          ],
        );
        addTearDown(container.dispose);

        (container.read(mcpProvider.notifier) as _ExposedMcpNotifier).seed(
          McpState(
            justCreatedToken: McpTokenCreated.fromJson(<String, dynamic>{
              'token': _sampleTokenJson(),
              'secret': 'nmm_segredo_do_usuario_a',
            }),
            loaded: true,
          ),
        );
        expect(container.read(mcpProvider).justCreatedToken, isNotNull);

        change(
          container.read(authProvider.notifier) as _SwitchableAuthNotifier,
        );

        expect(container.read(mcpProvider).justCreatedToken, isNull);
        expect(container.read(mcpProvider).loaded, isFalse);
      });
    }
  });

  group('McpSettingsCard, Create Token Sheet & Help Sheet widgets', () {
    testWidgets(
      'renders even when local AI is disabled, shows one-time secret snippets, and resolves deploy app chips',
      (tester) async {
        final seeded = _SeededMcpNotifier(
          McpState(
            overview: McpOverview.fromJson(_sampleOverviewJson()),
            tokens: <McpTokenView>[
              McpTokenView.fromJson(
                _sampleTokenJson(
                  lastUsedAt: '2026-11-24T12:00:00Z',
                  lastUsedIp: '10.8.0.12',
                ),
              ),
            ],
            loaded: true,
            tokensLoaded: true,
            justCreatedToken: McpTokenCreated(
              token: McpTokenView.fromJson(_sampleTokenJson()),
              secret: 'nmm_one_time_secret',
            ),
          ),
        );

        await tester.pumpWidget(
          _wrapApp(
            const McpSettingsCard(),
            overrides: <Override>[
              aiSettingsProvider.overrideWith(_DisabledLocalAiNotifier.new),
              mcpProvider.overrideWith(() => seeded),
              serverConfigProvider.overrideWith(
                () => _FixedServerConfigNotifier('http://10.8.0.1:5150'),
              ),
            ],
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // Card is visible even with local AI disabled
        expect(find.text('Servidor MCP (Agentes externos)'), findsOneWidget);
        expect(find.text('/mcp'), findsOneWidget);
        expect(find.text('58 ferramentas disponíveis'), findsOneWidget);

        // Admin switches are visible for admin
        expect(find.text('Ativar servidor MCP'), findsOneWidget);
        expect(
          find.text('Permitir conexões fora da rede interna/VPN'),
          findsOneWidget,
        );

        // Token list resolves app name from deployApplications
        expect(find.text('Antigravity - meu-app'), findsOneWidget);
        expect(find.text('a1b2'), findsOneWidget);
        expect(find.text('Meu App'), findsOneWidget);

        // One-time secret banner displays JSON snippet by default
        expect(find.textContaining('nmm_one_time_secret'), findsOneWidget);
        expect(find.textContaining('"mcpServers"'), findsOneWidget);

        // VS Code tem formato próprio (`servers` + `type: http`)
        await tester.ensureVisible(find.text('VS Code'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('VS Code'));
        await tester.pumpAndSettle();
        expect(find.textContaining('"servers"'), findsOneWidget);
        expect(find.textContaining('"mcpServers"'), findsNothing);

        // Switch to Claude Code tab
        await tester.ensureVisible(find.text('Claude Code'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Claude Code'));
        await tester.pumpAndSettle();
        expect(
          find.textContaining(
            'claude mcp add --transport http netmonitor http://10.8.0.1:5150/mcp',
          ),
          findsOneWidget,
        );

        // Switch to Token bruto tab and dismiss
        await tester.ensureVisible(find.text('Token bruto'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Token bruto'));
        await tester.pumpAndSettle();
        expect(find.text('nmm_one_time_secret'), findsOneWidget);

        // O X pede confirmação: o segredo não volta.
        await tester.ensureVisible(find.byTooltip('Fechar aviso'));
        await tester.pumpAndSettle();
        await tester.tap(find.byTooltip('Fechar aviso'));
        await tester.pumpAndSettle();
        expect(find.text('Já copiou o token?'), findsOneWidget);
        await tester.tap(find.text('Voltar'));
        await tester.pumpAndSettle();
        expect(find.text('nmm_one_time_secret'), findsOneWidget);

        await tester.ensureVisible(find.text('Entendi, já copiei'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Entendi, já copiei'));
        await tester.pumpAndSettle();
        expect(find.text('Entendi, já copiei'), findsNothing);
      },
    );

    testWidgets('hides admin switches for non-admin users', (tester) async {
      final seeded = _SeededMcpNotifier(
        McpState(
          overview: McpOverview.fromJson(
            _sampleOverviewJson(
              canManageSettings: false,
              grantAdminScopes: false,
            ),
          ),
          tokens: const <McpTokenView>[],
          loaded: true,
          tokensLoaded: true,
        ),
      );

      await tester.pumpWidget(
        _wrapApp(
          const McpSettingsCard(),
          overrides: <Override>[
            mcpProvider.overrideWith(() => seeded),
            serverConfigProvider.overrideWith(
              () => _FixedServerConfigNotifier('http://10.8.0.1:5150'),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      expect(find.text('Ativar servidor MCP'), findsNothing);
      expect(
        find.text('Permitir conexões fora da rede interna/VPN'),
        findsNothing,
      );
      expect(find.byType(Switch), findsNothing);
      expect(find.text('Nenhum token MCP criado'), findsOneWidget);
    });

    testWidgets(
      'create token sheet enforces role gates, deploy app restriction, and submits payload',
      (tester) async {
        final overview = McpOverview.fromJson(_sampleOverviewJson());
        final seeded = _SeededMcpNotifier(
          McpState(overview: overview, loaded: true, tokensLoaded: true),
        );

        await tester.pumpWidget(
          _wrapApp(
            Builder(
              builder: (context) => FilledButton(
                onPressed: () =>
                    showCreateMcpTokenSheet(context, overview: overview),
                child: const Text('Abrir modal'),
              ),
            ),
            overrides: <Override>[mcpProvider.overrideWith(() => seeded)],
          ),
        );

        await tester.tap(find.text('Abrir modal'));
        await tester.pumpAndSettle();

        // Enter token name
        await tester.enterText(
          find.widgetWithText(TextField, 'Nome do token'),
          'Antigravity — meu-app',
        );

        // Select "Deploy de projeto" preset
        await tester.tap(find.text('Deploy de projeto'));
        await tester.pumpAndSettle();

        // Deploy app restriction section is visible
        expect(find.text('Restringir a aplicações de deploy'), findsOneWidget);

        // Toggle off "Todas as aplicações permitidas" and select "Meu App"
        await tester.ensureVisible(find.text('Todas as aplicações permitidas'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Todas as aplicações permitidas'));
        await tester.pumpAndSettle();

        await tester.ensureVisible(find.text('Meu App'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Meu App'));
        await tester.pumpAndSettle();

        // Submit
        await tester.ensureVisible(find.text('Criar token'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Criar token'));
        await tester.pumpAndSettle();

        expect(seeded.createdPayloads.length, 1);
        final payload = seeded.createdPayloads.first;
        expect(payload['name'], 'Antigravity — meu-app');
        expect(payload['scopes'], const <String>[
          'deploys:read',
          'deploys:write',
        ]);
        expect(payload['allowedApplicationIds'], const <int>[1]);
        expect(payload['ttlDays'], 90);
      },
    );

    testWidgets(
      'create token sheet distinguishes requiresAdmin vs non-admin unavailable scopes',
      (tester) async {
        const overview = McpOverview(
          settings: McpSettings(),
          scopes: <McpScopeDescriptor>[
            McpScopeDescriptor(
              id: 'read',
              label: 'Leitura geral',
              description: 'Consulta dispositivos e alertas.',
              grantedByRole: true,
              requiresAdmin: false,
            ),
            McpScopeDescriptor(
              id: 'operations:write',
              label: 'Operações no sistema',
              description: 'Silenciar alertas e criar monitores.',
              grantedByRole: false,
              requiresAdmin: false,
            ),
            McpScopeDescriptor(
              id: 'deploys:approve',
              label: 'Aprovar deploys',
              description: 'Aprova revisões pendentes.',
              grantedByRole: false,
              requiresAdmin: true,
            ),
          ],
          toolsCount: 10,
          canManageSettings: false,
        );
        final seeded = _SeededMcpNotifier(
          const McpState(overview: overview, loaded: true, tokensLoaded: true),
        );

        await tester.pumpWidget(
          _wrapApp(
            Builder(
              builder: (context) => FilledButton(
                onPressed: () =>
                    showCreateMcpTokenSheet(context, overview: overview),
                child: const Text('Abrir modal'),
              ),
            ),
            overrides: <Override>[mcpProvider.overrideWith(() => seeded)],
          ),
        );

        await tester.tap(find.text('Abrir modal'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        expect(find.text('Leitura geral'), findsOneWidget);
        expect(
          find.text('Operações no sistema (Indisponível)'),
          findsOneWidget,
        );
        expect(find.text('Operações no sistema (Requer Admin)'), findsNothing);
        expect(find.text('Aprovar deploys (Requer Admin)'), findsOneWidget);
        expect(
          find.textContaining('"Requer Admin" ou "Indisponível"'),
          findsOneWidget,
        );

        final nonAdminLockedChip = tester.widget<FilterChip>(
          find.widgetWithText(
            FilterChip,
            'Operações no sistema (Indisponível)',
          ),
        );
        final adminLockedChip = tester.widget<FilterChip>(
          find.widgetWithText(FilterChip, 'Aprovar deploys (Requer Admin)'),
        );
        expect(nonAdminLockedChip.onSelected, isNull);
        expect(adminLockedChip.onSelected, isNull);
      },
    );

    testWidgets(
      'help sheet warns when central URL is public while allowPublicOrigin is false and fits 390px mobile',
      (tester) async {
        tester.view.physicalSize = const Size(390, 844);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final seeded = _SeededMcpNotifier(
          McpState(
            overview: McpOverview.fromJson(
              _sampleOverviewJson(allowPublicOrigin: false),
            ),
            tokens: <McpTokenView>[McpTokenView.fromJson(_sampleTokenJson())],
            loaded: true,
            tokensLoaded: true,
          ),
        );

        await tester.pumpWidget(
          _wrapApp(
            const McpSettingsCard(),
            overrides: <Override>[
              mcpProvider.overrideWith(() => seeded),
              serverConfigProvider.overrideWith(
                () => _FixedServerConfigNotifier(
                  'https://monitor.empresa.com.br',
                ),
              ),
            ],
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        await tester.tap(find.text('Como configurar'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        expect(find.text('Como conectar agentes ao MCP'), findsOneWidget);
        expect(
          find.textContaining('o endereço escolhido passa pela internet'),
          findsOneWidget,
        );
        expect(
          find.textContaining('https://monitor.empresa.com.br/mcp'),
          findsWidgets,
        );
      },
    );
  });
}

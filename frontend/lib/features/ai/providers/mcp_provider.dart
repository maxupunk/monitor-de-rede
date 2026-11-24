import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/ai/models/mcp_models.dart';

@immutable
class McpState {
  const McpState({
    this.overview,
    this.tokens = const <McpTokenView>[],
    this.loading = false,
    this.loadingTokens = false,
    this.savingSettings = false,
    this.loaded = false,
    this.tokensLoaded = false,
    this.justCreatedToken,
    this.error,
  });

  final McpOverview? overview;
  final List<McpTokenView> tokens;
  final bool loading;
  final bool loadingTokens;
  final bool savingSettings;
  final bool loaded;
  final bool tokensLoaded;
  final McpTokenCreated? justCreatedToken;
  final String? error;

  McpState copyWith({
    McpOverview? overview,
    List<McpTokenView>? tokens,
    bool? loading,
    bool? loadingTokens,
    bool? savingSettings,
    bool? loaded,
    bool? tokensLoaded,
    McpTokenCreated? justCreatedToken,
    bool clearJustCreatedToken = false,
    String? error,
    bool clearError = false,
  }) {
    return McpState(
      overview: overview ?? this.overview,
      tokens: tokens ?? this.tokens,
      loading: loading ?? this.loading,
      loadingTokens: loadingTokens ?? this.loadingTokens,
      savingSettings: savingSettings ?? this.savingSettings,
      loaded: loaded ?? this.loaded,
      tokensLoaded: tokensLoaded ?? this.tokensLoaded,
      justCreatedToken: clearJustCreatedToken
          ? null
          : (justCreatedToken ?? this.justCreatedToken),
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Estado do servidor MCP (agentes externos) e tokens pessoais.
/// Atualizado em tempo real exclusivamente pelo evento SSE `mcp:changed` (AGENTS.md §9).
class McpNotifier extends Notifier<McpState> {
  /// Recomeça a cada sessão: o segredo recém-criado (`justCreatedToken`) e os
  /// tokens são de quem está logado e não podem sobreviver a sair ou trocar
  /// de usuário na mesma aba.
  @override
  McpState build() {
    ref.watch(authProvider.select((auth) => (auth.token, auth.user?.id)));
    return const McpState();
  }

  ApiClient get _api => ref.read(apiClientProvider);

  Future<void> loadAll() async {
    state = state.copyWith(
      loading: true,
      loadingTokens: true,
      clearError: true,
    );
    try {
      final results = await Future.wait<dynamic>(<Future<dynamic>>[
        _api.get<Map<String, dynamic>>('/mcp/overview'),
        _api.get<List<dynamic>>('/mcp/tokens'),
      ]);
      final overview = McpOverview.fromJson(asJsonMap(results[0]));
      final tokens = List<McpTokenView>.unmodifiable(
        asJsonList(results[1], McpTokenView.fromJson),
      );
      state = state.copyWith(
        overview: overview,
        tokens: tokens,
        loading: false,
        loadingTokens: false,
        loaded: true,
        tokensLoaded: true,
      );
    } catch (err) {
      state = state.copyWith(
        loading: false,
        loadingTokens: false,
        error: describeError(
          err,
          fallback: 'Falha ao carregar configurações do servidor MCP.',
        ),
      );
    }
  }

  Future<void> fetchOverview() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await _api.get<Map<String, dynamic>>('/mcp/overview');
      state = state.copyWith(
        overview: McpOverview.fromJson(raw),
        loading: false,
        loaded: true,
      );
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(
          err,
          fallback: 'Falha ao carregar visão geral do servidor MCP.',
        ),
      );
    }
  }

  Future<List<McpTokenView>> fetchTokens() async {
    state = state.copyWith(loadingTokens: true);
    try {
      final raw = await _api.get<List<dynamic>>('/mcp/tokens');
      final list = List<McpTokenView>.unmodifiable(
        asJsonList(raw, McpTokenView.fromJson),
      );
      state = state.copyWith(
        tokens: list,
        loadingTokens: false,
        tokensLoaded: true,
        clearError: true,
      );
      return list;
    } catch (err) {
      state = state.copyWith(
        loadingTokens: false,
        error: describeError(err, fallback: 'Falha ao carregar tokens MCP.'),
      );
      return state.tokens;
    }
  }

  Future<bool> updateSettings({
    required bool enabled,
    required bool allowPublicOrigin,
  }) async {
    state = state.copyWith(savingSettings: true, clearError: true);
    try {
      final raw = await _api.put<Map<String, dynamic>>(
        '/mcp/settings',
        data: <String, dynamic>{
          'enabled': enabled,
          'allowPublicOrigin': allowPublicOrigin,
        },
      );
      final updatedSettings = McpSettings.fromJson(raw);
      final currentOverview = state.overview;
      state = state.copyWith(
        savingSettings: false,
        overview: currentOverview != null
            ? currentOverview.copyWith(settings: updatedSettings)
            : McpOverview(settings: updatedSettings),
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        savingSettings: false,
        error: describeError(
          err,
          fallback: 'Falha ao atualizar configurações do servidor MCP.',
        ),
      );
      return false;
    }
  }

  Future<McpTokenCreated> createToken({
    required String name,
    required List<String> scopes,
    List<int>? allowedApplicationIds,
    required int ttlDays,
  }) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/mcp/tokens',
      data: <String, dynamic>{
        'name': name,
        'scopes': scopes,
        'allowedApplicationIds': allowedApplicationIds,
        'ttlDays': ttlDays,
      },
    );
    final created = McpTokenCreated.fromJson(raw);
    state = state.copyWith(justCreatedToken: created, clearError: true);
    await fetchTokens();
    return created;
  }

  void clearJustCreatedToken() {
    state = state.copyWith(clearJustCreatedToken: true);
  }

  Future<void> revokeToken(int id) async {
    await _api.delete<dynamic>('/mcp/tokens/$id');
    await fetchTokens();
  }

  /// Disparado pelo evento SSE `mcp:changed` (sem polling).
  void refreshIfLoaded() {
    if (state.loaded || state.tokensLoaded) {
      loadAll();
    }
  }
}

final mcpProvider = NotifierProvider<McpNotifier, McpState>(McpNotifier.new);

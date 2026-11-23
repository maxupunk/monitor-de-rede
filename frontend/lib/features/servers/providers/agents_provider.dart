import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/utils/collection_helpers.dart';

@immutable
class AgentsState {
  const AgentsState({
    this.agents = const <AgentView>[],
    this.addresses = const <ServerAddressOption>[],
    this.loading = false,
    this.saving = false,
    this.loaded = false,
    this.error,
  });

  final List<AgentView> agents;
  final List<ServerAddressOption> addresses;
  final bool loading;
  final bool saving;
  final bool loaded;
  final String? error;

  AgentView? byId(int id) {
    for (final agent in agents) {
      if (agent.id == id) return agent;
    }
    return null;
  }

  AgentsState copyWith({
    List<AgentView>? agents,
    List<ServerAddressOption>? addresses,
    bool? loading,
    bool? saving,
    bool? loaded,
    String? error,
    bool clearError = false,
  }) {
    return AgentsState(
      agents: agents ?? this.agents,
      addresses: addresses ?? this.addresses,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      loaded: loaded ?? this.loaded,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Estado dos servidores remotos/agentes (ADR 011).
/// Carrega sob demanda; atualizações de conexão/host chegam por SSE (`probe:status`).
class AgentsNotifier extends Notifier<AgentsState> {
  @override
  AgentsState build() => const AgentsState();

  ApiClient get _api => ref.read(apiClientProvider);

  List<AgentView> _upsertSorted(List<AgentView> list, AgentView agent) {
    return upsertSortedByName<AgentView>(
      list,
      agent,
      idOf: (a) => a.id,
      nameOf: (a) => a.name,
    );
  }

  Future<void> fetchAgents() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await _api.get<List<dynamic>>('/agents');
      final parsed = asJsonList(raw, AgentView.fromJson);
      state = state.copyWith(
        agents: List<AgentView>.unmodifiable(parsed),
        loading: false,
        loaded: true,
      );
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(
          err,
          fallback: 'Erro ao carregar os servidores remotos',
        ),
      );
    }
  }

  Future<void> fetchServerAddresses() async {
    try {
      final raw = await _api.get<dynamic>('/server-addresses');
      final preferredId = raw is Map ? raw['preferredId'] as String? : null;
      final list = raw is List
          ? raw
          : (raw is Map && raw['data'] is List
                ? raw['data']
                : (raw is Map && raw['addresses'] is List
                      ? raw['addresses']
                      : const <dynamic>[]));
      final parsed = asJsonList(list, ServerAddressOption.fromJson)
          .where((a) => a.host.trim().isNotEmpty)
          .map(
            (a) => preferredId != null && a.id == preferredId
                ? ServerAddressOption(
                    id: a.id,
                    label: a.label,
                    kind: a.kind,
                    host: a.host,
                    url: a.url,
                    isDefault: true,
                  )
                : a,
          )
          .toList(growable: false);
      state = state.copyWith(
        addresses: List<ServerAddressOption>.unmodifiable(parsed),
      );
    } catch (_) {
      // Opcional para o seletor de endereço no diálogo de instalação.
    }
  }

  Future<AgentWithEnrollment?> createAgent(AgentCreateInput input) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await _api.post<Map<String, dynamic>>(
        '/agents',
        data: input.toJson(),
      );
      final created = AgentWithEnrollment.fromJson(raw);
      state = state.copyWith(
        agents: _upsertSorted(state.agents, created.agent),
        saving: false,
      );
      return created;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: describeError(err, fallback: 'Erro ao conectar o servidor'),
      );
      return null;
    }
  }

  Future<bool> updateAgent(int id, AgentUpdateInput input) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await _api.put<Map<String, dynamic>>(
        '/agents/$id',
        data: input.toJson(),
      );
      final updated = AgentView.fromJson(raw);
      state = state.copyWith(
        agents: _upsertSorted(state.agents, updated),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: describeError(err, fallback: 'Erro ao alterar o servidor'),
      );
      return false;
    }
  }

  Future<AgentEnrollmentView?> reissueEnrollment(int id) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await _api.post<Map<String, dynamic>>(
        '/agents/$id/enrollment',
      );
      state = state.copyWith(saving: false);
      return AgentEnrollmentView.fromJson(raw);
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: describeError(
          err,
          fallback: 'Erro ao gerar o código de instalação',
        ),
      );
      return null;
    }
  }

  Future<AgentInstallCommands?> installCommands({
    required String code,
    String? addressId,
    InstallPolicyView? policy,
  }) async {
    state = state.copyWith(clearError: true);
    try {
      final payload = <String, dynamic>{
        'code': code,
        'addressId': addressId,
        if (policy != null) 'policy': policy.toJson(),
      };
      final raw = await _api.post<Map<String, dynamic>>(
        '/agents/install-commands',
        data: payload,
      );
      return AgentInstallCommands.fromJson(raw);
    } catch (err) {
      state = state.copyWith(
        error: describeError(
          err,
          fallback: 'Erro ao gerar os comandos para este endereço',
        ),
      );
      return null;
    }
  }

  Future<InstallPolicyView?> suggestedPolicy() async {
    try {
      final raw = await _api.get<Map<String, dynamic>>(
        '/agents/install-policy',
      );
      return InstallPolicyView.fromJson(raw);
    } catch (err) {
      state = state.copyWith(
        error: describeError(
          err,
          fallback: 'Erro ao carregar as permissões sugeridas',
        ),
      );
      return null;
    }
  }

  Future<bool> revokeAgent(int id) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await _api.post<Map<String, dynamic>>('/agents/$id/revoke');
      final updated = AgentView.fromJson(raw);
      state = state.copyWith(
        agents: _upsertSorted(state.agents, updated),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: describeError(
          err,
          fallback: 'Erro ao revogar o acesso do servidor',
        ),
      );
      return false;
    }
  }

  Future<bool> removeAgent(int id) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      await _api.delete<dynamic>('/agents/$id');
      state = state.copyWith(
        agents: List<AgentView>.unmodifiable(
          state.agents.where((a) => a.id != id),
        ),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: describeError(err, fallback: 'Erro ao remover o servidor'),
      );
      return false;
    }
  }

  /// Aplica `probe:status` via SSE sem polling.
  void applyRealtimeStatus(Map<String, dynamic> data) {
    final id = parseIntOrNull(data['id'] ?? data['probeId']);
    if (id == null) return;
    final idx = state.agents.indexWhere((a) => a.id == id);
    if (idx == -1) return;

    final current = state.agents[idx];
    final nextStatus = data['status'] is String
        ? data['status'] as String
        : current.status;
    final nextConnected = data['status'] is String
        ? nextStatus == 'online'
        : current.connected;
    final nextHost = AgentHostInfo.tryFromDynamic(data['host']) ?? current.host;

    final updated = current.copyWith(
      status: nextStatus,
      connected: nextConnected,
      version: data['version'] is String
          ? data['version'] as String
          : current.version,
      lastSeenAt: data['lastSeenAt'] is String
          ? data['lastSeenAt'] as String
          : current.lastSeenAt,
      host: nextHost,
    );
    final nextList = List<AgentView>.from(state.agents);
    nextList[idx] = updated;
    state = state.copyWith(agents: List<AgentView>.unmodifiable(nextList));
  }
}

final agentsProvider = NotifierProvider<AgentsNotifier, AgentsState>(
  AgentsNotifier.new,
);

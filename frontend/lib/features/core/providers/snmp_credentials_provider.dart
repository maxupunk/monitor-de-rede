import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/snmp_credential_models.dart';

/// Credenciais SNMP reutilizáveis (`/api/snmp/credentials`).
///
/// Mora no Núcleo do app porque três áreas a usam — o cadastro de
/// dispositivo e o diagnóstico (Rede), o peer da VPN e o assistente inicial —
/// e nenhuma área importa outra.
@immutable
class SnmpCredentialsState {
  const SnmpCredentialsState({
    this.items = const <SnmpCredentialModel>[],
    this.loading = false,
    this.loaded = false,
    this.error,
  });

  final List<SnmpCredentialModel> items;
  final bool loading;
  final bool loaded;
  final String? error;

  /// A credencial padrão, quando há uma.
  SnmpCredentialModel? get defaultCredential {
    for (final item in items) {
      if (item.isDefault) return item;
    }
    return null;
  }

  SnmpCredentialModel? byId(int? id) {
    if (id == null) return null;
    for (final item in items) {
      if (item.id == id) return item;
    }
    return null;
  }

  SnmpCredentialsState copyWith({
    List<SnmpCredentialModel>? items,
    bool? loading,
    bool? loaded,
    String? error,
    bool clearError = false,
  }) {
    return SnmpCredentialsState(
      items: items ?? this.items,
      loading: loading ?? this.loading,
      loaded: loaded ?? this.loaded,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class SnmpCredentialsNotifier extends Notifier<SnmpCredentialsState> {
  static const String _base = '/snmp/credentials';

  @override
  SnmpCredentialsState build() => const SnmpCredentialsState();

  /// Carrega a lista (uma vez; `force` recarrega).
  Future<void> fetchAll({bool force = false}) async {
    if (state.loading || (state.loaded && !force)) return;
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>(_base);
      state = state.copyWith(
        items: asJsonMapList(
          raw,
        ).map(SnmpCredentialModel.fromJson).toList(growable: false),
        loading: false,
        loaded: true,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  /// Cria e devolve a credencial. Erros (nome repetido, senha curta) sobem
  /// para o formulário mostrar.
  Future<SnmpCredentialModel> create(SnmpCredentialDraft draft) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(_base, data: draft.toJson());
    final created = SnmpCredentialModel.fromJson(asJsonMap(raw));
    await fetchAll(force: true);
    return created;
  }

  /// Edita. Segredo vazio no rascunho mantém o gravado.
  Future<SnmpCredentialModel> save(int id, SnmpCredentialDraft draft) async {
    final raw = await ref
        .read(apiClientProvider)
        .put<dynamic>('$_base/$id', data: draft.toJson());
    final saved = SnmpCredentialModel.fromJson(asJsonMap(raw));
    await fetchAll(force: true);
    return saved;
  }

  /// Apaga. A API recusa (409) a credencial em uso, dizendo por quem.
  Future<void> remove(int id) async {
    await ref.read(apiClientProvider).delete<dynamic>('$_base/$id');
    await fetchAll(force: true);
  }

  /// Testa a credencial gravada contra um alvo. Devolve o `sysDescr`/`sysName`
  /// lidos; falha de autenticação sobe como erro.
  Future<Map<String, dynamic>> testAgainst(int id, String host) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '$_base/$id/test',
          timeout: const Duration(seconds: 30),
          data: <String, dynamic>{'host': host.trim()},
        );
    return asJsonMap(raw);
  }

  /// O assistente inicial: grava a comunidade padrão (v2c) como a credencial
  /// padrão — edita a padrão de comunidade que existir ou cria uma.
  Future<void> saveDefaultCommunity(String community) async {
    await fetchAll(force: true);
    final current = state.defaultCredential;
    final draft = SnmpCredentialDraft(
      name: current?.name ?? 'Padrão (v2c)',
      version: current != null && !current.isV3 ? current.version : 'v2c',
      community: community,
      isDefault: true,
    );
    if (current != null && !current.isV3) {
      await save(current.id, draft);
    } else {
      await create(draft);
    }
  }
}

final snmpCredentialsProvider =
    NotifierProvider<SnmpCredentialsNotifier, SnmpCredentialsState>(
      SnmpCredentialsNotifier.new,
    );

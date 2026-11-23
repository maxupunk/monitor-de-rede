import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';

/// Estado das áreas do produto (ADR 017).
///
/// O estado chega **só pelo SSE** (`areas:updated`), sem polling (AGENTS.md §9).
/// Enquanto o snapshot não chega, todas as áreas contam como ligadas.
@immutable
class AreasState {
  const AreasState({this.snapshot, this.saving});

  final AreasSnapshot? snapshot;
  final ProductArea? saving;

  List<AreaStateModel> get states =>
      snapshot?.areas ?? const <AreaStateModel>[];

  Set<ProductArea> get enabledAreas {
    if (snapshot == null) {
      return ProductArea.values.toSet();
    }
    return <ProductArea>{
      ProductArea.core,
      for (final item in snapshot!.areas)
        if (item.enabled) item.area,
    };
  }

  bool isEnabled(ProductArea area) {
    return area == ProductArea.core || enabledAreas.contains(area);
  }

  AreaStateModel? stateOf(ProductArea area) {
    for (final item in states) {
      if (item.area == area) return item;
    }
    return null;
  }

  AreasState copyWith({
    AreasSnapshot? snapshot,
    ProductArea? saving,
    bool clearSaving = false,
  }) {
    return AreasState(
      snapshot: snapshot ?? this.snapshot,
      saving: clearSaving ? null : (saving ?? this.saving),
    );
  }
}

class AreasNotifier extends Notifier<AreasState> {
  @override
  AreasState build() {
    ref.listen<String?>(sessionTokenProvider, (prev, next) {
      if (prev != null && (next == null || next.isEmpty)) {
        state = const AreasState();
      }
    });
    return const AreasState();
  }

  /// Limpa o snapshot em memória (ex.: troca de conta ou servidor).
  void reset() {
    state = const AreasState();
  }

  /// Aplica snapshot recebido pelo SSE (`areas:updated`) ou resposta HTTP de ação.
  void apply(Object? payload) {
    final parsed = AreasSnapshot.tryFromJson(payload);
    if (parsed != null) {
      state = state.copyWith(snapshot: parsed);
    }
  }

  /// Verifica consequências e bloqueios ao desligar uma área (somente admin).
  Future<AreaImpact> fetchImpact(ProductArea area) async {
    final response = await ref
        .read(apiClientProvider)
        .get<Map<String, dynamic>>('/areas/${area.id}/impact');
    return AreaImpact.fromJson(response);
  }

  /// Liga ou desliga uma área opcional (somente admin).
  Future<void> setEnabled(ProductArea area, bool enabled) async {
    state = state.copyWith(saving: area);
    try {
      final response = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>(
            '/areas/${area.id}',
            data: <String, dynamic>{'enabled': enabled},
          );
      apply(response);
    } finally {
      state = state.copyWith(clearSaving: true);
    }
  }

  /// Aplica a intenção inicial do assistente (`POST /api/settings/onboarding/intent`).
  Future<void> applyIntent(OnboardingIntentInput intent) async {
    final response = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>(
          '/settings/onboarding/intent',
          data: intent.toJson(),
        );
    apply(response);
  }
}

final areasProvider = NotifierProvider<AreasNotifier, AreasState>(
  AreasNotifier.new,
);

/// Áreas visíveis para o usuário autenticado (ligadas E com acesso — ADR 017 + 019).
final visibleAreasProvider = Provider<Set<ProductArea>>((ref) {
  final areas = ref.watch(areasProvider);
  final auth = ref.watch(authProvider);
  return <ProductArea>{
    for (final area in ProductArea.values)
      if (areas.isEnabled(area) && auth.hasAccess(area)) area,
  };
});

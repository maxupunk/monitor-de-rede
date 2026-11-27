import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';

/// A análise de um console pela IA. Cada pedido é uma instância de
/// [AiLogAnalysisRequest] (o console no momento do clique); "Analisar de
/// novo" é `ref.invalidate` sobre a mesma instância. Some ao fechar a folha.
final aiLogAnalysisProvider = FutureProvider.autoDispose
    .family<AiLogAnalysisReport, AiLogAnalysisRequest>((ref, request) {
      return ref
          .read(aiRepositoryProvider)
          .analyzeLog(source: request.source, content: request.content);
    });

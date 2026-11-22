import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';

/// Repositório de Plugins, Aplicativos de Frota, Execuções, Revelação Efêmera e Aprovações.
/// (Credenciais e chave dedicada SSH foram unificadas em `DeviceAccessController` — ADR 016.)
class PluginsRepository {
  const PluginsRepository(this._api);

  final ApiClient _api;

  // --- Biblioteca & Preview ---

  Future<List<PluginSummary>> fetchLibrary() async {
    final raw = await _api.get<List<dynamic>>('/plugins');
    return asJsonList<PluginSummary>(raw, PluginSummary.fromJson);
  }

  Future<PluginDetail> fetchDetail(int id) async {
    final json = await _api.get<Map<String, dynamic>>('/plugins/$id');
    return PluginDetail.fromJson(json);
  }

  Future<PluginDetail> createPlugin(PluginPackage pkg, {int? deviceId}) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins',
      data: <String, dynamic>{'package': pkg.toJson(), 'deviceId': deviceId},
    );
    return PluginDetail.fromJson(json);
  }

  Future<PluginDetail> updatePlugin(int id, PluginPackage pkg) async {
    final json = await _api.put<Map<String, dynamic>>(
      '/plugins/$id',
      data: <String, dynamic>{'package': pkg.toJson()},
    );
    return PluginDetail.fromJson(json);
  }

  /// Importa pacote externo — entra sempre em `quarantine` (ADR 012).
  Future<PluginDetail> importPlugin(PluginPackage pkg, {int? deviceId}) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins/import',
      data: <String, dynamic>{'package': pkg.toJson(), 'deviceId': deviceId},
    );
    return PluginDetail.fromJson(json);
  }

  Future<void> deletePlugin(int id) async {
    await _api.delete<dynamic>('/plugins/$id');
  }

  Future<DownloadResult> exportPlugin(int id) {
    return _api.downloadBytes('/plugins/$id/export');
  }

  Future<PluginDetail> lifecycle(int id, String step) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins/$id/$step',
      data: const <String, dynamic>{},
    );
    return PluginDetail.fromJson(json);
  }

  Future<PluginDetail> acceptReview(
    int id, {
    required bool acknowledgeRisk,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins/$id/accept-review',
      data: <String, dynamic>{'acknowledgeRisk': acknowledgeRisk},
    );
    return PluginDetail.fromJson(json);
  }

  Future<PluginTestReport> runTests(int id) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins/$id/test',
      data: const <String, dynamic>{},
      timeout: const Duration(seconds: 60),
    );
    return PluginTestReport.fromJson(json);
  }

  Future<PluginPreviewResult> previewPackage(PluginPackage pkg) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins/preview',
      data: <String, dynamic>{'package': pkg.toJson()},
      timeout: const Duration(seconds: 60),
    );
    return PluginPreviewResult.fromJson(json);
  }

  Future<Map<int, List<Map<String, dynamic>>>> fetchTopologyActions({
    List<int>? deviceIds,
  }) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/plugins/topology-actions',
      queryParameters: deviceIds != null && deviceIds.isNotEmpty
          ? <String, dynamic>{'deviceIds': deviceIds.join(',')}
          : null,
    );
    final result = <int, List<Map<String, dynamic>>>{};
    json.forEach((k, v) {
      final id = int.tryParse(k);
      if (id != null) {
        result[id] = asJsonMapList(v);
      }
    });
    return result;
  }

  // --- Aba do dispositivo, Runs & Reveal ---

  Future<DevicePluginsView> fetchDevicePlugins(int deviceId) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/devices/$deviceId/plugins',
    );
    return DevicePluginsView.fromJson(json);
  }

  Future<DevicePluginsView> installOnDevice(int deviceId, int pluginId) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/devices/$deviceId/plugins/$pluginId/install',
      data: const <String, dynamic>{},
    );
    return DevicePluginsView.fromJson(json);
  }

  Future<DevicePluginsView> uninstallFromDevice(
    int deviceId,
    int pluginId,
  ) async {
    final json = await _api.delete<Map<String, dynamic>>(
      '/devices/$deviceId/plugins/$pluginId/install',
    );
    return DevicePluginsView.fromJson(json);
  }

  Future<Map<String, dynamic>> saveDeviceSettings(
    int deviceId,
    int pluginId,
    Map<String, dynamic> value,
  ) async {
    final json = await _api.put<Map<String, dynamic>>(
      '/devices/$deviceId/plugins/$pluginId/settings',
      data: <String, dynamic>{'value': value},
    );
    return json;
  }

  Future<int> runAction({
    required int deviceId,
    required int pluginId,
    required String action,
    Map<String, dynamic> params = const <String, dynamic>{},
    bool confirmWrite = false,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/devices/$deviceId/plugins/$pluginId/actions/${Uri.encodeComponent(action)}',
      data: <String, dynamic>{'params': params, 'confirmWrite': confirmWrite},
    );
    return parseIntOrNull(json['runId']) ?? 0;
  }

  /// Ação `reveals: true` (ADR 012): chamada direta efêmera, somente admin.
  /// NUNCA grava em histórico, log ou SSE.
  Future<Map<String, dynamic>> revealSecret({
    required int deviceId,
    required int pluginId,
    required String action,
    Map<String, dynamic> params = const <String, dynamic>{},
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/devices/$deviceId/plugins/$pluginId/reveal/${Uri.encodeComponent(action)}',
      data: <String, dynamic>{'params': params},
      timeout: const Duration(seconds: 30),
    );
    return asJsonMap(json['output']);
  }

  Future<int> validateOnDevice(int deviceId, int pluginId) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/devices/$deviceId/plugins/$pluginId/validate',
      data: const <String, dynamic>{},
    );
    return parseIntOrNull(json['runId']) ?? 0;
  }

  /// Mantido por compatibilidade de contrato; o fluxo canônico usa `DeviceAccessController.fetchAccess`.
  Future<DeviceAccessView> fetchAccess(int deviceId) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/devices/$deviceId/access',
    );
    return DeviceAccessViewModel.fromJson(json, fallbackDeviceId: deviceId);
  }

  Future<PluginRunView> fetchRun(int runId) async {
    final json = await _api.get<Map<String, dynamic>>('/plugin-runs/$runId');
    return PluginRunView.fromJson(json);
  }

  Future<void> cancelRun(int runId) async {
    try {
      await _api.post<dynamic>(
        '/plugin-runs/$runId/cancel',
        data: const <String, dynamic>{},
      );
    } on ApiException catch (err) {
      if (!err.isNotFound) rethrow;
    }
  }

  Future<void> respondApproval(String key, {required bool approved}) async {
    await _api.post<dynamic>(
      '/plugin-approvals/${Uri.encodeComponent(key)}',
      data: <String, dynamic>{'approved': approved},
    );
  }

  // --- Fleet Apps & Batches ---

  Future<List<PluginApp>> fetchApps() async {
    final raw = await _api.get<List<dynamic>>('/plugins/apps');
    return asJsonList<PluginApp>(raw, PluginApp.fromJson);
  }

  Future<FleetView> fetchFleet(int pluginId) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/plugins/$pluginId/fleet',
    );
    return FleetView.fromJson(json);
  }

  Future<Map<String, dynamic>> saveFleetSettings(
    int pluginId,
    Map<String, dynamic> value,
  ) async {
    return _api.put<Map<String, dynamic>>(
      '/plugins/$pluginId/settings',
      data: <String, dynamic>{'value': value},
    );
  }

  Future<int> runFleetAction({
    required int pluginId,
    required String action,
    required List<int> deviceIds,
    required bool confirmWrite,
    Map<String, dynamic> params = const <String, dynamic>{},
    Map<int, Map<String, dynamic>>? deviceParams,
  }) async {
    final encodedDeviceParams = deviceParams == null
        ? null
        : <String, dynamic>{
            for (final entry in deviceParams.entries)
              entry.key.toString(): entry.value,
          };
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins/$pluginId/fleet/actions/${Uri.encodeComponent(action)}',
      data: <String, dynamic>{
        'deviceIds': deviceIds,
        'params': params,
        'confirmWrite': confirmWrite,
        'deviceParams': ?encodedDeviceParams,
      },
    );
    return parseIntOrNull(json['batchId']) ?? 0;
  }

  Future<FleetView> identifyFleet(int pluginId, List<int> deviceIds) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugins/$pluginId/fleet/identify',
      data: <String, dynamic>{'deviceIds': deviceIds},
      timeout: const Duration(seconds: 120),
    );
    return FleetView.fromJson(json);
  }

  Future<void> cancelBatch(int batchId) async {
    await _api.post<dynamic>(
      '/plugin-batches/$batchId/cancel',
      data: const <String, dynamic>{},
    );
  }

  Future<PluginBatchView> applyBatchPatch(int batchId) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugin-batches/$batchId/apply-patch',
      data: const <String, dynamic>{},
    );
    return PluginBatchView.fromJson(json);
  }
}

final pluginsRepositoryProvider = Provider<PluginsRepository>((ref) {
  return PluginsRepository(ref.watch(apiClientProvider));
});

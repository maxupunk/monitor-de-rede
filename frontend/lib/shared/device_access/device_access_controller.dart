import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/shared/device_access/device_access_models.dart';

/// Pedido imperativo de abertura do modal de credenciais (`DeviceAccessHost`).
@immutable
class DeviceAccessRequest {
  const DeviceAccessRequest({
    required this.deviceId,
    this.deviceName,
    this.transport = 'ssh',
    this.lockedTransport,
    this.message,
  });

  final int deviceId;
  final String? deviceName;
  final String transport;
  final String? lockedTransport;
  final String? message;
}

/// Controlador centralizado de credenciais e chave SSH dedicada (ADR 016).
/// Independente de `features/network/` e `features/plugins/` (DIP — ADR 017).
class DeviceAccessController extends Notifier<DeviceAccessRequest?> {
  Completer<bool>? _pendingCompleter;

  @override
  DeviceAccessRequest? build() => null;

  /// Solicita abertura do modal global `DeviceAccessHost` e aguarda conclusão.
  Future<bool> requestAccess({
    required int deviceId,
    String? deviceName,
    String transport = 'ssh',
    String? lockedTransport,
    String? message,
  }) {
    if (_pendingCompleter != null && !_pendingCompleter!.isCompleted) {
      _pendingCompleter!.complete(false);
    }
    final completer = Completer<bool>();
    _pendingCompleter = completer;
    state = DeviceAccessRequest(
      deviceId: deviceId,
      deviceName: deviceName,
      transport: transport,
      lockedTransport: lockedTransport,
      message: message,
    );
    return completer.future;
  }

  /// Finaliza o pedido ativo do `DeviceAccessHost`.
  void settle(bool saved) {
    final completer = _pendingCompleter;
    _pendingCompleter = null;
    state = null;
    if (completer != null && !completer.isCompleted) {
      completer.complete(saved);
    }
  }

  /// Carrega `/devices/:id/access` e enriquece nome/plataforma via `/devices/:id` se necessário.
  Future<DeviceAccessViewModel> fetchAccess(int deviceId) async {
    final api = ref.read(apiClientProvider);
    final raw = await api.get<dynamic>('/devices/$deviceId/access');
    var view = DeviceAccessViewModel.fromJson(
      asJsonMap(raw),
      fallbackDeviceId: deviceId,
    );

    if (view.deviceName.isEmpty || (view.platformHint ?? '').isEmpty) {
      try {
        final devRaw = await api.get<dynamic>('/devices/$deviceId');
        final dev = asJsonMap(devRaw);
        final resolvedName = (dev['name'] ?? dev['ipAddress'] ?? '').toString();
        final hintParts = <String>[
          if (view.dedicatedPlatform != null &&
              view.dedicatedPlatform!.isNotEmpty)
            view.dedicatedPlatform!,
          if (dev['osName'] != null) dev['osName'].toString(),
          if (dev['vendor'] != null) dev['vendor'].toString(),
          if (dev['model'] != null) dev['model'].toString(),
        ].where((s) => s.trim().isNotEmpty).join(' ');
        view = view.copyWith(
          deviceName: view.deviceName.isNotEmpty
              ? view.deviceName
              : resolvedName,
          platformHint: hintParts.isNotEmpty ? hintParts : view.platformHint,
        );
      } catch (_) {
        // Metadados extras são opcionais; a visão de `/access` já basta.
      }
    }
    return view;
  }

  /// Salva credencial de sessão temporária (30 min em memória na central — `storage: 'ask'`).
  Future<void> saveSessionCredential({
    required int deviceId,
    required String kind,
    required String username,
    required String secret,
    int? port,
    bool https = false,
    int? viaProbeId,
  }) async {
    await ref
        .read(apiClientProvider)
        .put<dynamic>(
          '/devices/$deviceId/credentials',
          data: <String, dynamic>{
            'kind': kind,
            'username': username.trim(),
            'storage': 'ask',
            if (secret.isNotEmpty) 'secret': secret,
            if (port != null && port > 0) 'port': port,
            if (kind == 'http') 'https': https,
            'viaProbeId': viaProbeId,
          },
        );
  }

  /// Remove uma credencial salva de transporte específico.
  Future<void> deleteCredential({
    required int deviceId,
    required String kind,
  }) async {
    await ref
        .read(apiClientProvider)
        .delete<dynamic>('/devices/$deviceId/credentials/$kind');
  }

  /// Provisiona conta SSH dedicada por par de chaves Ed25519 (ADR 016).
  Future<void> provisionDedicatedKey({
    required int deviceId,
    required String platform,
    required String username,
    required String password,
    int port = 22,
    int? viaProbeId,
  }) async {
    await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/devices/$deviceId/access/dedicated',
          timeout: const Duration(seconds: 45),
          data: <String, dynamic>{
            'platform': platform,
            'username': username.trim(),
            'password': password,
            'port': port,
            'viaProbeId': ?viaProbeId,
          },
        );
  }

  /// Rotaciona a chave SSH dedicada existente no equipamento e na central.
  Future<void> rotateDedicatedKey(int deviceId) async {
    await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/devices/$deviceId/access/dedicated/rotate',
          timeout: const Duration(seconds: 45),
        );
  }

  /// Revoga o acesso dedicado no equipamento (ou remove apenas na central com `localOnly: true`).
  Future<void> revokeDedicatedKey(
    int deviceId, {
    bool localOnly = false,
  }) async {
    await ref
        .read(apiClientProvider)
        .delete<dynamic>(
          '/devices/$deviceId/access/dedicated',
          timeout: const Duration(seconds: 30),
          data: <String, dynamic>{'localOnly': localOnly},
        );
  }
}

final deviceAccessProvider =
    NotifierProvider<DeviceAccessController, DeviceAccessRequest?>(
      DeviceAccessController.new,
    );

import 'dart:async';
import 'dart:convert';

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:netmonitor_app/core/config/server_config.dart';

/// Código de negócio retornado pelo backend quando falta credencial de dispositivo.
const String missingCredentialCode = 'missing_credential';

/// Mensagem padrão em PT-BR quando um equipamento remoto recusa login/senha (HTTP 401)
/// sem corpo detalhado.
const String defaultDeviceAuthRejectedMessage =
    'Usuário ou senha recusados pelo equipamento.';

/// Padrões de rotas onde um HTTP 401 indica falha de credencial no alvo remoto,
/// chave externa ou tentativa de login/setup — e NUNCA expiração do JWT da sessão NetMonitor.
final List<RegExp> _nonSession401Patterns = <RegExp>[
  RegExp(r'^/auth/(login|setup|magic-link|verify|forgot|reset)(/|$)'),
  RegExp(r'^/devices/[^/]+/access(/|$)'),
  RegExp(r'^/devices/[^/]+/credentials(/|$)'),
  RegExp(r'^/devices/[^/]+/plugins(/|$)'),
  RegExp(r'^/logs/devices/[^/]+/provision'),
  RegExp(r'^/plugins(/|$)'),
  RegExp(r'^/plugin-(batches|runs|approvals)(/|$)'),
  RegExp(r'^/snmp(/|$)'),
  RegExp(r'^/(network-tools|tools)(/|$)'),
  RegExp(r'^/databases(/|$)'),
  RegExp(r'^/storages(/|$)'),
  RegExp(r'^/ai/(ollama|decider|providers)(/|$)'),
  RegExp(r'^/ai/settings/test'),
  RegExp(r'^/deploys(/|$)'),
];

/// Subconjunto de rotas cujo alvo direto é um equipamento gerenciado (SSH/Telnet/HTTP).
final List<RegExp> _deviceTargetAuthPatterns = <RegExp>[
  RegExp(r'^/devices/[^/]+/access(/|$)'),
  RegExp(r'^/devices/[^/]+/credentials(/|$)'),
  RegExp(r'^/devices/[^/]+/plugins(/|$)'),
  RegExp(r'^/logs/devices/[^/]+/provision'),
  RegExp(r'^/plugins(/|$)'),
  RegExp(r'^/(network-tools|tools)(/|$)'),
];

String _normalizeApiPath(String rawPath) {
  var clean = rawPath.trim();
  final queryIndex = clean.indexOf('?');
  if (queryIndex != -1) clean = clean.substring(0, queryIndex);
  final hashIndex = clean.indexOf('#');
  if (hashIndex != -1) clean = clean.substring(0, hashIndex);
  if (!clean.startsWith('/')) clean = '/$clean';
  if (clean == '/api') {
    clean = '/';
  } else if (clean.startsWith('/api/')) {
    clean = clean.substring(4);
  }
  return clean;
}

/// Retorna `true` se um HTTP 401 nesta rota nunca deve deslogar a sessão do operador.
bool isNonSession401Path(String rawPath) {
  final clean = _normalizeApiPath(rawPath);
  return _nonSession401Patterns.any((pattern) => pattern.hasMatch(clean));
}

bool _isDeviceTargetAuthPath(String rawPath) {
  final clean = _normalizeApiPath(rawPath);
  return _deviceTargetAuthPatterns.any((pattern) => pattern.hasMatch(clean));
}

bool _isGenericUnauthorizedText(String text) {
  final lower = text.trim().toLowerCase();
  return lower.isEmpty ||
      lower == 'unauthorized' ||
      lower == '401 unauthorized' ||
      lower == '401: unauthorized' ||
      lower == 'unauthenticated' ||
      lower == 'invalid credentials' ||
      lower == 'erro http 401' ||
      lower == 'erro http 401: unauthorized' ||
      lower.startsWith('<!doctype html') ||
      lower.startsWith('<html');
}

const Duration _defaultTimeout = Duration(seconds: 15);

/// Token Bearer mantido em memória para o `ApiClient` e o `SseClient` sem ciclo com `AuthNotifier`.
final sessionTokenProvider = StateProvider<String?>((ref) => null);

/// Contador incrementado quando a API responde 401 em rota protegida.
final sessionExpiredSignalProvider = StateProvider<int>((ref) => 0);

@immutable
class ApiFieldError {
  const ApiFieldError({this.field, required this.message});

  final String? field;
  final String message;

  factory ApiFieldError.fromJson(Map<String, dynamic> json) {
    return ApiFieldError(
      field: json['field'] as String?,
      message: (json['message'] as String?) ?? 'Valor inválido.',
    );
  }
}

@immutable
class ApiErrorPayload {
  const ApiErrorPayload({
    this.message,
    this.error,
    this.description,
    this.code,
    this.deviceId,
    this.transport,
    this.errors = const <ApiFieldError>[],
  });

  final String? message;
  final String? error;
  final String? description;
  final String? code;
  final int? deviceId;
  final String? transport;
  final List<ApiFieldError> errors;

  static ApiErrorPayload? tryFromDynamic(Object? raw) {
    if (raw is String && raw.trim().isNotEmpty) {
      try {
        final decoded = jsonDecode(raw);
        return tryFromDynamic(decoded);
      } catch (_) {
        return ApiErrorPayload(message: raw.trim());
      }
    }
    if (raw is! Map) return null;
    final json = Map<String, dynamic>.from(raw);
    final rawErrors = json['errors'];
    final fieldErrors = <ApiFieldError>[];
    if (rawErrors is List) {
      for (final item in rawErrors) {
        if (item is Map) {
          fieldErrors.add(
            ApiFieldError.fromJson(Map<String, dynamic>.from(item)),
          );
        }
      }
    }
    final rawDeviceId = json['deviceId'];
    return ApiErrorPayload(
      message: json['message'] as String?,
      error: json['error'] as String?,
      description: json['description'] as String?,
      code: json['code'] as String?,
      deviceId: rawDeviceId is num ? rawDeviceId.toInt() : null,
      transport: json['transport'] as String?,
      errors: List.unmodifiable(fieldErrors),
    );
  }
}

/// Erro estruturado de resposta HTTP (>= 400), espelhando `ApiError` do Vue.
class ApiException implements Exception {
  const ApiException({
    required this.message,
    required this.statusCode,
    this.payload,
  });

  final String message;
  final int statusCode;
  final ApiErrorPayload? payload;

  bool get isUnauthorized => statusCode == 401;
  bool get isForbidden => statusCode == 403;
  bool get isNotFound => statusCode == 404;
  bool get isConflict => statusCode == 409;
  bool get isMissingCredential => payload?.code == missingCredentialCode;
  String? get code => payload?.code;
  int? get deviceId => payload?.deviceId;
  String? get transport => payload?.transport;
  List<ApiFieldError> get fieldErrors =>
      payload?.errors ?? const <ApiFieldError>[];

  String? fieldError(String field) {
    for (final err in fieldErrors) {
      if (err.field == field) return err.message;
    }
    return null;
  }

  @override
  String toString() => message;
}

/// Erro de rede ou timeout antes de receber resposta HTTP.
class NetworkException implements Exception {
  const NetworkException([this.message = 'Falha de conexão com o servidor.']);

  final String message;

  @override
  String toString() => message;
}

@immutable
class DownloadResult {
  const DownloadResult({
    required this.bytes,
    required this.contentType,
    this.filename,
  });

  final Uint8List bytes;
  final String contentType;
  final String? filename;
}

/// Resultado da drenagem de linhas NDJSON completas (`frontend/src/services/ndjson.ts`).
@immutable
class NdjsonDrainResult {
  const NdjsonDrainResult({required this.events, required this.remainder});

  final List<Map<String, dynamic>> events;
  final String remainder;
}

/// Consome apenas linhas completas de um fluxo NDJSON sem perder objetos partidos entre chunks.
NdjsonDrainResult drainNdjson(String buffer, {bool isFinal = false}) {
  final lines = buffer.split(RegExp(r'\r?\n'));
  final remainder = isFinal ? '' : (lines.isNotEmpty ? lines.removeLast() : '');
  final events = <Map<String, dynamic>>[];

  for (final line in lines) {
    final trimmed = line.trim();
    if (trimmed.isEmpty) continue;
    final decoded = jsonDecode(trimmed);
    if (decoded is Map) {
      events.add(Map<String, dynamic>.from(decoded));
    }
  }

  if (isFinal && remainder.trim().isNotEmpty) {
    final decoded = jsonDecode(remainder.trim());
    if (decoded is Map) {
      events.add(Map<String, dynamic>.from(decoded));
    }
  }

  return NdjsonDrainResult(events: events, remainder: remainder);
}

/// Cliente HTTP centralizado com Dio (REST, upload, download) + `http` (NDJSON/SSE POST).
class ApiClient {
  ApiClient({
    required this.baseUrlProvider,
    required this.tokenProvider,
    required this.onUnauthorized,
    Dio? dio,
  }) : _dio =
           dio ??
           Dio(
             BaseOptions(
               connectTimeout: _defaultTimeout,
               receiveTimeout: _defaultTimeout,
               sendTimeout: _defaultTimeout,
               headers: const <String, String>{'Accept': 'application/json'},
               validateStatus: (_) => true,
             ),
           ) {
    _dio.interceptors.add(
      InterceptorsWrapper(
        onRequest: (options, handler) {
          final token = tokenProvider();
          if (token != null && token.isNotEmpty) {
            options.headers['Authorization'] = 'Bearer $token';
          }
          handler.next(options);
        },
      ),
    );
  }

  final String Function() baseUrlProvider;
  final String? Function() tokenProvider;
  final VoidCallback onUnauthorized;
  final Dio _dio;

  String _buildUrl(String path) {
    final normalizedPath = path.startsWith('/') ? path : '/$path';
    return '${baseUrlProvider()}$normalizedPath';
  }

  String _normalizeWhitespace(String text) {
    return text.replaceAll(RegExp(r'[ \t]{2,}'), ' ').trim();
  }

  String? _extractPayloadMessage(ApiErrorPayload? payload) {
    if (payload?.message != null && payload!.message!.trim().isNotEmpty) {
      return _normalizeWhitespace(payload.message!);
    }
    if (payload?.description != null &&
        payload!.description!.trim().isNotEmpty) {
      final desc = _normalizeWhitespace(payload.description!);
      if (payload.error != null && payload.error!.trim().isNotEmpty) {
        final err = _normalizeWhitespace(payload.error!);
        if (err.toLowerCase() != desc.toLowerCase() &&
            !_isGenericUnauthorizedText(err)) {
          return '$err: $desc';
        }
      }
      return desc;
    }
    if (payload?.error != null && payload!.error!.trim().isNotEmpty) {
      return _normalizeWhitespace(payload.error!);
    }
    if (payload != null && payload.errors.isNotEmpty) {
      return payload.errors
          .map((e) => _normalizeWhitespace(e.message))
          .join(', ');
    }
    return null;
  }

  String _extractErrorMessage(
    int statusCode,
    String? statusMessage,
    ApiErrorPayload? payload, {
    String? path,
  }) {
    final payloadMessage = _extractPayloadMessage(payload);
    if (statusCode == 401 && path != null && _isDeviceTargetAuthPath(path)) {
      if (payloadMessage == null ||
          _isGenericUnauthorizedText(payloadMessage)) {
        return defaultDeviceAuthRejectedMessage;
      }
    }
    if (payloadMessage != null && payloadMessage.isNotEmpty) {
      return payloadMessage;
    }
    return 'Erro HTTP $statusCode${statusMessage != null && statusMessage.isNotEmpty ? ': $statusMessage' : ''}';
  }

  bool _shouldExpireSessionOn401(String path, ApiErrorPayload? payload) {
    if (isNonSession401Path(path)) {
      return false;
    }
    final code = payload?.code?.toLowerCase();
    if (code == 'auth_failed' ||
        code == 'invalid_credentials' ||
        code == missingCredentialCode) {
      return false;
    }
    final combined = <String?>[
      payload?.message,
      payload?.description,
      payload?.error,
    ].whereType<String>().join(' ').toLowerCase();
    if (combined.contains('equipamento') ||
        combined.contains('chave de api') ||
        combined.contains('token do github') ||
        combined.contains('assinatura do github')) {
      return false;
    }
    return true;
  }

  ApiException _buildApiException(
    int statusCode,
    String? statusMessage,
    String path,
    Object? rawData,
  ) {
    final normalizedPath = _normalizeApiPath(path);
    final payload = ApiErrorPayload.tryFromDynamic(rawData);
    if (statusCode == 401 &&
        _shouldExpireSessionOn401(normalizedPath, payload)) {
      onUnauthorized();
    }
    return ApiException(
      message: _extractErrorMessage(
        statusCode,
        statusMessage,
        payload,
        path: normalizedPath,
      ),
      statusCode: statusCode,
      payload: payload,
    );
  }

  T _handleResponse<T>(Response<dynamic> response, String path) {
    final statusCode = response.statusCode ?? 0;
    if (statusCode < 200 || statusCode >= 300) {
      throw _buildApiException(
        statusCode,
        response.statusMessage,
        path,
        response.data,
      );
    }

    if (statusCode == 204 || response.data == null) {
      return _emptyOrCast<T>();
    }

    final data = response.data;
    if (data is String) {
      if (data.trim().isEmpty) return _emptyOrCast<T>();
      return jsonDecode(data) as T;
    }
    return data as T;
  }

  T _emptyOrCast<T>() {
    if (<String, dynamic>{} is T) {
      return <String, dynamic>{} as T;
    }
    return null as T;
  }

  Future<T> _runRequest<T>(
    String path,
    Future<Response<dynamic>> Function(String url) executor,
  ) async {
    try {
      final response = await executor(_buildUrl(path));
      return _handleResponse<T>(response, path);
    } on ApiException {
      rethrow;
    } on DioException catch (err) {
      if (err.type == DioExceptionType.cancel) rethrow;
      if (err.response != null) {
        throw _buildApiException(
          err.response!.statusCode ?? 500,
          err.response!.statusMessage,
          path,
          err.response!.data,
        );
      }
      throw const NetworkException();
    }
  }

  Options _jsonOptions({Duration? timeout}) => Options(
    contentType: Headers.jsonContentType,
    receiveTimeout: timeout ?? _defaultTimeout,
    sendTimeout: timeout ?? _defaultTimeout,
  );

  Future<T> get<T>(
    String path, {
    Map<String, dynamic>? queryParameters,
    Duration? timeout,
    CancelToken? cancelToken,
  }) {
    return _runRequest<T>(
      path,
      (url) => _dio.get<dynamic>(
        url,
        queryParameters: queryParameters,
        options: _jsonOptions(timeout: timeout),
        cancelToken: cancelToken,
      ),
    );
  }

  Future<T> post<T>(
    String path, {
    Object? data,
    Map<String, dynamic>? queryParameters,
    Duration? timeout,
    CancelToken? cancelToken,
  }) {
    return _runRequest<T>(
      path,
      (url) => _dio.post<dynamic>(
        url,
        data: data,
        queryParameters: queryParameters,
        options: _jsonOptions(timeout: timeout),
        cancelToken: cancelToken,
      ),
    );
  }

  Future<T> put<T>(
    String path, {
    Object? data,
    Map<String, dynamic>? queryParameters,
    Duration? timeout,
    CancelToken? cancelToken,
  }) {
    return _runRequest<T>(
      path,
      (url) => _dio.put<dynamic>(
        url,
        data: data,
        queryParameters: queryParameters,
        options: _jsonOptions(timeout: timeout),
        cancelToken: cancelToken,
      ),
    );
  }

  Future<T> patch<T>(
    String path, {
    Object? data,
    Map<String, dynamic>? queryParameters,
    Duration? timeout,
    CancelToken? cancelToken,
  }) {
    return _runRequest<T>(
      path,
      (url) => _dio.patch<dynamic>(
        url,
        data: data,
        queryParameters: queryParameters,
        options: _jsonOptions(timeout: timeout),
        cancelToken: cancelToken,
      ),
    );
  }

  Future<T> delete<T>(
    String path, {
    Object? data,
    Map<String, dynamic>? queryParameters,
    Duration? timeout,
    CancelToken? cancelToken,
  }) {
    return _runRequest<T>(
      path,
      (url) => _dio.delete<dynamic>(
        url,
        data: data,
        queryParameters: queryParameters,
        options: _jsonOptions(timeout: timeout),
        cancelToken: cancelToken,
      ),
    );
  }

  /// Alias explícito para POST de bytes crus (backup preview/restore, speedtest LAN upload).
  Future<T> postRawBytes<T>(
    String path,
    Uint8List bytes, {
    String contentType = 'application/octet-stream',
    ProgressCallback? onSendProgress,
    CancelToken? cancelToken,
  }) => uploadRaw<T>(
    path,
    bytes,
    contentType: contentType,
    onSendProgress: onSendProgress,
    cancelToken: cancelToken,
  );

  /// Envia corpo binário cru (paridade com `apiService.upload` do Vue).
  Future<T> uploadRaw<T>(
    String path,
    Uint8List bytes, {
    String contentType = 'application/octet-stream',
    ProgressCallback? onSendProgress,
    CancelToken? cancelToken,
  }) {
    return _runRequest<T>(
      path,
      (url) => _dio.post<dynamic>(
        url,
        data: bytes,
        onSendProgress: onSendProgress,
        cancelToken: cancelToken,
        options: Options(
          contentType: contentType,
          sendTimeout: const Duration(minutes: 10),
          receiveTimeout: const Duration(minutes: 10),
        ),
      ),
    );
  }

  /// Envia payload `multipart/form-data`.
  Future<T> uploadMultipart<T>(
    String path,
    FormData formData, {
    ProgressCallback? onSendProgress,
    CancelToken? cancelToken,
  }) {
    return _runRequest<T>(
      path,
      (url) => _dio.post<dynamic>(
        url,
        data: formData,
        onSendProgress: onSendProgress,
        cancelToken: cancelToken,
        options: Options(
          sendTimeout: const Duration(minutes: 10),
          receiveTimeout: const Duration(minutes: 10),
        ),
      ),
    );
  }

  /// Baixa arquivo binário com progresso e detecção de `Content-Disposition`.
  Future<DownloadResult> downloadBytes(
    String path, {
    Map<String, dynamic>? queryParameters,
    ProgressCallback? onReceiveProgress,
    CancelToken? cancelToken,
  }) async {
    try {
      final response = await _dio.get<List<int>>(
        _buildUrl(path),
        queryParameters: queryParameters,
        onReceiveProgress: onReceiveProgress,
        cancelToken: cancelToken,
        options: Options(
          responseType: ResponseType.bytes,
          receiveTimeout: const Duration(minutes: 10),
        ),
      );
      final statusCode = response.statusCode ?? 0;
      if (statusCode < 200 || statusCode >= 300) {
        final rawText = response.data != null
            ? utf8.decode(response.data!, allowMalformed: true)
            : null;
        throw _buildApiException(
          statusCode,
          response.statusMessage,
          path,
          rawText,
        );
      }

      final disposition = response.headers.value('content-disposition') ?? '';
      final match = RegExp(r'filename="?([^";]+)"?').firstMatch(disposition);
      final contentType =
          response.headers.value('content-type') ?? 'application/octet-stream';
      return DownloadResult(
        bytes: Uint8List.fromList(response.data ?? const <int>[]),
        contentType: contentType,
        filename: match?.group(1),
      );
    } on ApiException {
      rethrow;
    } on DioException catch (err) {
      if (err.type == DioExceptionType.cancel) rethrow;
      throw const NetworkException();
    }
  }

  /// POST com resposta em streaming NDJSON (Chat IA, varreduras, pull de modelos).
  Stream<Map<String, dynamic>> streamNdjson(
    String path, {
    Object? body,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) async* {
    final client = http.Client();
    onCancelAvailable?.call(client.close);

    try {
      final request = http.Request('POST', Uri.parse(_buildUrl(path)))
        ..headers['Content-Type'] = 'application/json'
        ..headers['Accept'] = 'application/x-ndjson, application/json';

      final token = tokenProvider();
      if (token != null && token.isNotEmpty) {
        request.headers['Authorization'] = 'Bearer $token';
      }
      if (body != null) {
        request.body = jsonEncode(body);
      }

      final response = await client.send(request);
      if (response.statusCode < 200 || response.statusCode >= 300) {
        final errorBody = await response.stream.bytesToString();
        throw _buildApiException(
          response.statusCode,
          response.reasonPhrase,
          path,
          errorBody,
        );
      }

      var buffer = '';
      await for (final chunk in response.stream.transform(utf8.decoder)) {
        buffer += chunk;
        final drained = drainNdjson(buffer);
        buffer = drained.remainder;
        for (final event in drained.events) {
          yield event;
        }
      }
      if (buffer.trim().isNotEmpty) {
        final drained = drainNdjson(buffer, isFinal: true);
        for (final event in drained.events) {
          yield event;
        }
      }
    } on ApiException {
      rethrow;
    } catch (err) {
      if (err is http.ClientException) {
        throw const NetworkException();
      }
      rethrow;
    } finally {
      client.close();
    }
  }

  /// POST com resposta `text/event-stream` (`data: {...}` — paridade com `sseReader.ts`).
  Stream<Map<String, dynamic>> streamSsePost(
    String path, {
    Object? body,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) async* {
    final client = http.Client();
    onCancelAvailable?.call(client.close);

    try {
      final request = http.Request('POST', Uri.parse(_buildUrl(path)))
        ..headers['Content-Type'] = 'application/json'
        ..headers['Accept'] = 'text/event-stream';

      final token = tokenProvider();
      if (token != null && token.isNotEmpty) {
        request.headers['Authorization'] = 'Bearer $token';
      }
      if (body != null) {
        request.body = jsonEncode(body);
      }

      final response = await client.send(request);
      if (response.statusCode < 200 || response.statusCode >= 300) {
        final errorBody = await response.stream.bytesToString();
        throw _buildApiException(
          response.statusCode,
          response.reasonPhrase,
          path,
          errorBody,
        );
      }

      var buffer = '';
      await for (final chunk in response.stream.transform(utf8.decoder)) {
        buffer += chunk;
        final lines = buffer.split('\n');
        buffer = lines.isNotEmpty ? lines.removeLast() : '';
        for (final line in lines) {
          final parsed = _parseSseDataLine(line);
          if (parsed != null) yield parsed;
        }
      }
      if (buffer.trim().isNotEmpty) {
        final parsed = _parseSseDataLine(buffer);
        if (parsed != null) yield parsed;
      }
    } on ApiException {
      rethrow;
    } catch (err) {
      if (err is http.ClientException) {
        throw const NetworkException();
      }
      rethrow;
    } finally {
      client.close();
    }
  }

  Map<String, dynamic>? _parseSseDataLine(String line) {
    final trimmed = line.trim();
    if (!trimmed.startsWith('data:')) return null;
    final raw = trimmed.substring(5).trim();
    if (raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      return decoded is Map ? Map<String, dynamic>.from(decoded) : null;
    } catch (_) {
      return null;
    }
  }
}

final apiClientProvider = Provider<ApiClient>((ref) {
  return ApiClient(
    baseUrlProvider: () => ref.read(serverConfigProvider).apiBaseUrl,
    tokenProvider: () => ref.read(sessionTokenProvider),
    onUnauthorized: () {
      ref.read(sessionTokenProvider.notifier).state = null;
      ref.read(sessionExpiredSignalProvider.notifier).state++;
    },
  );
});

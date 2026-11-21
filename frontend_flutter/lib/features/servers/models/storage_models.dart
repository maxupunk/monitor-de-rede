import 'package:flutter/foundation.dart';

/// Destino de backup listado em `GET /api/storages`.
@immutable
class StorageDestinationResponse {
  const StorageDestinationResponse({
    required this.id,
    required this.name,
    required this.provider,
    this.target,
    this.createdAt,
    this.updatedAt,
  });

  final int id;
  final String name;
  final String provider;
  final String? target;
  final String? createdAt;
  final String? updatedAt;

  bool get hasUnreadableCredentials => target == null;

  factory StorageDestinationResponse.fromJson(Map<String, dynamic> json) {
    return StorageDestinationResponse(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      provider: (json['provider'] as String?) ?? 'local',
      target: json['target'] as String?,
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
    );
  }
}

/// Detalhe de destino (`GET /api/storages/{id}`), sem segredos em claro.
@immutable
class StorageDestinationDetail {
  const StorageDestinationDetail({
    required this.id,
    required this.name,
    required this.provider,
    this.target,
    required this.config,
    required this.secretsSet,
    this.createdAt,
    this.updatedAt,
  });

  final int id;
  final String name;
  final String provider;
  final String? target;
  final Map<String, dynamic> config;
  final List<String> secretsSet;
  final String? createdAt;
  final String? updatedAt;

  StorageDestinationResponse toSummary() => StorageDestinationResponse(
    id: id,
    name: name,
    provider: provider,
    target: target,
    createdAt: createdAt,
    updatedAt: updatedAt,
  );

  factory StorageDestinationDetail.fromJson(Map<String, dynamic> json) {
    final rawSecrets = json['secretsSet'];
    return StorageDestinationDetail(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      provider: (json['provider'] as String?) ?? 'local',
      target: json['target'] as String?,
      config: json['config'] is Map
          ? Map<String, dynamic>.from(json['config'] as Map)
          : const <String, dynamic>{},
      secretsSet: rawSecrets is List
          ? List<String>.unmodifiable(rawSecrets.whereType<String>())
          : const <String>[],
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
    );
  }
}

@immutable
class StorageDestinationInput {
  const StorageDestinationInput({
    required this.name,
    required this.provider,
    required this.config,
  });

  final String name;
  final String provider;
  final Map<String, dynamic> config;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'provider': provider,
    'config': config,
  };
}

@immutable
class StoragesMetaResponse {
  const StoragesMetaResponse({required this.localRoot});
  final String localRoot;

  factory StoragesMetaResponse.fromJson(Map<String, dynamic> json) {
    return StoragesMetaResponse(
      localRoot: (json['localRoot'] as String?) ?? 'storage/',
    );
  }
}

@immutable
class StorageTestResponse {
  const StorageTestResponse({
    required this.ok,
    required this.message,
    this.hostKeyFingerprint,
  });

  final bool ok;
  final String message;
  final String? hostKeyFingerprint;

  factory StorageTestResponse.fromJson(Map<String, dynamic> json) {
    return StorageTestResponse(
      ok: json['ok'] == true,
      message: (json['message'] as String?) ?? '',
      hostKeyFingerprint: json['hostKeyFingerprint'] as String?,
    );
  }
}

@immutable
class StorageObject {
  const StorageObject({
    required this.key,
    required this.name,
    required this.size,
    this.lastModified,
    required this.isDirectory,
  });

  final String key;
  final String name;
  final int size;
  final String? lastModified;
  final bool isDirectory;

  factory StorageObject.fromJson(Map<String, dynamic> json) {
    return StorageObject(
      key: (json['key'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      size: (json['size'] as num?)?.toInt() ?? 0,
      lastModified: json['lastModified'] as String?,
      isDirectory: json['isDirectory'] == true,
    );
  }
}

@immutable
class StorageBrowseResponse {
  const StorageBrowseResponse({
    required this.path,
    required this.objects,
    this.nextCursor,
  });

  final String path;
  final List<StorageObject> objects;
  final String? nextCursor;

  factory StorageBrowseResponse.fromJson(Map<String, dynamic> json) {
    final rawObjs = json['objects'];
    return StorageBrowseResponse(
      path: (json['path'] as String?) ?? '',
      objects: rawObjs is List
          ? List<StorageObject>.unmodifiable(
              rawObjs.whereType<Map>().map(
                (e) => StorageObject.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <StorageObject>[],
      nextCursor: json['nextCursor'] as String?,
    );
  }
}

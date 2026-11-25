import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// O que a API devolve no lugar de um segredo gravado. Reenviar a máscara (ou
/// deixar o campo vazio) mantém o segredo que estava.
const String kSnmpSecretMask = '********';

/// Versões SNMP aceitas por uma credencial: `(valor, rótulo)`.
const List<(String, String)> kSnmpVersionOptions = <(String, String)>[
  ('v1', 'SNMP v1'),
  ('v2c', 'SNMP v2c'),
  ('v3', 'SNMP v3'),
];

/// Protocolos de autenticação do SNMPv3 (os mesmos do backend).
const List<(String, String)> kSnmpAuthProtocols = <(String, String)>[
  ('md5', 'MD5'),
  ('sha', 'SHA-1'),
  ('sha224', 'SHA-224'),
  ('sha256', 'SHA-256'),
  ('sha384', 'SHA-384'),
  ('sha512', 'SHA-512'),
];

/// Protocolos de privacidade do SNMPv3.
const List<(String, String)> kSnmpPrivProtocols = <(String, String)>[
  ('des', 'DES'),
  ('3des', '3DES'),
  ('aes128', 'AES-128'),
  ('aes192', 'AES-192'),
  ('aes256', 'AES-256'),
];

/// Rótulo do nível de segurança devolvido pela API.
String snmpSecurityLevelLabel(String level) {
  switch (level) {
    case 'authPriv':
      return 'Autenticação + criptografia';
    case 'authNoPriv':
      return 'Só autenticação';
    case 'noAuthNoPriv':
      return 'Sem autenticação';
    default:
      return 'Comunidade';
  }
}

/// Uma credencial SNMP reutilizável (`/api/snmp/credentials`). Os segredos
/// chegam mascarados ([kSnmpSecretMask]) ou nulos — nunca em claro.
@immutable
class SnmpCredentialModel {
  const SnmpCredentialModel({
    required this.id,
    required this.name,
    required this.version,
    required this.securityLevel,
    this.community,
    this.username,
    this.authProtocol,
    this.authKey,
    this.privProtocol,
    this.privKey,
    this.contextName,
    this.isDefault = false,
    this.deviceCount = 0,
  });

  final int id;
  final String name;
  final String version;
  final String securityLevel;
  final String? community;
  final String? username;
  final String? authProtocol;
  final String? authKey;
  final String? privProtocol;
  final String? privKey;
  final String? contextName;
  final bool isDefault;
  final int deviceCount;

  bool get isV3 => version == 'v3';
  bool get hasCommunity => community != null;
  bool get hasAuthKey => authKey != null;
  bool get hasPrivKey => privKey != null;

  /// Resumo de uma linha: `SNMP v2c · Comunidade` ou `SNMP v3 · usuário · …`.
  String get summary {
    final versionLabel = kSnmpVersionOptions
        .firstWhere(
          (option) => option.$1 == version,
          orElse: () => (version, version),
        )
        .$2;
    return <String>[
      versionLabel,
      if (isV3 && username != null) username!,
      snmpSecurityLevelLabel(securityLevel),
    ].join(' · ');
  }

  factory SnmpCredentialModel.fromJson(Map<String, dynamic> json) {
    return SnmpCredentialModel(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] ?? '').toString(),
      version: (json['version'] ?? 'v2c').toString(),
      securityLevel: (json['securityLevel'] ?? 'community').toString(),
      community: json['community'] as String?,
      username: json['username'] as String?,
      authProtocol: json['authProtocol'] as String?,
      authKey: json['authKey'] as String?,
      privProtocol: json['privProtocol'] as String?,
      privKey: json['privKey'] as String?,
      contextName: json['contextName'] as String?,
      isDefault: parseBool(json['isDefault']),
      deviceCount: parseIntOrNull(json['deviceCount']) ?? 0,
    );
  }
}

/// Corpo de criação/edição. Segredo vazio é omitido: na edição, mantém o
/// gravado.
@immutable
class SnmpCredentialDraft {
  const SnmpCredentialDraft({
    required this.name,
    required this.version,
    this.community = '',
    this.username = '',
    this.authProtocol,
    this.authKey = '',
    this.privProtocol,
    this.privKey = '',
    this.contextName = '',
    this.isDefault = false,
  });

  final String name;
  final String version;
  final String community;
  final String username;
  final String? authProtocol;
  final String authKey;
  final String? privProtocol;
  final String privKey;
  final String contextName;
  final bool isDefault;

  Map<String, dynamic> toJson() {
    final v3 = version == 'v3';
    String? text(String value) => value.trim().isEmpty ? null : value.trim();
    String? secret(String value) => value.isEmpty ? null : value;
    return <String, dynamic>{
      'name': name.trim(),
      'version': version,
      'isDefault': isDefault,
      if (!v3) 'community': ?secret(community.trim()),
      if (v3) ...<String, dynamic>{
        'username': ?text(username),
        'authProtocol': ?authProtocol,
        if (authProtocol != null) 'authKey': ?secret(authKey),
        if (authProtocol != null) 'privProtocol': ?privProtocol,
        if (authProtocol != null && privProtocol != null)
          'privKey': ?secret(privKey),
        'contextName': ?text(contextName),
      },
    };
  }
}

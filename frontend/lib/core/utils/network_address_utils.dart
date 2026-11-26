import 'package:flutter/foundation.dart';

/// Representação estruturada de uma sub-rede IPv4 em notação CIDR (`a.b.c.d/prefix`).
@immutable
class ParsedIpv4Cidr {
  const ParsedIpv4Cidr({
    required this.network,
    required this.mask,
    required this.prefix,
  });

  final int network;
  final int mask;
  final int prefix;

  String get addressText {
    final a = (network >>> 24) & 0xff;
    final b = (network >>> 16) & 0xff;
    final c = (network >>> 8) & 0xff;
    final d = network & 0xff;
    return '$a.$b.$c.$d';
  }

  String get cidr => '$addressText/$prefix';
}

/// Faz o parse estrito de um bloco IPv4 CIDR (`10.8.0.0/24`).
ParsedIpv4Cidr? parseIpv4Cidr(String raw) {
  final parts = raw.trim().split('/');
  if (parts.length != 2) return null;
  final prefix = int.tryParse(parts[1]);
  if (prefix == null || prefix < 0 || prefix > 32) return null;

  final octets = parts[0].split('.');
  if (octets.length != 4) return null;

  var ip = 0;
  for (final octet in octets) {
    final n = int.tryParse(octet);
    if (n == null || n < 0 || n > 255) return null;
    ip = ((ip << 8) | n) & 0xffffffff;
  }

  final mask = prefix == 0 ? 0 : ((0xffffffff << (32 - prefix)) & 0xffffffff);
  return ParsedIpv4Cidr(network: ip & mask, mask: mask, prefix: prefix);
}

/// Retorna `true` se duas faixas IPv4 CIDR se sobrepõem.
bool cidrOverlaps(String a, String b) {
  final pa = parseIpv4Cidr(a);
  final pb = parseIpv4Cidr(b);
  if (pa == null || pb == null) return false;
  final commonMask = pa.mask & pb.mask;
  return (pa.network & commonMask) == (pb.network & commonMask);
}

const List<String> _vpnCandidateCidrs = <String>[
  '10.8.0.0/24',
  '10.9.0.0/24',
  '10.10.0.0/24',
  '10.20.0.0/24',
  '172.28.0.0/24',
  '192.168.250.0/24',
];

/// Sugere faixas privadas `/24` para VPN que não colidem com redes já conhecidas.
List<String> freeVpnCidrs(Iterable<String> existingCidrs) {
  final clean = existingCidrs
      .map((c) => c.trim())
      .where((c) => c.isNotEmpty)
      .toList(growable: false);
  return _vpnCandidateCidrs
      .where((cand) => !clean.any((used) => cidrOverlaps(cand, used)))
      .toList(growable: false);
}

/// Ordena endereços IPv4 numericamente octeto a octeto (com fallback lexicográfico).
int compareIpAddresses(String a, String b) {
  final pa = a.trim().split('.');
  final pb = b.trim().split('.');
  if (pa.length == 4 && pb.length == 4) {
    for (var i = 0; i < 4; i++) {
      final na = int.tryParse(pa[i]);
      final nb = int.tryParse(pb[i]);
      if (na == null || nb == null) break;
      if (na != nb) return na.compareTo(nb);
    }
  }
  return a.compareTo(b);
}

/// Verifica se um hostname ou IPv4 pertence a rede privada/local (RFC 1918, loopback, CGNAT/VPN, `.local`).
bool isPrivateHost(String host) {
  final h = host.trim().toLowerCase();
  if (h.isEmpty) return false;
  if (h == 'localhost' ||
      h == '127.0.0.1' ||
      h == '::1' ||
      h.endsWith('.local') ||
      h.endsWith('.lan') ||
      h.endsWith('.internal')) {
    return true;
  }
  final parts = h.split('.');
  if (parts.length == 4) {
    final a = int.tryParse(parts[0]);
    final b = int.tryParse(parts[1]);
    final c = int.tryParse(parts[2]);
    final d = int.tryParse(parts[3]);
    if (a != null && b != null && c != null && d != null) {
      if (a == 10 || a == 127) return true;
      if (a == 192 && b == 168) return true;
      if (a == 172 && b >= 16 && b <= 31) return true;
      if (a == 100 && b >= 64 && b <= 127) return true;
    }
  }
  return false;
}

/// Verifica se uma URL aponta para um host privado/interno.
bool looksPrivateUrl(String rawUrl) {
  final uri = Uri.tryParse(rawUrl.trim());
  if (uri == null || uri.host.isEmpty) return false;
  return isPrivateHost(uri.host);
}

/// `true` se o texto é um IP literal (IPv4 ou IPv6, com ou sem colchetes).
bool isIpLiteral(String raw) {
  var value = raw.trim();
  if (value.startsWith('[') && value.endsWith(']')) {
    value = value.substring(1, value.length - 1);
  }
  if (value.isEmpty) return false;
  try {
    Uri.parseIPv4Address(value);
    return true;
  } on FormatException {
    // Não é IPv4; tenta IPv6 abaixo.
  }
  try {
    Uri.parseIPv6Address(value);
    return true;
  } on FormatException {
    return false;
  }
}

/// Sugere o endpoint público (DDNS) a partir do host pelo qual a central é
/// acessada — um host (`monitor.exemplo.com.br:8443`) ou uma URL completa.
///
/// Devolve só o nome de domínio, sem esquema, porta ou caminho. Endereço IP
/// (IPv4/IPv6) e `localhost` não servem de sugestão: devolve `null`.
String? suggestPublicEndpoint(String? hostOrUrl) {
  final value = hostOrUrl?.trim() ?? '';
  if (value.isEmpty || isIpLiteral(value)) return null;
  final uri = Uri.tryParse(value.contains('://') ? value : 'http://$value');
  var host = (uri?.host ?? '').toLowerCase();
  while (host.endsWith('.')) {
    host = host.substring(0, host.length - 1);
  }
  if (host.isEmpty ||
      isIpLiteral(host) ||
      host == 'localhost' ||
      host.endsWith('.localhost')) {
    return null;
  }
  return host;
}

/// Resolve a origem HTTP/HTTPS efetiva da central (Web PWA vs cliente nativo).
String resolveCentralOrigin(String configuredBaseUrl) {
  return kIsWeb ? Uri.base.origin : configuredBaseUrl;
}

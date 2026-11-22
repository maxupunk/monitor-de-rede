import 'package:netmonitor_app/core/network/api_client.dart';

const List<String> _ubiquitiTokens = <String>[
  'ubiquiti',
  'unifi',
  'edgeos',
  'edgemax',
  'airmax',
  'airos',
  'ubnt',
];

const List<String> _adminTokens = <String>[
  'mikrotik',
  'routeros',
  'cisco',
  'fortios',
  'fortigate',
  'fortinet',
  'tp-link',
  'tplink',
  'intelbras',
  'huawei',
];

const List<String> _rootTokens = <String>[
  'openwrt',
  'lede',
  'linux',
  'debian',
  'ubuntu',
  'alpine',
  'proxmox',
  'freebsd',
  'pfsense',
  'opnsense',
  'vyos',
];

/// Retorna o usuário administrativo padrão para acesso SSH/Telnet/HTTP conforme a
/// plataforma ou sistema operacional do equipamento.
///
/// No OpenWrt e distribuições Linux/BSD o login padrão é `root` (e não `admin`);
/// em contextos SSH dedicados sem plataforma identificada também assume `root`.
String defaultDeviceAdminUsername(String? platformOrOs) {
  final normalized = (platformOrOs ?? '').trim().toLowerCase();
  if (normalized.isEmpty) {
    return 'root';
  }
  for (final token in _ubiquitiTokens) {
    if (normalized.contains(token)) {
      return 'ubnt';
    }
  }
  for (final token in _adminTokens) {
    if (normalized.contains(token)) {
      return 'admin';
    }
  }
  for (final token in _rootTokens) {
    if (normalized.contains(token)) {
      return 'root';
    }
  }
  return 'root';
}

/// Indica se o erro corresponde a falha de autenticação/autorização no equipamento remoto.
bool isDeviceAuthFailure(Object? error) {
  if (error is! ApiException) return false;
  final code = error.code?.toLowerCase();
  if (error.statusCode == 401 ||
      error.statusCode == 403 ||
      code == 'auth_failed' ||
      code == 'invalid_credentials') {
    return true;
  }
  final msg = error.message.toLowerCase();
  return msg.contains('usuário ou senha recusados') ||
      msg.contains('recusou a chave') ||
      msg.contains('recusou o login') ||
      msg.contains('authentication failed');
}

/// Formata mensagem clara em PT-BR para falhas de login em equipamentos gerenciados,
/// incluindo dica contextual do usuário padrão da plataforma (ex.: `root` no OpenWrt).
String formatDeviceAuthFailureMessage(Object? error, {String? platformOrOs}) {
  if (error is ApiException) {
    final lowerMsg = error.message.toLowerCase();
    if (lowerMsg.contains('chave do acesso dedicado')) {
      return error.message;
    }
  }

  final normalized = (platformOrOs ?? '').trim().toLowerCase();
  final defaultUser = defaultDeviceAdminUsername(platformOrOs);

  String hint;
  if (normalized.contains('openwrt') || normalized.contains('lede')) {
    hint = "No OpenWrt o usuário padrão é 'root'.";
  } else if (normalized.contains('mikrotik') ||
      normalized.contains('routeros')) {
    hint = "No MikroTik/RouterOS o usuário padrão é 'admin'.";
  } else if (defaultUser == 'ubnt') {
    hint = "Em equipamentos Ubiquiti o usuário padrão costuma ser 'ubnt'.";
  } else if (normalized.isNotEmpty) {
    hint = "Para este sistema o usuário padrão costuma ser '$defaultUser'.";
  } else {
    hint =
        "No OpenWrt/Linux o usuário padrão é 'root' (e 'admin' no MikroTik).";
  }

  return 'O equipamento recusou o login informado. $hint '
      'Verifique o usuário e a senha administrativa e tente novamente.';
}

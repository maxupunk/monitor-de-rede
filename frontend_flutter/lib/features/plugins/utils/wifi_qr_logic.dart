// Formato ZXing `WIFI:` para conectar celulares por câmera (paridade com `wifiQr.ts`).

String wifiQrSecurity(String encryption) {
  final value = encryption.trim().toLowerCase();
  if (value.isEmpty || value == 'none' || value.startsWith('owe')) {
    return 'nopass';
  }
  if (value.startsWith('wep')) return 'WEP';
  return 'WPA';
}

bool wifiNeedsPassword(String encryption) =>
    wifiQrSecurity(encryption) != 'nopass';

String _escapeWifiQr(String text) {
  return text.replaceAllMapped(RegExp(r'([\\;,:"])'), (m) => '\\${m.group(1)}');
}

/// Constrói o payload ZXing: `WIFI:T:WPA;S:MinhaRede;P:senha;;`.
String wifiQrPayload({
  required String ssid,
  required String encryption,
  String? password,
  bool hidden = false,
}) {
  final security = wifiQrSecurity(encryption);
  final parts = <String>[
    'T:$security',
    'S:${_escapeWifiQr(ssid)}',
    if (security != 'nopass') 'P:${_escapeWifiQr(password ?? '')}',
    if (hidden) 'H:true',
  ];
  return 'WIFI:${parts.join(';')};;';
}

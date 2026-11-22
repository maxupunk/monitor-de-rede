import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_fleet_models.dart';

/// Valor retornado pelo backend no lugar de um segredo salvo.
const String pluginSecretMask = '********';

@immutable
class PluginVisibleWhen {
  const PluginVisibleWhen({
    required this.field,
    this.equals,
    this.inValues,
    this.notInValues,
  });

  final String field;
  final Object? equals;
  final List<Object?>? inValues;
  final List<Object?>? notInValues;

  static PluginVisibleWhen? tryParse(Object? raw) {
    if (raw is! Map) return null;
    final field = raw['field'];
    if (field is! String || field.isEmpty) return null;
    return PluginVisibleWhen(
      field: field,
      equals: raw['equals'],
      inValues: raw['in'] is List
          ? List<Object?>.from(raw['in'] as List)
          : null,
      notInValues: raw['notIn'] is List
          ? List<Object?>.from(raw['notIn'] as List)
          : null,
    );
  }

  bool holds(Map<String, dynamic> values) {
    final current = values[field];
    if (inValues != null) return inValues!.contains(current);
    if (notInValues != null) return !notInValues!.contains(current);
    return current == equals;
  }
}

@immutable
class PluginSchemaField {
  const PluginSchemaField({
    required this.name,
    required this.kind,
    required this.label,
    this.hint,
    this.options,
    this.titles,
    this.advanced = false,
    this.hidden = false,
    this.widget,
    this.group,
    this.visibleWhen,
    this.reveal,
    this.secret = false,
    this.required = false,
    this.items,
    this.schema = const <String, dynamic>{},
  });

  final String name;
  final String
  kind; // 'string' | 'integer' | 'number' | 'boolean' | 'list' | 'objects'
  final String label;
  final String? hint;
  final List<Object?>? options;
  final List<String>? titles;
  final bool advanced;
  final bool hidden;
  final String? widget; // ip|cidr|mac|port|hostname|url|textarea|slider|tags
  final String? group;
  final PluginVisibleWhen? visibleWhen;
  final String? reveal;
  final bool secret;
  final bool required;
  final Map<String, dynamic>? items;
  final Map<String, dynamic> schema;
}

final RegExp _macColonRe = RegExp(r'^([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}$');
final RegExp _macDashRe = RegExp(r'^([0-9a-fA-F]{2}-){5}[0-9a-fA-F]{2}$');
final RegExp _hostLabelRe = RegExp(
  r'^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$',
);

bool _isIpv4(String text) => FormValidators.ipv4()(text) == null;

bool _isIpv6(String text) {
  if (!RegExp(r'^[0-9a-fA-F:.]+$').hasMatch(text) || !text.contains(':')) {
    return false;
  }
  if (text.split('::').length - 1 > 1) return false;
  final groups = text.split(':').where((g) => g.isNotEmpty).toList();
  return groups.length <= 8 &&
      groups.every(
        (g) => _isIpv4(g) || RegExp(r'^[0-9a-fA-F]{1,4}$').hasMatch(g),
      );
}

String? validatePluginWidget(String? widget, String raw) {
  if (widget == null || raw.isEmpty || raw == pluginSecretMask) return null;
  switch (widget) {
    case 'ip':
      return (_isIpv4(raw) || _isIpv6(raw))
          ? null
          : 'Precisa ser um endereço IP (ex.: 192.168.1.1)';
    case 'cidr':
      if (FormValidators.cidr()(raw) == null) return null;
      final parts = raw.split('/');
      if (parts.length == 2) {
        final bits = int.tryParse(parts[1]);
        if (bits != null && _isIpv6(parts[0]) && bits >= 0 && bits <= 128) {
          return null;
        }
      }
      return 'Precisa ser uma rede no formato 192.168.1.0/24';
    case 'mac':
      return (_macColonRe.hasMatch(raw) || _macDashRe.hasMatch(raw))
          ? null
          : 'Precisa ser um MAC (ex.: AA:BB:CC:DD:EE:FF)';
    case 'port':
      return FormValidators.port('Precisa ser uma porta de 1 a 65535')(raw);
    case 'hostname':
      final ok =
          raw.isNotEmpty &&
          raw.length <= 253 &&
          raw.split('.').every((label) => _hostLabelRe.hasMatch(label));
      return ok
          ? null
          : 'Precisa ser um nome de host (letras, números, hífen e ponto)';
    case 'url':
      return FormValidators.url('Precisa ser um endereço http:// ou https://')(
        raw,
      );
    default:
      return null;
  }
}

String? validatePluginField(PluginSchemaField field, Object? value) {
  final text = value?.toString() ?? '';
  if (field.required && (value == null || text.isEmpty)) {
    return 'Obrigatório';
  }
  if (text.isEmpty || text == pluginSecretMask) return null;

  final pattern = field.schema['pattern'];
  if (pattern is String && pattern.isNotEmpty) {
    if (!RegExp('^(?:$pattern)\$').hasMatch(text)) {
      return 'Formato inválido';
    }
  }
  final minLen = field.schema['minLength'];
  if (minLen is num && text.length < minLen.toInt()) {
    return 'Ao menos ${minLen.toInt()} caracteres';
  }
  final maxLen = field.schema['maxLength'];
  if (maxLen is num && text.length > maxLen.toInt()) {
    return 'No máximo ${maxLen.toInt()} caracteres';
  }
  final minVal = field.schema['minimum'];
  if (minVal is num) {
    final parsed = num.tryParse(text);
    if (parsed == null || parsed < minVal) return 'Mínimo $minVal';
  }
  final maxVal = field.schema['maximum'];
  if (maxVal is num) {
    final parsed = num.tryParse(text);
    if (parsed == null || parsed > maxVal) return 'Máximo $maxVal';
  }
  return validatePluginWidget(field.widget, text);
}

List<PluginSchemaField> parsePluginSchemaFields(Map<String, dynamic>? schema) {
  if (schema == null) return const <PluginSchemaField>[];
  final rawProps = schema['properties'];
  if (rawProps is! Map) return const <PluginSchemaField>[];
  final requiredSet = asStringList(schema['required']).toSet();

  final fields = <PluginSchemaField>[];
  rawProps.forEach((key, rawProp) {
    if (rawProp is! Map) return;
    final prop = asJsonMap(rawProp);
    final name = key.toString();
    final type = (prop['type'] ?? 'string').toString();
    final items = prop['items'] is Map ? asJsonMap(prop['items']) : null;
    final kind = type == 'array'
        ? (items?['type'] == 'object' ? 'objects' : 'list')
        : (const <String>['integer', 'number', 'boolean'].contains(type)
              ? type
              : 'string');
    fields.add(
      PluginSchemaField(
        name: name,
        kind: kind,
        label: prop['title'] is String ? prop['title'] as String : name,
        hint: prop['description'] as String?,
        options: prop['enum'] is List
            ? List<Object?>.from(prop['enum'] as List)
            : null,
        titles: prop['enumTitles'] is List
            ? asStringList(prop['enumTitles'])
            : null,
        advanced: parseBool(prop['advanced']),
        hidden: parseBool(prop['hidden']),
        widget: prop['widget'] as String?,
        group: prop['group'] as String?,
        visibleWhen: PluginVisibleWhen.tryParse(prop['visibleWhen']),
        reveal: prop['reveal'] as String?,
        secret: parseBool(prop['secret']),
        required: requiredSet.contains(name),
        items: items,
        schema: prop,
      ),
    );
  });

  final order = schema['order'];
  if (order is List) {
    final orderNames = asStringList(order);
    fields.sort((a, b) {
      final ia = orderNames.indexOf(a.name);
      final ib = orderNames.indexOf(b.name);
      final pa = ia < 0 ? orderNames.length : ia;
      final pb = ib < 0 ? orderNames.length : ib;
      return pa.compareTo(pb);
    });
  }
  return fields;
}

Map<String, dynamic> pluginSchemaDefaults(Map<String, dynamic>? schema) {
  final result = <String, dynamic>{};
  for (final field in parsePluginSchemaFields(schema)) {
    if (field.schema.containsKey('default')) {
      result[field.name] = field.schema['default'];
    } else if (field.kind == 'boolean') {
      result[field.name] = false;
    } else if (field.kind == 'list' || field.kind == 'objects') {
      result[field.name] = <dynamic>[];
    } else if (field.kind == 'string') {
      result[field.name] = '';
    }
  }
  return result;
}

bool isPluginFieldVisible(
  PluginSchemaField field,
  Map<String, dynamic>? schema,
  Map<String, dynamic> values,
) {
  if (field.visibleWhen == null) return true;
  final effective = <String, dynamic>{
    ...pluginSchemaDefaults(schema),
    ...values,
  };
  return field.visibleWhen!.holds(effective);
}

Map<String, dynamic> normalizePluginFormParams(
  Map<String, dynamic>? schema,
  Map<String, dynamic> values, {
  bool omitEmpty = true,
}) {
  final out = Map<String, dynamic>.from(values);
  for (final field in parsePluginSchemaFields(schema)) {
    if (!isPluginFieldVisible(field, schema, values)) {
      out.remove(field.name);
      continue;
    }
    final current = out[field.name];
    if (field.kind == 'integer' || field.kind == 'number') {
      if (current == null || current.toString().trim().isEmpty) {
        out.remove(field.name);
      } else {
        final parsed = field.kind == 'integer'
            ? parseIntOrNull(current)
            : parseDoubleOrNull(current);
        if (parsed != null) out[field.name] = parsed;
      }
    }
    if (omitEmpty && (out[field.name] == null || out[field.name] == '')) {
      out.remove(field.name);
    }
  }
  return out;
}

/// Limpa prefixos pontuados redundantes e humaniza razões de compatibilidade.
List<String> cleanPluginReasonMessages(List<String> rawReasons) {
  final out = <String>[];
  for (final raw in rawReasons) {
    var cleaned = raw.replaceFirst(RegExp(r'^[\s·•\-;,]+'), '').trim();
    if (cleaned.isEmpty) continue;
    final lower = cleaned.toLowerCase();
    if (lower.contains('firmware ainda não foi lido')) {
      cleaned = 'Firmware ainda não identificado — toque em Detectar sistema.';
    } else {
      cleaned = '${cleaned[0].toUpperCase()}${cleaned.substring(1)}';
    }
    out.add(cleaned);
  }
  return out;
}

/// Mensagem amigável de status para candidatos de frota.
String formatCandidateStatusMessage(FleetCandidate candidate) {
  final cleaned = cleanPluginReasonMessages(candidate.reasons);
  if (cleaned.isNotEmpty) {
    return cleaned.join(' · ');
  }
  return switch (candidate.compatibility) {
    'validated' ||
    'likely' => 'Sistema compatível pronto para entrar na frota.',
    'incompatible' =>
      'Sistema incompatível com os requisitos deste aplicativo.',
    _ => 'Firmware ainda não identificado — toque em Detectar sistema.',
  };
}

const Map<String, String> _commonPluginKeyLabels = <String, String>{
  'ssid': 'SSID',
  'bssid': 'BSSID',
  'band': 'Banda',
  'bands': 'Bandas',
  'channel': 'Canal',
  'channels': 'Canais',
  'current_channel': 'Canal atual',
  'suggested_channel': 'Canal sugerido',
  'width': 'Largura do canal',
  'channel_width': 'Largura do canal',
  'htmode': 'Largura do canal',
  'encryption': 'Segurança',
  'security': 'Segurança',
  'auth': 'Autenticação',
  'clients': 'Clientes',
  'stations': 'Clientes',
  'connected_clients': 'Clientes conectados',
  'hidden': 'Rede oculta',
  'disabled': 'Desativada',
  'enabled': 'Ativa',
  'status': 'Estado',
  'state': 'Estado',
  'mode': 'Modo',
  'radio': 'Rádio',
  'radios': 'Rádios',
  'frequency': 'Frequência',
  'freq': 'Frequência',
  'tx_power': 'Potência TX',
  'txpower': 'Potência TX',
  'signal': 'Sinal',
  'rssi': 'Sinal (RSSI)',
  'noise': 'Ruído',
  'vlan': 'VLAN',
  'vlan_id': 'VLAN',
  'network': 'Interface',
  'interface': 'Interface',
  'ip': 'Endereço IP',
  'ip_address': 'Endereço IP',
  'mac': 'Endereço MAC',
  'mac_address': 'Endereço MAC',
  'model': 'Modelo',
  'modelo': 'Modelo',
  'firmware': 'Firmware',
  'version': 'Versão',
  'uptime': 'Tempo ativo',
  'hostname': 'Hostname',
  'platform': 'Plataforma',
  'rx_bytes': 'Tráfego RX',
  'tx_bytes': 'Tráfego TX',
  'redes_ativas': 'Redes ativas',
  'active_networks': 'Redes ativas',
};

const Set<String> _upperAcronyms = <String>{
  'ip',
  'mac',
  'vlan',
  'ssid',
  'bssid',
  'ssh',
  'vpn',
  'dns',
  'dhcp',
  'cpu',
  'ram',
  'rx',
  'tx',
  'qr',
  'snmp',
  'http',
  'https',
  'rssi',
};

/// Humaniza e traduz chaves técnicas de saída de plugins (`band` -> `'Banda'`,
/// `encryption` -> `'Segurança'`, `clients` -> `'Clientes'`, `redesAtivas` -> `'Redes ativas'`).
String humanizePluginFieldKey(String rawKey, {Map<String, String>? labels}) {
  final explicit = labels?[rawKey];
  if (explicit != null && explicit.trim().isNotEmpty) {
    return explicit.trim();
  }
  final trimmed = rawKey.trim();
  if (trimmed.isEmpty) return '';

  // Converte camelCase/PascalCase e hífens em snake_case para lookup unificado.
  final snake = trimmed
      .replaceAllMapped(
        RegExp(r'([a-z0-9])([A-Z])'),
        (m) => '${m.group(1)}_${m.group(2)}',
      )
      .replaceAllMapped(
        RegExp(r'([A-Z]+)([A-Z][a-z])'),
        (m) => '${m.group(1)}_${m.group(2)}',
      )
      .replaceAll('-', '_')
      .toLowerCase();

  final mapped = _commonPluginKeyLabels[snake];
  if (mapped != null) return mapped;

  final words = snake
      .split(RegExp(r'[_\s]+'))
      .where((w) => w.isNotEmpty)
      .toList(growable: false);
  if (words.isEmpty) return trimmed;

  final formatted = <String>[];
  for (var i = 0; i < words.length; i++) {
    final w = words[i];
    if (_upperAcronyms.contains(w)) {
      formatted.add(w.toUpperCase());
    } else if (i == 0) {
      formatted.add('${w[0].toUpperCase()}${w.substring(1)}');
    } else {
      formatted.add(w);
    }
  }
  return formatted.join(' ');
}

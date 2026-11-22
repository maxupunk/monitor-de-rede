import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Áreas do produto (ADR 017): Núcleo, Rede, Servidores e VPN.
/// `core` nunca desliga.
enum ProductArea {
  core('core', 'Núcleo', Icons.hub_outlined),
  network('network', 'Rede', Icons.lan_outlined),
  servers('servers', 'Servidores', Icons.dns_outlined),
  vpn('vpn', 'VPN', Icons.vpn_lock_outlined);

  const ProductArea(this.id, this.defaultTitle, this.defaultIcon);

  final String id;
  final String defaultTitle;
  final IconData defaultIcon;

  bool get isOptional => this != ProductArea.core;

  static const List<ProductArea> optionalAreas = <ProductArea>[
    ProductArea.network,
    ProductArea.servers,
    ProductArea.vpn,
  ];

  static ProductArea? tryParse(Object? value) {
    if (value is ProductArea) return value;
    if (value is! String) return null;
    for (final area in ProductArea.values) {
      if (area.id == value) return area;
    }
    return null;
  }
}

/// Sinal persistido de uso de uma área (para sugestões).
@immutable
class AreaEvidence {
  const AreaEvidence({required this.present, this.reason});

  final bool present;
  final String? reason;

  factory AreaEvidence.fromJson(Map<String, dynamic> json) {
    return AreaEvidence(
      present: json['present'] == true,
      reason: json['reason'] as String?,
    );
  }
}

/// Estado de uma área como o backend entrega em `GET /api/areas` e `areas:updated`.
@immutable
class AreaStateModel {
  const AreaStateModel({
    required this.area,
    required this.title,
    required this.summary,
    required this.icon,
    required this.enabled,
    required this.optional,
    this.evidence,
  });

  final ProductArea area;
  final String title;
  final String summary;
  final String icon;
  final bool enabled;
  final bool optional;
  final AreaEvidence? evidence;

  static AreaStateModel? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final json = asJsonMap(raw);
    final area = ProductArea.tryParse(json['area']);
    if (area == null || json['enabled'] is! bool) return null;

    final rawEvidence = json['evidence'];
    return AreaStateModel(
      area: area,
      title: (json['title'] as String?) ?? area.defaultTitle,
      summary: (json['summary'] as String?) ?? '',
      icon: (json['icon'] as String?) ?? '',
      enabled: json['enabled'] as bool,
      optional: (json['optional'] as bool?) ?? area.isOptional,
      evidence: rawEvidence is Map
          ? AreaEvidence.fromJson(asJsonMap(rawEvidence))
          : null,
    );
  }
}

/// Snapshot completo das áreas (`GET /api/areas` e evento SSE `areas:updated`).
@immutable
class AreasSnapshot {
  const AreasSnapshot({required this.areas});

  final List<AreaStateModel> areas;

  static AreasSnapshot? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final rawAreas = raw['areas'];
    if (rawAreas is! List) return null;

    final parsed = <AreaStateModel>[];
    for (final item in rawAreas) {
      final state = AreaStateModel.tryFromJson(item);
      if (state == null) return null;
      parsed.add(state);
    }
    return AreasSnapshot(areas: List.unmodifiable(parsed));
  }
}

/// Consequências e bloqueio ao desligar uma área (`GET /api/areas/:area/impact`).
@immutable
class AreaImpact {
  const AreaImpact({
    required this.area,
    required this.consequences,
    this.blockerMessage,
  });

  final ProductArea area;
  final List<String> consequences;
  final String? blockerMessage;

  bool get isBlocked => blockerMessage != null && blockerMessage!.isNotEmpty;

  factory AreaImpact.fromJson(Map<String, dynamic> json) {
    final area = ProductArea.tryParse(json['area']) ?? ProductArea.core;
    final rawConsequences = json['consequences'];
    final consequences = <String>[];
    if (rawConsequences is List) {
      for (final item in rawConsequences) {
        if (item is Map && item['message'] is String) {
          consequences.add(item['message'] as String);
        } else if (item is String) {
          consequences.add(item);
        }
      }
    }
    final rawBlocker = json['blocker'];
    String? blockerMessage;
    if (rawBlocker is Map && rawBlocker['message'] is String) {
      blockerMessage = rawBlocker['message'] as String;
    }
    return AreaImpact(
      area: area,
      consequences: List.unmodifiable(consequences),
      blockerMessage: blockerMessage,
    );
  }
}

/// Corpo do `POST /api/settings/onboarding/intent`.
@immutable
class OnboardingIntentInput {
  const OnboardingIntentInput({
    required this.network,
    required this.servers,
    required this.vpn,
  });

  final bool network;
  final bool servers;
  final bool vpn;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'network': network,
    'servers': servers,
    'vpn': vpn,
  };
}

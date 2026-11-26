import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/models/pagination.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/device_presentation.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/core/utils/mdi_icons.dart';
import 'package:netmonitor_app/core/utils/network_address_utils.dart';
import 'package:netmonitor_app/shared/widgets/dividers_helper.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  group('json_parsers', () {
    test(
      'parseIntOrNull e parseDoubleOrNull tratam tipos mistos e NaN/Infinity',
      () {
        expect(parseIntOrNull(42), 42);
        expect(parseIntOrNull(42.9), 42);
        expect(parseIntOrNull(' 17 '), 17);
        expect(parseIntOrNull('17.8'), 17);
        expect(parseIntOrNull(double.nan), isNull);
        expect(parseIntOrNull('abc'), isNull);
        expect(parseIntOrNull(null), isNull);

        expect(parseDoubleOrNull(3.5), 3.5);
        expect(parseDoubleOrNull(10), 10.0);
        expect(parseDoubleOrNull(' 2.75 '), 2.75);
        expect(parseDoubleOrNull(double.infinity), isNull);
        expect(parseDoubleOrNull(''), isNull);
      },
    );

    test('parseBool aceita bool, números e strings usuais', () {
      expect(parseBool(true), isTrue);
      expect(parseBool(1), isTrue);
      expect(parseBool('true'), isTrue);
      expect(parseBool('YES'), isTrue);
      expect(parseBool(0), isFalse);
      expect(parseBool('false'), isFalse);
      expect(parseBool(null, fallback: true), isTrue);
    });

    test(
      'asJsonMap, asJsonMapList, asJsonList, asStringList, asIntList e asStringMap',
      () {
        expect(asJsonMap(<Object?, Object?>{'a': 1}), <String, dynamic>{
          'a': 1,
        });
        expect(asJsonMap('invalido'), isEmpty);

        final maps = asJsonMapList(<dynamic>[
          <String, dynamic>{'id': 1},
          'ignorar',
          <dynamic, dynamic>{'id': 2},
        ]);
        expect(maps.length, 2);
        expect(maps.map((m) => m['id']), <int>[1, 2]);

        final parsed = asJsonList<int>(<dynamic>[
          <String, dynamic>{'id': '10'},
          <String, dynamic>{'id': 20},
        ], (json) => parseIntOrNull(json['id']) ?? 0);
        expect(parsed, <int>[10, 20]);

        expect(asStringList(<dynamic>['a', 2, null]), <String>['a', '2']);
        expect(asIntList(<dynamic>['3', 4, 'x', null]), <int>[3, 4]);
        expect(
          asStringMap(<dynamic, dynamic>{'k': 123, 'n': null}),
          <String, String>{'k': '123'},
        );
      },
    );

    test('shortGitSha abrevia hashes com segurança', () {
      expect(shortGitSha('abcdef1234567890'), 'abcdef1');
      expect(shortGitSha('abc', length: 7), 'abc');
      expect(shortGitSha('   '), '—');
      expect(shortGitSha(null, fallback: 'n/d'), 'n/d');
    });
  });

  group('error_utils', () {
    test(
      'describeError formata ApiException, NetworkException e remove prefixos técnicos',
      () {
        expect(
          describeError(
            const ApiException(statusCode: 422, message: 'IP inválido.'),
          ),
          'IP inválido.',
        );
        expect(
          describeError(
            ApiException(
              statusCode: 422,
              message: '',
              payload: ApiErrorPayload.tryFromDynamic(const <String, dynamic>{
                'errors': <Map<String, String>>[
                  <String, String>{
                    'field': 'name',
                    'message': 'Campo obrigatório.',
                  },
                ],
              }),
            ),
          ),
          'Campo obrigatório.',
        );
        expect(
          describeError(const NetworkException('Servidor inacessível.')),
          'Servidor inacessível.',
        );
        expect(
          describeError(TimeoutException('timeout')),
          'Tempo limite esgotado ao comunicar com o servidor.',
        );
        expect(
          describeError(Exception('Falha ao validar certificado')),
          'Falha ao validar certificado',
        );
        expect(
          describeError(StateError('Estado inconsistente')),
          'Estado inconsistente',
        );
        expect(describeError(null, fallback: 'Erro padrão'), 'Erro padrão');
      },
    );
  });

  group('formatters estendidos', () {
    test(
      'parseDateOrNull, formatDecimal, formatDurationSeconds, formatSignalDbm e slugs',
      () {
        final parsed = parseDateOrNull('2026-08-05T12:30:00Z');
        expect(parsed, isNotNull);
        expect(parseDateOrNull(''), isNull);

        expect(formatDecimal(12.34, maxDigits: 1), '12,3');
        expect(formatDecimal(12.0, maxDigits: 2), '12');
        expect(formatDecimal(null), '—');

        expect(formatDurationSeconds(45), '45 s');
        expect(formatDurationSeconds(125), '2 min 5 s');
        expect(formatDurationSeconds(3600), '1 h');
        expect(formatDurationSeconds(90000), '1 dia e 1 h');

        expect(formatSignalDbm(-63.4), '-63 dBm');
        expect(formatSignalDbm(null), '—');

        final ts = formatFileTimestamp(DateTime(2026, 8, 5, 9, 4, 2));
        expect(ts, '20260805-090402');

        expect(
          sanitizeFileSlug('Relatório São Paulo / Core #1!'),
          'relatorio-sao-paulo-core-1',
        );
        expect(sanitizeFileSlug('...'), 'arquivo');
      },
    );
  });

  group('network_address_utils', () {
    test(
      'parseIpv4Cidr, cidrOverlaps, freeVpnCidrs, compareIpAddresses e isPrivateHost',
      () {
        final cidr = parseIpv4Cidr('192.168.10.45/24');
        expect(cidr, isNotNull);
        expect(cidr!.cidr, '192.168.10.0/24');
        expect(parseIpv4Cidr('999.1.1.1/24'), isNull);

        expect(cidrOverlaps('10.8.0.0/24', '10.8.0.0/16'), isTrue);
        expect(cidrOverlaps('10.8.0.0/24', '10.9.0.0/24'), isFalse);

        final free = freeVpnCidrs(<String>['10.8.0.0/16']);
        expect(free, isNot(contains('10.8.0.0/24')));
        expect(free, contains('10.9.0.0/24'));

        final ips = <String>['10.0.0.10', '10.0.0.2', '10.0.0.1']
          ..sort(compareIpAddresses);
        expect(ips, <String>['10.0.0.1', '10.0.0.2', '10.0.0.10']);

        expect(isPrivateHost('192.168.1.1'), isTrue);
        expect(isPrivateHost('100.64.0.10'), isTrue);
        expect(isPrivateHost('router.local'), isTrue);
        expect(isPrivateHost('8.8.8.8'), isFalse);
        expect(looksPrivateUrl('http://10.0.0.5:8080/api'), isTrue);
        expect(looksPrivateUrl('https://netmonitor.exemplo.com.br'), isFalse);
      },
    );

    test(
      'suggestPublicEndpoint: domínio vira sugestão; IP e localhost não',
      () {
        // Domínio, com ou sem esquema, porta e caminho.
        expect(
          suggestPublicEndpoint('monitor.master.pro.br'),
          'monitor.master.pro.br',
        );
        expect(
          suggestPublicEndpoint('https://Monitor.Master.pro.br/vpn?x=1'),
          'monitor.master.pro.br',
        );
        expect(
          suggestPublicEndpoint('monitor.master.pro.br:8443'),
          'monitor.master.pro.br',
        );
        expect(
          suggestPublicEndpoint('http://monitor.master.pro.br:5150'),
          'monitor.master.pro.br',
        );

        // IPv4, IPv6 e localhost: o campo fica vazio.
        for (final host in <String>[
          '203.0.113.7',
          '192.168.1.10:5150',
          'http://127.0.0.1:3333',
          '::1',
          '2001:db8::1',
          '[2001:db8::1]:8443',
          'https://[2001:db8::1]/',
          'localhost',
          'http://localhost:5150',
        ]) {
          expect(suggestPublicEndpoint(host), isNull, reason: host);
        }

        // Vazio.
        expect(suggestPublicEndpoint(null), isNull);
        expect(suggestPublicEndpoint(''), isNull);
        expect(suggestPublicEndpoint('   '), isNull);

        expect(isIpLiteral('10.8.0.1'), isTrue);
        expect(isIpLiteral('[fe80::1]'), isTrue);
        expect(isIpLiteral('vpn.exemplo.com'), isFalse);
      },
    );
  });

  group('device_presentation & mdi_icons', () {
    test('padroniza ícones de switch vs unmanaged_switch/hub e tons Wi-Fi', () {
      expect(deviceTypeOptions, isNotEmpty);
      expect(isUnmanagedSwitchType('unmanaged_switch'), isTrue);
      expect(isUnmanagedSwitchType('hub'), isTrue);
      expect(isUnmanagedSwitchType('switch'), isFalse);

      expect(deviceTypeIcon('switch'), Icons.lan_outlined);
      expect(deviceTypeIcon('unmanaged_switch'), Icons.hub_outlined);
      expect(deviceTypeIcon('hub'), Icons.hub_outlined);
      expect(deviceTypeLabel('router'), 'Roteador');
      expect(deviceTypeLabel('router', isSystem: true), 'Servidor NetMonitor');

      expect(wifiSignalTone(-55), SemanticTone.success);
      expect(wifiSignalTone(-68), SemanticTone.info);
      expect(wifiSignalTone(-76), SemanticTone.warning);
      expect(wifiSignalTone(-85), SemanticTone.error);
      expect(wifiSignalGradeLabel('excellent'), 'excelente');
      expect(wifiSignalGradeLabel('poor'), 'fraco');

      expect(iconFromMdiName('mdi-router-wireless'), Icons.router_outlined);
      expect(iconFromMdiName('desconhecido', fallback: Icons.help), Icons.help);
    });
  });

  group('pagination & dividers_helper', () {
    test(
      'PaginatedResult.fromDynamic aceita envelope {data, meta} e lista direta',
      () {
        final env = PaginatedResult<int>.fromDynamic(<String, dynamic>{
          'data': <Map<String, dynamic>>[
            <String, dynamic>{'v': 1},
            <String, dynamic>{'v': 2},
          ],
          'meta': <String, dynamic>{
            'total': 10,
            'perPage': 2,
            'currentPage': 1,
            'lastPage': 5,
          },
        }, (j) => j['v'] as int);
        expect(env.data, <int>[1, 2]);
        expect(env.meta.hasMore, isTrue);
        expect(env.meta.hasNext, isTrue);

        final direct = PaginatedResult<int>.fromDynamic(<Map<String, dynamic>>[
          <String, dynamic>{'v': 7},
        ], (j) => j['v'] as int);
        expect(direct.data, <int>[7]);
        expect(direct.meta.hasMore, isFalse);

        final divided = withDividers(<Widget>[
          const SizedBox(key: ValueKey<int>(1)),
          const SizedBox(key: ValueKey<int>(2)),
        ]);
        expect(divided.length, 3);
        expect(intersperseDividers(const <Widget>[]), isEmpty);
      },
    );
  });

  group('resolveRouteMetaFrom com segmentos :param', () {
    test(
      'resolve /devices/:id e /monitors/:id com maior especificidade que prefixos',
      () {
        const testModule = AreaContribution(
          id: ProductArea.core,
          routeMeta: <String, AreaRouteMeta>{
            '/devices': AreaRouteMeta(
              area: ProductArea.core,
              aiScreen: AiScreenContribution(
                id: 'devices',
                title: 'Dispositivos',
              ),
            ),
            '/devices/:id': AreaRouteMeta(
              area: ProductArea.core,
              aiScreen: AiScreenContribution(
                id: 'device-detail',
                title: 'Detalhe do Dispositivo',
              ),
            ),
            '/monitors': AreaRouteMeta(
              area: ProductArea.core,
              aiScreen: AiScreenContribution(
                id: 'monitors',
                title: 'Monitores',
              ),
            ),
            '/monitors/:id': AreaRouteMeta(
              area: ProductArea.core,
              aiScreen: AiScreenContribution(
                id: 'monitor-detail',
                title: 'Detalhe do Monitor',
              ),
            ),
          },
        );

        expect(
          resolveRouteMetaFrom(<AreaContribution>[
            testModule,
          ], '/devices')?.aiScreen?.id,
          'devices',
        );
        expect(
          resolveRouteMetaFrom(<AreaContribution>[
            testModule,
          ], '/devices/42')?.aiScreen?.id,
          'device-detail',
        );
        expect(
          resolveRouteMetaFrom(<AreaContribution>[
            testModule,
          ], '/monitors/99?tab=alerts')?.aiScreen?.id,
          'monitor-detail',
        );
        expect(resolveRouteMeta('/monitors/7')?.aiScreen?.id, 'monitor-detail');
        expect(resolveRouteMeta('/devices/15')?.aiScreen?.id, 'device-detail');
      },
    );
  });
}

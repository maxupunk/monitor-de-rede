import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';

void main() {
  group('area_access (ADR 019)', () {
    test('admin has admin role and write access in every area', () {
      for (final area in ProductArea.values) {
        expect(
          roleInArea(UserRole.admin, const <ProductArea, UserRole>{}, area),
          UserRole.admin,
        );
        expect(
          canReadArea(UserRole.admin, const <ProductArea, UserRole>{}, area),
          isTrue,
        );
        expect(
          canWriteArea(UserRole.admin, const <ProductArea, UserRole>{}, area),
          isTrue,
        );
      }
    });

    test(
      'core area follows global role while optional areas follow areaRoles',
      () {
        const roles = <ProductArea, UserRole>{
          ProductArea.network: UserRole.viewer,
          ProductArea.servers: UserRole.operator,
        };

        expect(
          roleInArea(UserRole.operator, roles, ProductArea.core),
          UserRole.operator,
        );
        expect(
          canWriteArea(UserRole.operator, roles, ProductArea.core),
          isTrue,
        );

        // Leitor na Rede: pode ler, mas não alterar.
        expect(
          roleInArea(UserRole.operator, roles, ProductArea.network),
          UserRole.viewer,
        );
        expect(
          canReadArea(UserRole.operator, roles, ProductArea.network),
          isTrue,
        );
        expect(
          canWriteArea(UserRole.operator, roles, ProductArea.network),
          isFalse,
        );

        // Operador em Servidores: pode ler e alterar.
        expect(
          canReadArea(UserRole.operator, roles, ProductArea.servers),
          isTrue,
        );
        expect(
          canWriteArea(UserRole.operator, roles, ProductArea.servers),
          isTrue,
        );

        // Sem papel atribuído em VPN: sem acesso.
        expect(roleInArea(UserRole.operator, roles, ProductArea.vpn), isNull);
        expect(canReadArea(UserRole.operator, roles, ProductArea.vpn), isFalse);
        expect(
          canWriteArea(UserRole.operator, roles, ProductArea.vpn),
          isFalse,
        );
      },
    );

    test('parseAreaRoles ignores unknown areas and invalid roles', () {
      final parsed = parseAreaRoles(<String, dynamic>{
        'core': 'operator', // core não vem em areaRoles opcionais
        'network': 'viewer',
        'servers': 'admin', // admin não é papel válido em areaRoles
        'vpn': 'operator',
        'unknown': 'operator',
      });

      expect(parsed, <ProductArea, UserRole>{
        ProductArea.network: UserRole.viewer,
        ProductArea.vpn: UserRole.operator,
      });
    });
  });
}

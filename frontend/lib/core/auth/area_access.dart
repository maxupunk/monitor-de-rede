import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';

/// Papel global ou por área do usuário (ADR 019).
enum UserRole {
  admin(
    'admin',
    'Administrador',
    'Acesso total, incluindo usuários e perfis.',
    Icons.admin_panel_settings_outlined,
  ),
  operator(
    'operator',
    'Operador',
    'Visualiza e altera o monitoramento, sem gerenciar usuários.',
    Icons.manage_accounts_outlined,
  ),
  viewer(
    'viewer',
    'Visualizador',
    'Consulta dados e eventos sem realizar alterações.',
    Icons.visibility_outlined,
  );

  const UserRole(this.id, this.label, this.description, this.icon);

  final String id;
  final String label;
  final String description;
  final IconData icon;

  static UserRole? tryParse(Object? value) {
    if (value is UserRole) return value;
    if (value is! String) return null;
    for (final role in UserRole.values) {
      if (role.id == value) return role;
    }
    return null;
  }
}

/// Papel atribuído por área (ADR 019). Área ausente: sem acesso.
typedef AreaRoles = Map<ProductArea, UserRole>;

/// Rótulo legível do papel.
String roleLabel(Object? role) {
  return UserRole.tryParse(role)?.label ?? 'Perfil desconhecido';
}

/// Papel do usuário numa área — mesma regra do backend (`AreaAccess::role_in`):
/// - `admin` vale em todas as áreas;
/// - `core` segue o papel geral;
/// - demais áreas seguem o papel atribuído (`operator`/`viewer`) ou `null`.
UserRole? roleInArea(UserRole? role, AreaRoles? areaRoles, ProductArea area) {
  if (role == UserRole.admin) return UserRole.admin;
  if (area == ProductArea.core) return role;
  final assigned = areaRoles?[area];
  if (assigned == UserRole.operator || assigned == UserRole.viewer) {
    return assigned;
  }
  return null;
}

/// O papel permite alterações?
bool roleCanWrite(UserRole? role) {
  return role == UserRole.admin || role == UserRole.operator;
}

/// O usuário pode ler dados desta área?
bool canReadArea(UserRole? role, AreaRoles? areaRoles, ProductArea area) {
  return roleInArea(role, areaRoles, area) != null;
}

/// O usuário pode alterar dados desta área?
bool canWriteArea(UserRole? role, AreaRoles? areaRoles, ProductArea area) {
  return roleCanWrite(roleInArea(role, areaRoles, area));
}

/// Faz o parsing defensivo de `areaRoles` vindo da API ou do SSE.
AreaRoles parseAreaRoles(Object? value) {
  final result = <ProductArea, UserRole>{};
  if (value is! Map) return result;
  for (final area in ProductArea.optionalAreas) {
    final parsed = UserRole.tryParse(value[area.id]);
    if (parsed == UserRole.operator || parsed == UserRole.viewer) {
      result[area] = parsed!;
    }
  }
  return result;
}

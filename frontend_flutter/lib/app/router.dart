import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/features/auth/area_off_page.dart';
import 'package:netmonitor_app/features/auth/login_page.dart';
import 'package:netmonitor_app/features/auth/server_config_page.dart';
import 'package:netmonitor_app/features/auth/setup_page.dart';
import 'package:netmonitor_app/shared/widgets/adaptive_app_shell.dart';

const Set<String> _publicPaths = <String>{
  '/login',
  '/setup',
  '/server-config',
};

/// Ponte `ChangeNotifier` que dispara reavaliação de redirect no `GoRouter`
/// sempre que autenticação, áreas ou servidor mudarem.
class _RouterRefreshNotifier extends ChangeNotifier {
  _RouterRefreshNotifier(this._ref) {
    _ref.listen<AuthState>(authProvider, (_, _) => notifyListeners());
    _ref.listen<AreasState>(areasProvider, (_, _) => notifyListeners());
    _ref.listen<ServerConfigState>(serverConfigProvider, (_, _) => notifyListeners());
  }

  final Ref _ref;
}

final appRouterProvider = Provider<GoRouter>((ref) {
  final refreshNotifier = _RouterRefreshNotifier(ref);
  ref.onDispose(refreshNotifier.dispose);

  return GoRouter(
    initialLocation: '/',
    refreshListenable: refreshNotifier,
    redirect: (context, state) async {
      final serverConfig = ref.read(serverConfigProvider);
      final auth = ref.read(authProvider);
      final location = state.uri.path;
      final fullLocation = state.uri.toString();

      // 1. No Android, exige URL de servidor antes de autenticar.
      if (!kIsWeb && !serverConfig.isConfigured) {
        return location == '/server-config' ? null : '/server-config';
      }

      // 2. Autenticado: bloqueia login/setup, valida admin e área ligada/com acesso.
      if (auth.isAuthenticated) {
        if (location == '/login' || location == '/setup') {
          return '/';
        }

        final meta = resolveRouteMeta(location);
        if (meta != null) {
          if (meta.requiresAdmin && !auth.isAdmin) {
            await ref.read(authProvider.notifier).fetchMe();
            if (!ref.read(authProvider).isAdmin) {
              return '/';
            }
          }

          if (meta.area != ProductArea.core) {
            final areas = ref.read(areasProvider);
            final currentAuth = ref.read(authProvider);
            if (!areas.isEnabled(meta.area)) {
              final encodedFrom = Uri.encodeQueryComponent(fullLocation);
              return '/area-desligada/${meta.area.id}?from=$encodedFrom';
            }
            if (!currentAuth.hasAccess(meta.area)) {
              final encodedFrom = Uri.encodeQueryComponent(fullLocation);
              return '/area-desligada/${meta.area.id}?from=$encodedFrom&motivo=acesso';
            }
          }
        }
        return null;
      }

      // 3. Não autenticado: permite trocar servidor no Android.
      if (location == '/server-config') {
        return null;
      }

      // 4. Instalação pendente (banco sem usuários) -> `/setup`.
      final needsSetup = await ref.read(authProvider.notifier).ensureSetupStatus();
      if (needsSetup) {
        return location == '/setup' ? null : '/setup';
      }

      // 5. Instalação já concluída: `/setup` não tem mais função.
      if (location == '/setup') {
        return '/login';
      }

      // 6. Rota privada sem sessão -> `/login?redirect=...`.
      if (!_publicPaths.contains(location)) {
        final encoded = Uri.encodeQueryComponent(fullLocation);
        return '/login?redirect=$encoded';
      }

      return null;
    },
    routes: <RouteBase>[
      GoRoute(
        path: '/server-config',
        name: 'server-config',
        builder: (context, state) => const ServerConfigPage(),
      ),
      GoRoute(
        path: '/login',
        name: 'login',
        builder: (context, state) => LoginPage(
          redirectTo: state.uri.queryParameters['redirect'],
        ),
      ),
      GoRoute(
        path: '/setup',
        name: 'setup',
        builder: (context, state) => const SetupPage(),
      ),
      ShellRoute(
        builder: (context, state, child) {
          return AdaptiveAppShell(
            location: state.uri.path,
            child: child,
          );
        },
        routes: <RouteBase>[
          ...allAreaRoutes,
          GoRoute(
            path: '/area-desligada/:area',
            name: 'area-off',
            builder: (context, state) => AreaOffPage(
              areaId: state.pathParameters['area'] ?? 'core',
              fromPath: state.uri.queryParameters['from'],
              reason: state.uri.queryParameters['motivo'],
            ),
          ),
        ],
      ),
    ],
  );
});

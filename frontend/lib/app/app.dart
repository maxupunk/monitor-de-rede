import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/app/router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/theme_preference.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aplicação raiz NetMonitor (Material Design 3, pt-BR primário + en fallback).
class NetMonitorApp extends ConsumerStatefulWidget {
  const NetMonitorApp({super.key});

  @override
  ConsumerState<NetMonitorApp> createState() => _NetMonitorAppState();
}

class _NetMonitorAppState extends ConsumerState<NetMonitorApp> {
  bool _bootstrapped = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_bootstrap);
  }

  Future<void> _bootstrap() async {
    await ref.read(serverConfigProvider.notifier).initialize();
    await ref.read(authProvider.notifier).initialize();
    if (mounted) {
      setState(() => _bootstrapped = true);
    }
  }

  @override
  Widget build(BuildContext context) {
    final themeMode = ref.watch(themePreferenceProvider);

    if (!_bootstrapped) {
      return MaterialApp(
        title: 'NetMonitor',
        debugShowCheckedModeBanner: false,
        theme: AppTheme.light(),
        darkTheme: AppTheme.dark(),
        themeMode: themeMode,
        home: const Scaffold(body: Center(child: CircularProgressIndicator())),
      );
    }

    final router = ref.watch(appRouterProvider);

    return MaterialApp.router(
      title: 'NetMonitor',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      themeMode: themeMode,
      routerConfig: router,
      locale: const Locale('pt', 'BR'),
      supportedLocales: const <Locale>[Locale('pt', 'BR'), Locale('en')],
      localizationsDelegates: const <LocalizationsDelegate<dynamic>>[
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
    );
  }
}

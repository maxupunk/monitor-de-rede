import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'pwa_interop_stub.dart'
    if (dart.library.js_interop) 'pwa_interop_web.dart' as interop;

@immutable
class PwaInstallState {
  const PwaInstallState({
    this.canInstall = false,
    this.isInstalled = false,
    this.isIosSafari = false,
  });

  final bool canInstall;
  final bool isInstalled;
  final bool isIosSafari;

  bool get showInstallAction => kIsWeb && !isInstalled && (canInstall || isIosSafari);
}

class PwaInstallNotifier extends Notifier<PwaInstallState> {
  @override
  PwaInstallState build() {
    if (!kIsWeb) {
      return const PwaInstallState(canInstall: false, isInstalled: true);
    }
    interop.subscribePwaChanges(_refresh);
    return _readState();
  }

  PwaInstallState _readState() {
    return PwaInstallState(
      canInstall: interop.canInstallPwa(),
      isInstalled: interop.isPwaInstalled(),
      isIosSafari: interop.isIosSafari(),
    );
  }

  void _refresh() {
    state = _readState();
  }

  Future<bool> promptInstall() async {
    if (!kIsWeb) return false;
    final accepted = await interop.promptPwaInstall();
    _refresh();
    return accepted;
  }
}

final pwaInstallProvider =
    NotifierProvider<PwaInstallNotifier, PwaInstallState>(
  PwaInstallNotifier.new,
);

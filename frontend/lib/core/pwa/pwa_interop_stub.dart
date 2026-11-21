import 'dart:ui';

bool canInstallPwa() => false;
bool isPwaInstalled() => true;
bool isIosSafari() => false;
Future<bool> promptPwaInstall() async => false;
void subscribePwaChanges(VoidCallback onChanged) {}

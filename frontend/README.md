# NetMonitor Frontend (Flutter — Web PWA & Android)

Interface oficial do **NetMonitor** construída em **Flutter** (Material Design 3, mobile-first, Riverpod + GoRouter, SSE em tempo real sem polling), com suporte nativo para **Web PWA** e **Android**.

## Arquitetura

- `lib/core/`: configuração de servidor (same-origin na Web, URL configurável com teste de conexão no Android), `ApiClient` (REST, raw binary POST, multipart, NDJSON, SSE-over-POST), `SseClient` + `SseService` (`GET /api/events/stream` + streams dedicados + interesses sob demanda), autenticação + controle de acesso por área (ADR 019), navegação e formatadores canônicos (`formatters.dart`).
- `lib/areas/`: 4 áreas modulares do produto (ADR 017) — `core`, `network`, `servers` e `vpn` — mais registros declarativos de composição (`DashboardWidgetRegistry`, `SettingsSectionRegistry`, `BackupTabRegistry`, `DeviceDetailTabRegistry`, `ServerConnectSlot`).
- `lib/theme/app_theme.dart`: temas Claro e Escuro em Material Design 3 com alto contraste e tokens semânticos (`success`, `warning`, `info`, `error`, `caution`, `plugin` em teal `#00796B`/`#1DE9B6`, `ai` em roxo).
- `lib/shared/widgets/`: kit de componentes adaptativos (`AdaptiveAppShell`, `SectionCard`, `NestedPanel`, `ChoiceCard`, `SemanticChip`, `KpiCard`, `showAdaptiveFormSheet`, `MetricLineChart`, etc.).
- `lib/features/`: telas e providers organizados por domínio (`auth`, `core`, `network`, `servers`, `vpn`, `ai`, `plugins`).

## Pré-requisitos

- Flutter SDK (canal `stable`, Dart 3.13+)
- Android SDK (para compilar o APK nativo)

## Comandos principais

Execute a partir de `frontend/`:

```bash
# Instalar dependências
flutter pub get

# Executar localmente na Web (Chrome)
flutter run -d chrome

# Executar localmente em dispositivo/emulador Android
flutter run -d android

# Formatação e verificação de estilo
dart format lib test
dart format --output=none --set-exit-if-changed lib test

# Análise estática (deve passar com 0 issues)
flutter analyze

# Suíte de testes unitários e de widgets
flutter test

# Build Web PWA (produção, self-hosted sem CDN externa)
flutter build web --release --no-web-resources-cdn

# Build Android APK (debug ou release)
flutter build apk --debug
flutter build apk --release
```

Os artefatos compilados ficam em `build/web/` (Web PWA, empacotado automaticamente pelo `Dockerfile` na etapa `web` para `/app/web`) e `build/app/outputs/flutter-apk/` (Android APK).

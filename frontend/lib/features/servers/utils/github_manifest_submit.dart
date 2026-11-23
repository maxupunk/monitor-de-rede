import 'github_manifest_submit_stub.dart'
    if (dart.library.js_interop) 'github_manifest_submit_web.dart';

/// Submete o manifest do GitHub App via `<form method="POST">` na Web
/// (conforme exigido pelo fluxo GitHub App Manifest). Retorna `true` se o
/// formulário foi submetido nativamente, ou `false` em plataformas não-Web.
bool submitGithubManifestForm({
  required String actionUrl,
  required String manifestJson,
}) => platformSubmitGithubManifestForm(
  actionUrl: actionUrl,
  manifestJson: manifestJson,
);

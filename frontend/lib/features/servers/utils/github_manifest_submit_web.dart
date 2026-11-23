import 'package:web/web.dart' as web;

bool platformSubmitGithubManifestForm({
  required String actionUrl,
  required String manifestJson,
}) {
  final body = web.document.body;
  if (body == null) return false;
  final form = web.document.createElement('form') as web.HTMLFormElement
    ..method = 'POST'
    ..action = actionUrl
    ..style.display = 'none';
  final input = web.document.createElement('input') as web.HTMLInputElement
    ..type = 'hidden'
    ..name = 'manifest'
    ..value = manifestJson;
  form.appendChild(input);
  body.appendChild(form);
  form.submit();
  form.remove();
  return true;
}

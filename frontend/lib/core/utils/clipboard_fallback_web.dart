import 'package:web/web.dart' as web;

/// Fallback de cópia para origens HTTP não seguras (LAN/VPN) na Web onde
/// `navigator.clipboard` é indisponível e `Clipboard.setData` falha.
bool platformCopyToClipboardFallback(String text) {
  web.HTMLTextAreaElement? textarea;
  try {
    final body = web.document.body;
    if (body == null) return false;
    textarea = web.document.createElement('textarea') as web.HTMLTextAreaElement
      ..value = text
      ..readOnly = true
      ..setAttribute('aria-hidden', 'true');
    textarea.style
      ..position = 'fixed'
      ..top = '0'
      ..left = '-9999px'
      ..opacity = '0';
    body.appendChild(textarea);
    textarea.focus();
    textarea.select();
    textarea.setSelectionRange(0, text.length);
    return web.document.execCommand('copy');
  } catch (_) {
    return false;
  } finally {
    textarea?.remove();
  }
}

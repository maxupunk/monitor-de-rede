import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/network_address_utils.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_snippets.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Um caminho até o MCP desta central, como o agente externo vai usá-lo.
@immutable
class McpEndpointOption {
  const McpEndpointOption({
    required this.id,
    required this.label,
    required this.endpointUrl,
    required this.internal,
  });

  final String id;
  final String label;
  final String endpointUrl;

  /// Rede local ou VPN: o MCP aceita sem liberar conexões externas.
  final bool internal;

  bool get https => endpointUrl.startsWith('https://');
}

const String _currentOriginId = 'current';

/// Os caminhos até o MCP: os "Endereços deste servidor" (com a URL e a porta
/// certas, a mesma regra dos comandos do agente) e, por último, o endereço
/// pelo qual esta tela foi aberta — que pode ser o domínio público.
///
/// Rede local primeiro: o MCP recusa origem pública por padrão, então o
/// endereço que funciona sem mexer em nada vem na frente.
List<McpEndpointOption> mcpEndpointOptions({
  required List<ServerAddressEntry> addresses,
  required String currentOrigin,
  required String endpointPath,
}) {
  const order = <String, int>{'lan': 0, 'vpn': 1, 'custom': 2};
  final sorted = addresses.where((a) => (a.url ?? '').isNotEmpty).toList()
    ..sort((a, b) => (order[a.kind] ?? 3).compareTo(order[b.kind] ?? 3));
  final options = <McpEndpointOption>[];
  final seen = <String>{};
  void add(String id, String label, String origin, {bool? internal}) {
    final url = buildMcpEndpointUrl(origin: origin, endpointPath: endpointPath);
    if (!seen.add(url)) return;
    options.add(
      McpEndpointOption(
        id: id,
        label: label,
        endpointUrl: url,
        internal: internal ?? looksPrivateUrl(origin),
      ),
    );
  }

  for (final address in sorted) {
    add(
      address.id,
      address.label,
      address.url!,
      internal: address.kind == 'lan' || address.kind == 'vpn' ? true : null,
    );
  }
  if (currentOrigin.trim().isNotEmpty) {
    add(_currentOriginId, 'Endereço desta tela', currentOrigin);
  }
  return List<McpEndpointOption>.unmodifiable(options);
}

/// O caminho escolhido, ou o primeiro que funciona com a política atual.
McpEndpointOption? pickMcpEndpoint(
  List<McpEndpointOption> options, {
  String? selectedId,
  required bool allowPublicOrigin,
}) {
  for (final option in options) {
    if (option.id == selectedId) return option;
  }
  for (final option in options) {
    if (option.internal || allowPublicOrigin) return option;
  }
  return options.isEmpty ? null : options.first;
}

/// Escolha do endereço que vai nas configurações dos agentes, com o que cada
/// caminho exige. É aqui que fica claro que o domínio (Cloudflare, proxy) é
/// **externo**: mesmo de dentro da rede, a requisição sai e volta pela
/// internet, e o MCP a trata como de fora.
class McpEndpointPicker extends StatelessWidget {
  const McpEndpointPicker({
    super.key,
    required this.options,
    required this.selected,
    required this.allowPublicOrigin,
    required this.onSelected,
  });

  final List<McpEndpointOption> options;
  final McpEndpointOption? selected;
  final bool allowPublicOrigin;
  final ValueChanged<McpEndpointOption> onSelected;

  @override
  Widget build(BuildContext context) {
    final current = selected;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          'Endereço que vai na configuração do agente',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            for (final option in options)
              ChoiceChip(
                selected: option.id == current?.id,
                onSelected: (_) => onSelected(option),
                avatar: Icon(
                  option.internal ? Icons.lan_outlined : Icons.public,
                  size: 16,
                ),
                label: Text(option.label),
              ),
          ],
        ),
        if (current != null) ...<Widget>[
          const SizedBox(height: 8),
          SelectableText(
            current.endpointUrl,
            style: context.textTheme.bodyMedium?.copyWith(
              fontFamily: 'monospace',
            ),
          ),
          const SizedBox(height: 8),
          _PathNote(option: current, allowPublicOrigin: allowPublicOrigin),
        ],
      ],
    );
  }
}

class _PathNote extends StatelessWidget {
  const _PathNote({required this.option, required this.allowPublicOrigin});

  final McpEndpointOption option;
  final bool allowPublicOrigin;

  @override
  Widget build(BuildContext context) {
    final (SemanticTone tone, String text) = switch (option) {
      McpEndpointOption(internal: true) => (
        SemanticTone.info,
        'Rede interna ou VPN: o agente precisa estar na mesma rede do servidor '
            'ou conectado à VPN do NetMonitor. Funciona sem liberar conexões '
            'externas.',
      ),
      _ when !allowPublicOrigin => (
        SemanticTone.warning,
        'Este endereço passa pela internet (domínio, Cloudflare, proxy): mesmo '
            'de dentro da sua rede, a requisição sai e volta por fora, e o MCP '
            'a recusa enquanto "Permitir conexões fora da rede interna/VPN" '
            'estiver desligado. Use o endereço da rede local ou da VPN, ou '
            'ative a opção abaixo.',
      ),
      McpEndpointOption(https: false) => (
        SemanticTone.error,
        'Conexão pela internet sem HTTPS: o token viajaria às claras. Use um '
            'endereço HTTPS (o domínio atrás do proxy) ou a VPN.',
      ),
      _ => (
        SemanticTone.success,
        'Pela internet, com HTTPS: liberado nas configurações abaixo. O token '
            'é a única barreira — dê validade curta e revogue o que não usar.',
      ),
    };
    return NestedPanel(tone: tone, child: Text(text));
  }
}

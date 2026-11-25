import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/mcp_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Monta a URL completa do endpoint Streamable HTTP MCP a partir da origem da central.
String buildMcpEndpointUrl({
  required String origin,
  String endpointPath = '/mcp',
}) {
  final cleanPath = endpointPath.startsWith('/')
      ? endpointPath
      : '/$endpointPath';
  var cleanOrigin = origin.trim();
  while (cleanOrigin.endsWith('/')) {
    cleanOrigin = cleanOrigin.substring(0, cleanOrigin.length - 1);
  }
  if (cleanOrigin.isEmpty) {
    return 'https://seu-netmonitor.local$cleanPath';
  }
  return '$cleanOrigin$cleanPath';
}

/// Bloco `mcpServers` (`~/.cursor/mcp.json` e clientes do mesmo formato).
String buildMcpJsonSnippet({
  required String endpointUrl,
  required String token,
}) {
  return const JsonEncoder.withIndent('  ').convert(<String, dynamic>{
    'mcpServers': <String, dynamic>{
      'netmonitor': <String, dynamic>{
        'url': endpointUrl,
        'headers': <String, String>{'Authorization': 'Bearer $token'},
      },
    },
  });
}

/// Bloco do Antigravity (`mcp_config.json`): servidor remoto em `serverUrl`.
String buildAntigravityMcpSnippet({
  required String endpointUrl,
  required String token,
}) {
  return const JsonEncoder.withIndent('  ').convert(<String, dynamic>{
    'mcpServers': <String, dynamic>{
      'netmonitor': <String, dynamic>{
        'serverUrl': endpointUrl,
        'headers': <String, String>{'Authorization': 'Bearer $token'},
      },
    },
  });
}

/// Bloco do VS Code (`.vscode/mcp.json`): chave `servers` e `type: http`.
String buildVsCodeMcpSnippet({
  required String endpointUrl,
  required String token,
}) {
  return const JsonEncoder.withIndent('  ').convert(<String, dynamic>{
    'servers': <String, dynamic>{
      'netmonitor': <String, dynamic>{
        'type': 'http',
        'url': endpointUrl,
        'headers': <String, String>{'Authorization': 'Bearer $token'},
      },
    },
  });
}

/// Gera o comando pronto para conectar o servidor MCP pelo CLI do Claude Code.
String buildClaudeCodeCliSnippet({
  required String endpointUrl,
  required String token,
}) {
  return 'claude mcp add --transport http netmonitor $endpointUrl '
      '--header "Authorization: Bearer $token"';
}

/// Os clientes com configuração pronta: cada um sabe o próprio formato.
enum McpClient {
  antigravity(
    'Antigravity',
    'Antigravity (Agente › MCP Servers › Manage › View raw config)',
  ),
  cursor('Cursor', 'Cursor (~/.cursor/mcp.json)'),
  vscode('VS Code', 'VS Code (.vscode/mcp.json)'),
  claudeCode('Claude Code', 'Claude Code (terminal)');

  const McpClient(this.label, this.title);

  final String label;
  final String title;

  String snippet({required String endpointUrl, required String token}) {
    return switch (this) {
      McpClient.antigravity => buildAntigravityMcpSnippet(
        endpointUrl: endpointUrl,
        token: token,
      ),
      McpClient.cursor => buildMcpJsonSnippet(
        endpointUrl: endpointUrl,
        token: token,
      ),
      McpClient.vscode => buildVsCodeMcpSnippet(
        endpointUrl: endpointUrl,
        token: token,
      ),
      McpClient.claudeCode => buildClaudeCodeCliSnippet(
        endpointUrl: endpointUrl,
        token: token,
      ),
    };
  }
}

/// Abre o guia rápido com snippets prontos para conectar agentes externos ao servidor MCP.
///
/// `endpointUrl` é o endereço escolhido no cartão; `internal` diz se ele é da
/// rede local ou da VPN.
Future<void> showMcpHelpSheet(
  BuildContext context, {
  required String endpointUrl,
  required bool internal,
  required McpSettings settings,
  required int toolsCount,
}) {
  const placeholderToken = '<SEU_TOKEN_MCP>';

  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Como conectar agentes ao MCP',
    subtitle: endpointUrl,
    maxWidth: 680,
    builder: (_) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        if (!settings.enabled) ...<Widget>[
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'O servidor MCP está desativado no momento. Ative-o no painel de '
              'configurações para aceitar conexões de agentes externos.',
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (!internal && !settings.allowPublicOrigin) ...<Widget>[
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Atenção: o endereço escolhido passa pela internet, mas a opção '
              '"Permitir conexões fora da rede interna/VPN" está desativada. '
              'Escolha no cartão o endereço da rede local ou da VPN, ou '
              'libere conexões externas nas configurações do servidor MCP.',
            ),
          ),
          const SizedBox(height: 12),
        ],
        NestedPanel(
          tone: SemanticTone.info,
          child: Text(
            'O NetMonitor expõe ${countNoun(toolsCount, 'ferramenta', 'ferramentas')} via '
            'MCP (Streamable HTTP) diretamente em $endpointUrl, sem exigir que o '
            'modelo de IA local esteja ativado. Feche este guia, crie um token em '
            '"Novo token" e substitua $placeholderToken nos exemplos — ao criar, '
            'a configuração já vem pronta com o token.',
          ),
        ),
        for (final client in McpClient.values) ...<Widget>[
          const SizedBox(height: 12),
          CodeViewer(
            title: client.title,
            code: client.snippet(
              endpointUrl: endpointUrl,
              token: placeholderToken,
            ),
          ),
        ],
      ],
    ),
  );
}

/// Banner de exibição única para o segredo do token MCP recém-criado.
class McpCreatedTokenBanner extends StatefulWidget {
  const McpCreatedTokenBanner({
    super.key,
    required this.created,
    required this.endpointUrl,
    required this.onDismiss,
  });

  final McpTokenCreated created;
  final String endpointUrl;
  final VoidCallback onDismiss;

  @override
  State<McpCreatedTokenBanner> createState() => _McpCreatedTokenBannerState();
}

class _McpCreatedTokenBannerState extends State<McpCreatedTokenBanner> {
  /// `null` mostra o token bruto.
  McpClient? _client = McpClient.values.first;

  /// O segredo só aparece uma vez: fechar sem ter copiado pede confirmação.
  Future<void> _dismiss() async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Já copiou o token?',
      message:
          'Ele não poderá ser exibido de novo. Se não copiou, será preciso '
          'revogar este e criar outro.',
      confirmLabel: 'Já copiei, fechar',
      cancelLabel: 'Voltar',
      tone: SemanticTone.warning,
    );
    if (confirmed && mounted) widget.onDismiss();
  }

  @override
  Widget build(BuildContext context) {
    final secret = widget.created.secret;
    final client = _client;
    final (String title, String code) = client == null
        ? ('Token bruto', secret)
        : (
            client.title,
            client.snippet(endpointUrl: widget.endpointUrl, token: secret),
          );

    return NestedPanel(
      tone: SemanticTone.success,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      'Token "${widget.created.token.name}" criado — copie agora, ele não será exibido novamente',
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      'Cole a configuração pronta no seu agente ou guarde o token bruto em um cofre seguro.',
                      style: context.textTheme.bodySmall,
                    ),
                  ],
                ),
              ),
              IconButton(
                tooltip: 'Fechar aviso',
                icon: const Icon(Icons.close, size: 18),
                onPressed: _dismiss,
              ),
            ],
          ),
          const SizedBox(height: 10),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: SegmentedButton<McpClient?>(
              showSelectedIcon: false,
              style: SegmentedButton.styleFrom(
                visualDensity: VisualDensity.compact,
                padding: const EdgeInsets.symmetric(horizontal: 10),
              ),
              selected: <McpClient?>{client},
              onSelectionChanged: (selection) {
                if (selection.isEmpty) return;
                setState(() => _client = selection.first);
              },
              segments: <ButtonSegment<McpClient?>>[
                for (final item in McpClient.values)
                  ButtonSegment<McpClient?>(
                    value: item,
                    label: Text(item.label),
                  ),
                const ButtonSegment<McpClient?>(
                  value: null,
                  label: Text('Token bruto'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 10),
          CodeViewer(title: title, code: code),
          const SizedBox(height: 10),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.tonalIcon(
              onPressed: widget.onDismiss,
              icon: const Icon(Icons.check, size: 18),
              label: const Text('Entendi, já copiei'),
            ),
          ),
        ],
      ),
    );
  }
}

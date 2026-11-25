import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/network_address_utils.dart';
import 'package:netmonitor_app/features/ai/models/mcp_models.dart';
import 'package:netmonitor_app/features/ai/providers/mcp_provider.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_create_token_sheet.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_endpoint_picker.dart';
import 'package:netmonitor_app/features/ai/widgets/mcp_snippets.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Cartão de configuração do Servidor MCP (agentes externos) na aba `ia` de `/settings`.
///
/// Permanece sempre visível mesmo quando o assistente de IA local (`AiSettings.enabled`)
/// está desativado, pois agentes externos (Antigravity, Cursor, Claude Code, VS Code)
/// conectam-se diretamente às ferramentas da central via Streamable HTTP.
class McpSettingsCard extends ConsumerStatefulWidget {
  const McpSettingsCard({super.key});

  @override
  ConsumerState<McpSettingsCard> createState() => _McpSettingsCardState();
}

class _McpSettingsCardState extends ConsumerState<McpSettingsCard> {
  /// O endereço escolhido para as configurações dos agentes; nulo segue o
  /// primeiro que funciona com a política atual.
  String? _endpointId;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        ref.read(mcpProvider.notifier).loadAll();
        ref.read(coreSettingsProvider.notifier).fetchServerAddresses();
      }
    });
  }

  Future<void> _updateSettings({
    required bool enabled,
    required bool allowPublicOrigin,
  }) async {
    final ok = await ref
        .read(mcpProvider.notifier)
        .updateSettings(enabled: enabled, allowPublicOrigin: allowPublicOrigin);
    if (!mounted) return;
    if (ok) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Configurações do servidor MCP atualizadas.'),
        ),
      );
    }
  }

  Future<void> _revokeToken(McpTokenView token) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Revogar token "${token.name}"?',
      message:
          'Qualquer agente externo usando este token perderá o acesso imediatamente.',
      confirmLabel: 'Revogar',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    try {
      await ref.read(mcpProvider.notifier).revokeToken(token.id);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Token "${token.name}" revogado.')),
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(describeError(err))));
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(mcpProvider);
    final serverConfig = ref.watch(serverConfigProvider);
    final overview = state.overview ?? const McpOverview();
    final settings = overview.settings;
    final origin = resolveCentralOrigin(serverConfig.baseUrl);
    final endpointOptions = mcpEndpointOptions(
      addresses: ref.watch(coreSettingsProvider).addresses,
      currentOrigin: origin,
      endpointPath: settings.endpointPath,
    );
    final endpoint = pickMcpEndpoint(
      endpointOptions,
      selectedId: _endpointId,
      allowPublicOrigin: settings.allowPublicOrigin,
    );
    final endpointUrl =
        endpoint?.endpointUrl ??
        buildMcpEndpointUrl(
          origin: origin,
          endpointPath: settings.endpointPath,
        );

    return SectionCard(
      title: 'Servidor MCP (Agentes externos)',
      subtitle:
          'Conecte IDEs e agentes externos (Antigravity, Cursor, Claude Code, VS Code) '
          'direto ao NetMonitor, sem precisar do modelo de IA local.',
      icon: Icons.hub_outlined,
      iconTone: SemanticTone.ai,
      trailing: Wrap(
        spacing: 8,
        runSpacing: 6,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: <Widget>[
          // Sem a visão geral carregada, o padrão diria "Ativo" sem saber.
          if (state.loaded) ...<Widget>[
            SemanticChip(
              label: settings.enabled ? 'Ativo' : 'Desativado',
              tone: settings.enabled
                  ? SemanticTone.success
                  : SemanticTone.warning,
              compact: true,
            ),
            SemanticChip(
              label: settings.endpointPath,
              tone: SemanticTone.info,
              compact: true,
            ),
          ],
          OutlinedButton.icon(
            onPressed: () => showMcpHelpSheet(
              context,
              endpointUrl: endpointUrl,
              internal: endpoint?.internal ?? true,
              settings: settings,
              toolsCount: overview.toolsCount,
            ),
            icon: const Icon(Icons.help_outline, size: 16),
            label: const Text('Como configurar'),
          ),
        ],
      ),
      child: state.loading && !state.loaded
          ? const LoadingStateView(message: 'Carregando servidor MCP...')
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (state.error != null) ...<Widget>[
                  NestedPanel(
                    tone: SemanticTone.error,
                    child: Text(state.error!),
                  ),
                  const SizedBox(height: 12),
                ],
                Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  children: <Widget>[
                    SemanticChip(
                      label: countNoun(
                        overview.toolsCount,
                        'ferramenta disponível',
                        'ferramentas disponíveis',
                      ),
                      tone: SemanticTone.ai,
                      icon: Icons.build_outlined,
                      compact: true,
                    ),
                    if (settings.allowPublicOrigin)
                      const SemanticChip(
                        label: 'Acesso externo liberado',
                        tone: SemanticTone.warning,
                        icon: Icons.public_outlined,
                        compact: true,
                      )
                    else
                      const SemanticChip(
                        label: 'Apenas rede interna / VPN',
                        tone: SemanticTone.info,
                        icon: Icons.shield_outlined,
                        compact: true,
                      ),
                  ],
                ),
                if (endpointOptions.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 12),
                  McpEndpointPicker(
                    options: endpointOptions,
                    selected: endpoint,
                    allowPublicOrigin: settings.allowPublicOrigin,
                    onSelected: (option) =>
                        setState(() => _endpointId = option.id),
                  ),
                ],
                if (overview.canManageSettings) ...<Widget>[
                  const SizedBox(height: 12),
                  NestedPanel(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 14,
                      vertical: 8,
                    ),
                    child: Material(
                      type: MaterialType.transparency,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          SwitchListTile(
                            contentPadding: EdgeInsets.zero,
                            title: const Text('Ativar servidor MCP'),
                            subtitle: const Text(
                              'Habilita o endpoint Streamable HTTP para agentes externos autenticados por token.',
                            ),
                            value: settings.enabled,
                            onChanged: state.savingSettings
                                ? null
                                : (value) => _updateSettings(
                                    enabled: value,
                                    allowPublicOrigin:
                                        settings.allowPublicOrigin,
                                  ),
                          ),
                          const Divider(height: 12),
                          SwitchListTile(
                            contentPadding: EdgeInsets.zero,
                            title: Wrap(
                              spacing: 8,
                              runSpacing: 4,
                              crossAxisAlignment: WrapCrossAlignment.center,
                              children: <Widget>[
                                const Text(
                                  'Permitir conexões fora da rede interna/VPN',
                                ),
                                if (settings.allowPublicOrigin)
                                  const SemanticChip(
                                    label: 'Borda pública ativa',
                                    tone: SemanticTone.warning,
                                    compact: true,
                                  ),
                              ],
                            ),
                            subtitle: const Text(
                              'Segurança: mantenha desligado quando os agentes estiverem na LAN ou VPN. '
                              'Ative somente se precisar conectar pela internet (use sempre HTTPS).',
                            ),
                            value: settings.allowPublicOrigin,
                            onChanged: state.savingSettings
                                ? null
                                : (value) => _updateSettings(
                                    enabled: settings.enabled,
                                    allowPublicOrigin: value,
                                  ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
                if (state.justCreatedToken != null) ...<Widget>[
                  const SizedBox(height: 14),
                  McpCreatedTokenBanner(
                    created: state.justCreatedToken!,
                    endpointUrl: endpointUrl,
                    onDismiss: () =>
                        ref.read(mcpProvider.notifier).clearJustCreatedToken(),
                  ),
                ],
                const Divider(height: 28),
                Wrap(
                  spacing: 10,
                  runSpacing: 8,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          'Tokens MCP',
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        Text(
                          'Crie credenciais com escopos mínimos para cada projeto ou agente.',
                          style: context.textTheme.bodySmall?.copyWith(
                            color: context.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                    FilledButton.icon(
                      onPressed: overview.scopes.isEmpty && !state.loaded
                          ? null
                          : () => showCreateMcpTokenSheet(
                              context,
                              overview: overview,
                            ),
                      icon: const Icon(Icons.add, size: 18),
                      label: const Text('Novo token'),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                if (state.loadingTokens && !state.tokensLoaded)
                  const LoadingStateView(message: 'Carregando tokens MCP...')
                else if (state.tokens.isEmpty)
                  const EmptyStateView(
                    title: 'Nenhum token MCP criado',
                    description:
                        'Crie um token com escopos dedicados (ex.: Deploy de projeto) '
                        'para conectar seu agente externo ao NetMonitor.',
                    icon: Icons.vpn_key_outlined,
                    tone: SemanticTone.ai,
                  )
                else
                  ...state.tokens.map(
                    (token) => Padding(
                      padding: const EdgeInsets.only(bottom: 10),
                      child: _McpTokenTile(
                        token: token,
                        overview: overview,
                        onRevoke: () => _revokeToken(token),
                      ),
                    ),
                  ),
              ],
            ),
    );
  }
}

class _McpTokenTile extends StatelessWidget {
  const _McpTokenTile({
    required this.token,
    required this.overview,
    required this.onRevoke,
  });

  final McpTokenView token;
  final McpOverview overview;
  final VoidCallback onRevoke;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final restrictedApps = token.allowedApplicationIds;
    final lastUseBase = formatRelativeTime(token.lastUsedAt);
    final lastUseLabel = token.lastUsedIp != null
        ? '$lastUseBase (${token.lastUsedIp})'
        : lastUseBase;

    return NestedPanel(
      tone: token.isActive ? null : SemanticTone.secondary,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Wrap(
                  spacing: 6,
                  runSpacing: 4,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Text(
                      token.name,
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (token.hint.isNotEmpty)
                      SemanticChip(
                        label: token.hint,
                        tone: SemanticTone.secondary,
                        compact: true,
                      ),
                    if (token.revoked)
                      const SemanticChip(
                        label: 'Revogado',
                        tone: SemanticTone.error,
                        compact: true,
                      )
                    else if (token.expired)
                      const SemanticChip(
                        label: 'Vencido',
                        tone: SemanticTone.warning,
                        compact: true,
                      )
                    else
                      const SemanticChip(
                        label: 'Ativo',
                        tone: SemanticTone.success,
                        compact: true,
                      ),
                  ],
                ),
                if (token.scopes.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 6,
                    runSpacing: 6,
                    children: token.scopes
                        .map((scopeId) {
                          final desc = overview.scopeById(scopeId);
                          return SemanticChip(
                            label: desc?.label ?? scopeId,
                            tone: SemanticTone.ai,
                            compact: true,
                          );
                        })
                        .toList(growable: false),
                  ),
                ],
                if (restrictedApps != null &&
                    restrictedApps.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 6),
                  Wrap(
                    spacing: 6,
                    runSpacing: 6,
                    children: restrictedApps
                        .map((appId) {
                          final app = overview.applicationById(appId);
                          return SemanticChip(
                            label: app != null && app.name.isNotEmpty
                                ? app.name
                                : 'App #$appId',
                            tone: SemanticTone.info,
                            icon: Icons.rocket_launch_outlined,
                            compact: true,
                          );
                        })
                        .toList(growable: false),
                  ),
                ],
                const SizedBox(height: 8),
                Text(
                  'Último uso: $lastUseLabel · '
                  'Validade: ${formatDate(token.expiresAt)}'
                  '${token.owner.isNotEmpty ? ' · ${token.owner}' : ''}',
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
          if (!token.revoked)
            IconButton(
              tooltip: 'Revogar token',
              color: scheme.error,
              icon: const Icon(Icons.delete_outline),
              onPressed: onRevoke,
            ),
        ],
      ),
    );
  }
}

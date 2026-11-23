import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Ajuda rápida de `docker login` / `daemon.json` para o Registry interno (ADR 023).
Future<void> showRegistryDockerHelpSheet(
  BuildContext context, {
  required String origin,
}) {
  final host = registryHostFromUrl(origin);
  final isSecure = origin.startsWith('https://');
  final isInternal = looksPrivateUrl(origin);

  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Usar o Registry pelo Docker',
    subtitle: host,
    maxWidth: 680,
    builder: (_) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (!isInternal) ...<Widget>[
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Atenção (ADR 023): tokens pessoais de acesso são recusados pela borda pública. '
              'Conecte-se pela VPN do NetMonitor ou pela rede interna para usar docker login.',
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (!isSecure) ...<Widget>[
          CodeViewer(
            title: '/etc/docker/daemon.json (HTTP interno / VPN)',
            code: registryDaemonJson(host),
          ),
          const SizedBox(height: 12),
        ],
        CodeViewer(
          title: '1. Autenticar (cole o token como senha)',
          code: 'docker login $host -u token',
        ),
        const SizedBox(height: 12),
        CodeViewer(
          title: '2. Enviar ou baixar imagem',
          code:
              'docker tag minha-imagem $host/app/servico:v1\n'
              'docker push $host/app/servico:v1\n'
              'docker pull $host/app/servico:v1',
        ),
      ],
    ),
  );
}

/// Modal seguro para criação de token pessoal do Registry.
Future<RegistryTokenCreated?> showCreateRegistryTokenSheet(
  BuildContext context,
) {
  return showAdaptiveFormSheet<RegistryTokenCreated>(
    context: context,
    title: 'Novo token de acesso ao Registry',
    subtitle: 'Válido exclusivamente a partir da VPN ou rede interna (ADR 023)',
    builder: (_) => const _CreateRegistryTokenForm(),
  );
}

class _CreateRegistryTokenForm extends ConsumerStatefulWidget {
  const _CreateRegistryTokenForm();

  @override
  ConsumerState<_CreateRegistryTokenForm> createState() =>
      _CreateRegistryTokenFormState();
}

class _CreateRegistryTokenFormState
    extends ConsumerState<_CreateRegistryTokenForm> {
  final TextEditingController _nameCtrl = TextEditingController();
  final TextEditingController _reposCtrl = TextEditingController(text: '*');
  final Set<String> _actions = <String>{'pull'};
  int _days = 90;
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _nameCtrl.dispose();
    _reposCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final name = _nameCtrl.text.trim();
    if (name.isEmpty || _submitting) return;
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final repos = _reposCtrl.text
          .split(',')
          .map((e) => e.trim())
          .where((e) => e.isNotEmpty)
          .toList(growable: false);
      final res = await ref
          .read(registryProvider.notifier)
          .createToken(
            name: name,
            repositories: repos.isEmpty ? const <String>['*'] : repos,
            actions: _actions.toList(growable: false),
            days: _days,
          );
      if (!mounted) return;
      Navigator.of(context).pop(res);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = describeError(err, fallback: 'Erro ao criar token.');
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        TextField(
          controller: _nameCtrl,
          autofocus: true,
          decoration: const InputDecoration(
            labelText: 'Nome do token',
            hintText: 'ex.: ci-local',
          ),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _reposCtrl,
          decoration: const InputDecoration(
            labelText: 'Repositórios (separados por vírgula)',
            hintText: '* ou app/* ou app/api',
          ),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 8,
          children: const <String>['pull', 'push', 'delete']
              .map((act) {
                return FilterChip(
                  selected: _actions.contains(act),
                  label: Text(
                    '${act.toUpperCase()} (${registryActionLabel(act)})',
                  ),
                  onSelected: (checked) => setState(() {
                    if (checked) {
                      _actions.add(act);
                    } else if (_actions.length > 1) {
                      _actions.remove(act);
                    }
                  }),
                );
              })
              .toList(growable: false),
        ),
        const SizedBox(height: 10),
        AppDropdownFormField<int>(
          initialValue: _days,
          decoration: const InputDecoration(labelText: 'Validade'),
          items: const <DropdownMenuItem<int>>[
            DropdownMenuItem(value: 30, child: Text('30 dias')),
            DropdownMenuItem(value: 90, child: Text('90 dias')),
            DropdownMenuItem(value: 180, child: Text('180 dias')),
            DropdownMenuItem(value: 365, child: Text('365 dias')),
          ],
          onChanged: (v) {
            if (v != null) setState(() => _days = v);
          },
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton(
            onPressed: _submitting ? null : _submit,
            child: const Text('Criar token'),
          ),
        ),
      ],
    );
  }
}

/// Modal seguro para registro de exceção de vulnerabilidade OSV/CVE.
Future<bool?> showCreateRegistryExceptionSheet(BuildContext context) {
  return showAdaptiveFormSheet<bool>(
    context: context,
    title: 'Nova exceção de vulnerabilidade',
    builder: (_) => const _CreateRegistryExceptionForm(),
  );
}

class _CreateRegistryExceptionForm extends ConsumerStatefulWidget {
  const _CreateRegistryExceptionForm();

  @override
  ConsumerState<_CreateRegistryExceptionForm> createState() =>
      _CreateRegistryExceptionFormState();
}

class _CreateRegistryExceptionFormState
    extends ConsumerState<_CreateRegistryExceptionForm> {
  final TextEditingController _advisoryCtrl = TextEditingController();
  final TextEditingController _reasonCtrl = TextEditingController();
  int _days = 30;
  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _advisoryCtrl.dispose();
    _reasonCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final advisory = _advisoryCtrl.text.trim();
    final reason = _reasonCtrl.text.trim();
    if (advisory.isEmpty || reason.length < 10 || _submitting) {
      setState(
        () => _error =
            'Informe o identificador OSV/CVE e justificativa com mín. 10 caracteres.',
      );
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      await ref
          .read(registryProvider.notifier)
          .addException(advisory: advisory, reason: reason, days: _days);
      if (!mounted) return;
      Navigator.of(context).pop(true);
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = describeError(err, fallback: 'Erro ao registrar exceção.');
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        TextField(
          controller: _advisoryCtrl,
          decoration: const InputDecoration(
            labelText: 'Identificador OSV / CVE',
            hintText: 'ex.: GHSA-xxxx-xxxx-xxxx ou CVE-2026-1234',
          ),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _reasonCtrl,
          maxLines: 3,
          decoration: const InputDecoration(
            labelText: 'Justificativa (mín. 10 caracteres)',
          ),
        ),
        const SizedBox(height: 10),
        AppDropdownFormField<int>(
          initialValue: _days,
          decoration: const InputDecoration(labelText: 'Prazo'),
          items: const <DropdownMenuItem<int>>[
            DropdownMenuItem(value: 7, child: Text('7 dias')),
            DropdownMenuItem(value: 30, child: Text('30 dias')),
            DropdownMenuItem(value: 90, child: Text('90 dias')),
            DropdownMenuItem(value: 180, child: Text('180 dias')),
          ],
          onChanged: (v) {
            if (v != null) setState(() => _days = v);
          },
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton(
            onPressed: _submitting ? null : _submit,
            child: const Text('Registrar exceção'),
          ),
        ),
      ],
    );
  }
}

/// Aba "Tokens" do Registry — consome estado diretamente de `registryProvider`.
class RegistryTokensTab extends ConsumerWidget {
  const RegistryTokensTab({super.key, required this.origin});

  final String origin;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final reg = ref.watch(registryProvider);
    final host = registryHostFromUrl(origin);
    final justCreated = reg.justCreatedToken;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const NestedPanel(
          tone: SemanticTone.info,
          child: Text(
            'Segurança (ADR 023): tokens pessoais de acesso ao Registry só são aceitos de origem interna ou VPN — '
            'pela borda pública eles são recusados automaticamente.',
          ),
        ),
        const SizedBox(height: 12),
        if (justCreated != null) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.success,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        'Token "${justCreated.token.name}" criado — copie agora, ele não será exibido novamente:',
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                    ),
                    IconButton(
                      tooltip: 'Ocultar aviso',
                      icon: const Icon(Icons.close, size: 18),
                      onPressed: () => ref
                          .read(registryProvider.notifier)
                          .clearJustCreatedToken(),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                CodeViewer(title: 'Segredo do token', code: justCreated.secret),
                const SizedBox(height: 8),
                CodeViewer(
                  title: 'Login no Docker',
                  code: 'docker login $host -u token',
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (ref.watch(authProvider).canWriteIn(ProductArea.servers))
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              onPressed: () => showCreateRegistryTokenSheet(context),
              icon: const Icon(Icons.add),
              label: const Text('Novo token pessoal'),
            ),
          ),
        const SizedBox(height: 12),
        if (reg.loadingTokens && !reg.tokensLoaded)
          const LoadingStateView(message: 'Carregando tokens...')
        else if (reg.tokens.isEmpty)
          const EmptyStateView(
            title: 'Nenhum token pessoal criado',
            icon: Icons.vpn_key_outlined,
          )
        else
          ...reg.tokens.map(
            (t) => Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: NestedPanel(
                tone: t.revoked || t.expired ? SemanticTone.secondary : null,
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Wrap(
                            spacing: 6,
                            runSpacing: 4,
                            children: <Widget>[
                              Text(
                                t.name,
                                style: const TextStyle(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              SemanticChip(
                                label: t.hint,
                                tone: SemanticTone.secondary,
                                compact: true,
                              ),
                              if (t.revoked)
                                const SemanticChip(
                                  label: 'Revogado',
                                  tone: SemanticTone.error,
                                  compact: true,
                                ),
                              if (t.expired)
                                const SemanticChip(
                                  label: 'Vencido',
                                  tone: SemanticTone.warning,
                                  compact: true,
                                ),
                            ],
                          ),
                          const SizedBox(height: 4),
                          Text(
                            'Escopo: ${t.repositories.join(', ')} · '
                            'Ações: ${t.actions.map(registryActionLabel).join(', ')} · '
                            'Último uso: ${formatRelativeTime(t.lastUsedAt)}',
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    if (!t.revoked &&
                        ref.watch(authProvider).canWriteIn(ProductArea.servers))
                      IconButton(
                        tooltip: 'Revogar token',
                        icon: const Icon(Icons.delete_outline),
                        onPressed: () async {
                          final ok = await showConfirmActionDialog(
                            context: context,
                            title: 'Revogar token ${t.name}?',
                            message:
                                'O token deixará de autenticar imediatamente.',
                            confirmLabel: 'Revogar',
                            tone: SemanticTone.error,
                          );
                          if (!ok || !context.mounted) return;
                          try {
                            await ref
                                .read(registryProvider.notifier)
                                .revokeToken(t.id);
                          } catch (err) {
                            if (!context.mounted) return;
                            ScaffoldMessenger.of(context).showSnackBar(
                              SnackBar(content: Text(describeError(err))),
                            );
                          }
                        },
                      ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// Aba "Vulnerabilidades / Segurança" do Registry — consome `registryProvider`.
class RegistryVulnerabilitiesTab extends ConsumerWidget {
  const RegistryVulnerabilitiesTab({
    super.key,
    required this.overview,
    required this.canAdmin,
  });

  final RegistryOverview? overview;
  final bool canAdmin;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final reg = ref.watch(registryProvider);
    final feeds = overview?.feeds ?? const <RegistryFeedView>[];
    final isCompact = MediaQuery.sizeOf(context).width < 560;

    final syncBtn = canAdmin
        ? FilledButton.tonalIcon(
            onPressed: () async {
              try {
                await ref.read(registryProvider.notifier).syncAdvisories();
                if (!context.mounted) return;
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Sincronização OSV iniciada.')),
                );
              } catch (err) {
                if (!context.mounted) return;
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('Aviso: ${describeError(err)}')),
                );
              }
            },
            icon: const Icon(Icons.sync, size: 16),
            label: const Text('Atualizar agora'),
          )
        : null;

    final newExceptionBtn = canAdmin
        ? FilledButton.tonalIcon(
            onPressed: () => showCreateRegistryExceptionSheet(context),
            icon: const Icon(Icons.add, size: 16),
            label: const Text('Nova exceção'),
          )
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SectionCard(
          title: 'Bases locais de vulnerabilidades (OSV.dev)',
          subtitle:
              'Varredura offline por SBOM — nunca executa o conteúdo da imagem.',
          icon: Icons.shield_outlined,
          iconTone: SemanticTone.info,
          trailing: isCompact ? null : syncBtn,
          footer: isCompact && syncBtn != null
              ? Align(alignment: Alignment.centerRight, child: syncBtn)
              : null,
          child: feeds.isEmpty
              ? const EmptyStateView(title: 'Nenhuma base sincronizada ainda')
              : Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: feeds
                      .map((f) {
                        return SemanticChip(
                          label:
                              '${f.ecosystem}: ${formatCompactCount(f.entries)} (${formatRelativeTime(f.syncedAt)})',
                          tone: f.error != null
                              ? SemanticTone.error
                              : SemanticTone.success,
                        );
                      })
                      .toList(growable: false),
                ),
        ),
        const SizedBox(height: 14),
        SectionCard(
          title: 'Exceções de risco aceitas',
          icon: Icons.rule_outlined,
          iconTone: SemanticTone.warning,
          trailing: isCompact ? null : newExceptionBtn,
          footer: isCompact && newExceptionBtn != null
              ? Align(alignment: Alignment.centerRight, child: newExceptionBtn)
              : null,
          child: reg.loadingExceptions && !reg.exceptionsLoaded
              ? const LoadingStateView(message: 'Carregando exceções...')
              : reg.exceptions.isEmpty
              ? const EmptyStateView(title: 'Nenhuma exceção registrada')
              : Column(
                  children: reg.exceptions
                      .map((ex) {
                        return Padding(
                          padding: const EdgeInsets.only(bottom: 8),
                          child: NestedPanel(
                            child: Row(
                              children: <Widget>[
                                Expanded(
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: <Widget>[
                                      Wrap(
                                        spacing: 6,
                                        children: <Widget>[
                                          Text(
                                            ex.advisory,
                                            style: const TextStyle(
                                              fontFamily: 'monospace',
                                              fontWeight: FontWeight.w700,
                                            ),
                                          ),
                                          SemanticChip(
                                            label:
                                                ex.application ??
                                                'Todas as aplicações',
                                            tone: SemanticTone.secondary,
                                            compact: true,
                                          ),
                                          if (ex.expired)
                                            const SemanticChip(
                                              label: 'Expirada',
                                              tone: SemanticTone.warning,
                                              compact: true,
                                            ),
                                        ],
                                      ),
                                      const SizedBox(height: 4),
                                      Text(
                                        ex.reason,
                                        style: context.textTheme.bodySmall,
                                      ),
                                      Text(
                                        'Validade: ${formatDate(ex.expiresAt)}',
                                        style: context.textTheme.bodySmall
                                            ?.copyWith(
                                              color: context
                                                  .colorScheme
                                                  .onSurfaceVariant,
                                            ),
                                      ),
                                    ],
                                  ),
                                ),
                                if (canAdmin)
                                  IconButton(
                                    icon: const Icon(Icons.delete_outline),
                                    onPressed: () async {
                                      try {
                                        await ref
                                            .read(registryProvider.notifier)
                                            .removeException(ex.id);
                                      } catch (err) {
                                        if (!context.mounted) return;
                                        ScaffoldMessenger.of(
                                          context,
                                        ).showSnackBar(
                                          SnackBar(
                                            content: Text(describeError(err)),
                                          ),
                                        );
                                      }
                                    },
                                  ),
                              ],
                            ),
                          ),
                        );
                      })
                      .toList(growable: false),
                ),
        ),
      ],
    );
  }
}
